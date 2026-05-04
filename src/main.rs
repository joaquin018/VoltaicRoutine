use serde::{Deserialize, Serialize};
use rusqlite::{params, Connection};
use std::sync::Mutex;
use tauri::{State, Manager};
use uuid::Uuid;
use chrono::Utc;

// --- Models ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Habit {
    pub id: String,
    pub name: String,
    pub detail: Option<String>,
    pub is_deleted: bool,
    pub hlc: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HabitEntry {
    pub id: String,
    pub habit_id: String,
    pub date: String,
    pub status: i32,
    pub is_deleted: bool,
    pub hlc: String,
}

// --- CRDT / HLC ---
pub struct AppState {
    pub db: Mutex<Connection>,
    pub node_id: String,
}

// --- Commands ---
#[tauri::command]
fn get_habits(state: State<AppState>) -> Result<Vec<Habit>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.prepare("SELECT id, name, detail, is_deleted, hlc FROM habits WHERE is_deleted = 0").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(Habit {
            id: row.get(0)?,
            name: row.get(1)?,
            detail: row.get(2)?,
            is_deleted: row.get(3)?,
            hlc: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut habits = Vec::new();
    for row in rows {
        habits.push(row.map_err(|e| e.to_string())?);
    }
    Ok(habits)
}

#[tauri::command]
fn upsert_habit(state: State<AppState>, id: Option<String>, name: String, detail: Option<String>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let habit_id = id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let hlc = format!("{}:0000:{}", Utc::now().timestamp_millis(), state.node_id);
    
    db.execute(
        "INSERT OR REPLACE INTO habits (id, name, detail, is_deleted, hlc) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![habit_id, name, detail, false, hlc],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_habit(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let hlc = format!("{}:0000:{}", Utc::now().timestamp_millis(), state.node_id);
    
    db.execute(
        "UPDATE habits SET is_deleted = 1, hlc = ?1 WHERE id = ?2",
        params![hlc, id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_entries(state: State<AppState>, habit_id: String) -> Result<Vec<HabitEntry>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.prepare("SELECT id, habit_id, date, status, is_deleted, hlc FROM entries WHERE habit_id = ?1 AND is_deleted = 0").map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![habit_id], |row| {
        Ok(HabitEntry {
            id: row.get(0)?,
            habit_id: row.get(1)?,
            date: row.get(2)?,
            status: row.get(3)?,
            is_deleted: row.get(4)?,
            hlc: row.get(5)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(|e| e.to_string())?);
    }
    Ok(entries)
}

#[tauri::command]
fn upsert_entry(state: State<AppState>, habit_id: String, date: String, status: i32) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let id = format!("{}_{}", habit_id, date);
    let hlc = format!("{}:0000:{}", Utc::now().timestamp_millis(), state.node_id);

    db.execute(
        "INSERT OR REPLACE INTO entries (id, habit_id, date, status, is_deleted, hlc) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, habit_id, date, status, false, hlc],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn save_routine(data: String) -> Result<(), String> {
    std::fs::write("routine_data.json", data).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_routine() -> Result<String, String> {
    std::fs::read_to_string("routine_data.json").unwrap_or_else(|_| "[]".to_string());
    // Use read_to_string, if file doesn't exist return empty array
    match std::fs::read_to_string("routine_data.json") {
        Ok(data) => Ok(data),
        Err(_) => Ok("[]".to_string()),
    }
}

fn main() {
    let db_path = "habits.db";
    let conn = Connection::open(db_path).expect("failed to open database");
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS habits (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            detail TEXT,
            is_deleted BOOLEAN NOT NULL DEFAULT 0,
            hlc TEXT NOT NULL
        )",
        [],
    ).expect("failed to create habits table");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS entries (
            id TEXT PRIMARY KEY,
            habit_id TEXT NOT NULL,
            date TEXT NOT NULL,
            status INTEGER NOT NULL,
            is_deleted BOOLEAN NOT NULL DEFAULT 0,
            hlc TEXT NOT NULL,
            FOREIGN KEY (habit_id) REFERENCES habits(id)
        )",
        [],
    ).expect("failed to create entries table");

    tauri::Builder::default()
        .manage(AppState {
            db: Mutex::new(conn),
            node_id: Uuid::new_v4().to_string(),
        })
        .invoke_handler(tauri::generate_handler![get_habits, upsert_habit, get_entries, upsert_entry, delete_habit, save_routine, load_routine])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
