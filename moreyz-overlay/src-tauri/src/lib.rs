use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

const COMBAT_TIMEOUT_SECONDS: f64 = 8.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub character_name: String,
    pub log_path: String,
    pub include_history: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            character_name: String::new(),
            log_path: String::new(),
            include_history: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatRecord {
    pub id: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_seconds: f64,
    pub encounter_name: Option<String>,
    pub tyrant_count: i32,
    pub hand_count: i32,
    pub summons: std::collections::HashMap<String, i32>,
    pub total_events: usize,
}

#[derive(Debug, Clone)]
struct CombatSession {
    lines: Vec<String>,
    start_time: Option<f64>,
    start_time_str: Option<String>,
    last_event_time: Option<f64>,
    last_event_time_str: Option<String>,
    encounter_name: Option<String>,
    is_encounter: bool,
}

impl CombatSession {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            start_time: None,
            start_time_str: None,
            last_event_time: None,
            last_event_time_str: None,
            encounter_name: None,
            is_encounter: false,
        }
    }

    fn clear(&mut self) {
        self.lines.clear();
        self.start_time = None;
        self.start_time_str = None;
        self.last_event_time = None;
        self.last_event_time_str = None;
        self.encounter_name = None;
        self.is_encounter = false;
    }

    fn is_active(&self) -> bool {
        self.start_time.is_some()
    }
}

struct AppState {
    settings: Mutex<AppSettings>,
    combat_records: Mutex<Vec<CombatRecord>>,
    last_read_position: Mutex<u64>,
    current_session: Mutex<CombatSession>,
    initialized: Mutex<bool>,
}

#[tauri::command]
fn get_settings(state: tauri::State<Arc<AppState>>) -> AppSettings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(state: tauri::State<Arc<AppState>>, settings: AppSettings) {
    *state.settings.lock().unwrap() = settings;
}

#[tauri::command]
fn get_combat_records(state: tauri::State<Arc<AppState>>) -> Vec<CombatRecord> {
    state.combat_records.lock().unwrap().clone()
}

#[tauri::command]
fn get_combat_record_detail(state: tauri::State<Arc<AppState>>, id: String) -> Option<CombatRecord> {
    let records = state.combat_records.lock().unwrap();
    records.iter().find(|r| r.id == id).cloned()
}

#[tauri::command]
fn clear_records(state: tauri::State<Arc<AppState>>) {
    state.combat_records.lock().unwrap().clear();
}

#[tauri::command]
fn initialize_log(state: tauri::State<Arc<AppState>>, include_history: bool) -> Result<String, String> {
    let settings = state.settings.lock().unwrap();
    let log_path = &settings.log_path;
    let character_name = settings.character_name.clone();
    
    if log_path.is_empty() {
        return Err("请先设置日志文件路径".to_string());
    }
    if character_name.is_empty() {
        return Err("请先设置角色名称".to_string());
    }

    let path = PathBuf::from(log_path);
    if !path.exists() {
        return Err(format!("日志文件不存在: {}", log_path));
    }

    drop(settings);

    // Clear existing records
    state.combat_records.lock().unwrap().clear();
    state.current_session.lock().unwrap().clear();

    let file = File::open(&path).map_err(|e| format!("打开文件失败: {}", e))?;
    let file_size = file.metadata().map_err(|e| format!("获取文件信息失败: {}", e))?.len();

    if include_history {
        *state.last_read_position.lock().unwrap() = 0;
        process_log_content(&state, &path, 0, &character_name)?;
    } else {
        *state.last_read_position.lock().unwrap() = file_size;
    }

    *state.initialized.lock().unwrap() = true;

    let records_count = state.combat_records.lock().unwrap().len();
    Ok(format!("初始化完成，当前 {} 条战斗记录", records_count))
}

#[tauri::command]
fn check_new_content(state: tauri::State<Arc<AppState>>) -> Result<String, String> {
    if !*state.initialized.lock().unwrap() {
        return Err("请先初始化".to_string());
    }

    let settings = state.settings.lock().unwrap();
    let log_path = settings.log_path.clone();
    let character_name = settings.character_name.clone();
    drop(settings);

    if log_path.is_empty() {
        return Err("日志路径为空".to_string());
    }

    let path = PathBuf::from(&log_path);
    let last_pos = *state.last_read_position.lock().unwrap();
    
    process_log_content(&state, &path, last_pos, &character_name)?;

    let records_count = state.combat_records.lock().unwrap().len();
    Ok(format!("检查完成，当前 {} 条战斗记录", records_count))
}

fn process_log_content(
    state: &tauri::State<Arc<AppState>>,
    path: &PathBuf,
    start_pos: u64,
    character_name: &str,
) -> Result<(), String> {
    let file = File::open(path).map_err(|e| format!("打开文件失败: {}", e))?;
    let mut reader = BufReader::new(file);
    
    reader.seek(SeekFrom::Start(start_pos)).map_err(|e| format!("定位文件失败: {}", e))?;

    let mut line = String::new();
    let mut current_pos = start_pos;

    while reader.read_line(&mut line).map_err(|e| format!("读取行失败: {}", e))? > 0 {
        current_pos = reader.stream_position().map_err(|e| format!("获取位置失败: {}", e))?;
        
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            process_line(state, trimmed, character_name);
        }
        
        line.clear();
    }

    // After processing all lines, check if current session should be closed due to timeout
    // (This handles the case where the last combat hasn't ended yet)
    finalize_session_if_needed(state, character_name);

    *state.last_read_position.lock().unwrap() = current_pos;
    Ok(())
}

fn process_line(state: &tauri::State<Arc<AppState>>, line: &str, character_name: &str) {
    let timestamp = parse_timestamp(line);
    let timestamp_str = extract_timestamp_str(line);

    let mut session = state.current_session.lock().unwrap();

    // Handle ENCOUNTER_START
    if line.contains("ENCOUNTER_START") {
        // If there's an active non-encounter session, close it first
        if session.is_active() && !session.is_encounter {
            let record = create_combat_record(&session, character_name);
            if record.tyrant_count > 0 || record.hand_count > 0 || record.total_events > 5 {
                state.combat_records.lock().unwrap().push(record);
            }
        }
        
        session.clear();
        session.start_time = timestamp;
        session.start_time_str = timestamp_str.clone();
        session.last_event_time = timestamp;
        session.last_event_time_str = timestamp_str;
        session.encounter_name = extract_encounter_name(line);
        session.is_encounter = true;
        session.lines.push(line.to_string());
        return;
    }

    // Handle ENCOUNTER_END
    if line.contains("ENCOUNTER_END") {
        session.lines.push(line.to_string());
        session.last_event_time = timestamp;
        session.last_event_time_str = timestamp_str;
        
        if session.is_active() {
            let record = create_combat_record(&session, character_name);
            state.combat_records.lock().unwrap().push(record);
        }
        
        session.clear();
        return;
    }

    // For non-encounter combat, check timeout
    if session.is_active() && !session.is_encounter {
        if let (Some(last_time), Some(current_time)) = (session.last_event_time, timestamp) {
            if current_time - last_time > COMBAT_TIMEOUT_SECONDS {
                // Timeout! Close the current session
                let record = create_combat_record(&session, character_name);
                if record.tyrant_count > 0 || record.hand_count > 0 || record.total_events > 5 {
                    state.combat_records.lock().unwrap().push(record);
                }
                session.clear();
            }
        }
    }

    // Check if this line is a combat event for our character
    let is_relevant = is_relevant_combat_event(line, character_name);

    if is_relevant {
        if !session.is_active() {
            // Start a new combat session
            session.start_time = timestamp;
            session.start_time_str = timestamp_str.clone();
        }
        
        session.last_event_time = timestamp;
        session.last_event_time_str = timestamp_str;
        session.lines.push(line.to_string());
    } else if session.is_active() {
        // Even if not directly relevant, record combat events during active session
        if is_any_combat_event(line) {
            session.lines.push(line.to_string());
            session.last_event_time = timestamp;
            session.last_event_time_str = timestamp_str;
        }
    }
}

fn finalize_session_if_needed(state: &tauri::State<Arc<AppState>>, character_name: &str) {
    let mut session = state.current_session.lock().unwrap();
    
    if session.is_active() && !session.is_encounter {
        // For non-encounter sessions, we close them when processing ends
        // This ensures the last combat is recorded
        let record = create_combat_record(&session, character_name);
        if record.tyrant_count > 0 || record.hand_count > 0 || record.total_events > 5 {
            state.combat_records.lock().unwrap().push(record);
        }
        session.clear();
    }
}

fn is_relevant_combat_event(line: &str, character_name: &str) -> bool {
    if !line.contains(character_name) {
        return false;
    }
    
    // Combat events that indicate active combat
    line.contains("SPELL_CAST_SUCCESS") ||
    line.contains("SPELL_CAST_START") ||
    line.contains("SPELL_DAMAGE") ||
    line.contains("SPELL_PERIODIC_DAMAGE") ||
    line.contains("SPELL_SUMMON") ||
    line.contains("SPELL_HEAL") ||
    line.contains("SWING_DAMAGE") ||
    line.contains("RANGE_DAMAGE")
}

fn is_any_combat_event(line: &str) -> bool {
    line.contains("SPELL_") ||
    line.contains("SWING_") ||
    line.contains("RANGE_") ||
    line.contains("DAMAGE_") ||
    line.contains("UNIT_DIED")
}

fn create_combat_record(session: &CombatSession, character_name: &str) -> CombatRecord {
    let mut tyrant_count = 0;
    let mut hand_count = 0;
    let mut summons: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

    for line in &session.lines {
        if line.contains("SPELL_CAST_SUCCESS") && line.contains(character_name) {
            if line.contains("265187") || line.contains("Demonic Tyrant") || line.contains("恶魔暴君") {
                tyrant_count += 1;
            }
            if line.contains("105174") || line.contains("Hand of Gul'dan") || line.contains("古尔丹之手") {
                hand_count += 1;
            }
        }
        if line.contains("SPELL_SUMMON") && line.contains(character_name) {
            if let Some(demon) = extract_summoned_demon(line) {
                *summons.entry(demon).or_insert(0) += 1;
            }
        }
    }

    let start_str = session.start_time_str.clone().unwrap_or_else(|| "unknown".to_string());
    let end_str = session.last_event_time_str.clone().unwrap_or_else(|| "unknown".to_string());
    
    let duration = match (session.start_time, session.last_event_time) {
        (Some(start), Some(end)) => end - start,
        _ => 0.0,
    };

    CombatRecord {
        id: format!("{}_{}", start_str.replace(['/', ':', ' ', '.'], ""), rand_suffix()),
        start_time: start_str,
        end_time: end_str,
        duration_seconds: duration,
        encounter_name: session.encounter_name.clone(),
        tyrant_count,
        hand_count,
        summons,
        total_events: session.lines.len(),
    }
}

fn parse_timestamp(line: &str) -> Option<f64> {
    // WoW combat log format: "M/D HH:MM:SS.mmm  EVENT,..."
    // Example: "1/15 20:30:45.123  SPELL_CAST_SUCCESS,..."
    let parts: Vec<&str> = line.splitn(2, "  ").collect();
    if parts.is_empty() {
        return None;
    }
    
    let time_part = parts[0];
    // Parse "M/D HH:MM:SS.mmm"
    let time_parts: Vec<&str> = time_part.split(' ').collect();
    if time_parts.len() < 2 {
        return None;
    }
    
    let clock_part = time_parts.last()?;
    let clock_parts: Vec<&str> = clock_part.split(':').collect();
    if clock_parts.len() < 3 {
        return None;
    }
    
    let hours: f64 = clock_parts[0].parse().ok()?;
    let minutes: f64 = clock_parts[1].parse().ok()?;
    let seconds: f64 = clock_parts[2].parse().ok()?;
    
    // Convert to seconds since midnight for comparison
    // Note: This doesn't handle day boundaries, but should work for most cases
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

fn extract_timestamp_str(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.splitn(2, "  ").collect();
    if !parts.is_empty() {
        Some(parts[0].to_string())
    } else {
        None
    }
}

fn extract_encounter_name(line: &str) -> Option<String> {
    // ENCOUNTER_START format: timestamp  ENCOUNTER_START,encounterID,"encounterName",difficultyID,...
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() > 2 {
        Some(parts[2].trim_matches('"').to_string())
    } else {
        None
    }
}

fn extract_summoned_demon(line: &str) -> Option<String> {
    // SPELL_SUMMON format: ...,destGUID,destName,destFlags,...
    // destName is typically at position 9 (0-indexed)
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() > 9 {
        let demon_name = parts[9].trim_matches('"').to_string();
        if !demon_name.is_empty() && demon_name != "nil" && demon_name != "0x0000000000000000" {
            return Some(demon_name);
        }
    }
    None
}

fn rand_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    format!("{:08x}", nanos)
}

#[tauri::command]
async fn start_watching(app: AppHandle, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let settings = state.settings.lock().unwrap();
    let log_path = settings.log_path.clone();
    drop(settings);

    if log_path.is_empty() {
        return Err("请先设置日志文件路径".to_string());
    }

    let path = PathBuf::from(&log_path);
    if !path.exists() {
        return Err(format!("日志文件不存在: {}", log_path));
    }

    let app_clone = app.clone();

    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();

        let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();

        if let Some(parent) = path.parent() {
            watcher.watch(parent, RecursiveMode::NonRecursive).unwrap();
        }

        loop {
            match rx.recv() {
                Ok(_event) => {
                    let _ = app_clone.emit("log-changed", ());
                }
                Err(e) => {
                    eprintln!("Watch error: {:?}", e);
                    break;
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn close_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = Arc::new(AppState {
        settings: Mutex::new(AppSettings::default()),
        combat_records: Mutex::new(Vec::new()),
        last_read_position: Mutex::new(0),
        current_session: Mutex::new(CombatSession::new()),
        initialized: Mutex::new(false),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_combat_records,
            get_combat_record_detail,
            clear_records,
            initialize_log,
            check_new_content,
            start_watching,
            open_settings_window,
            close_settings_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
