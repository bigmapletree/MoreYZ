use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use chrono::{DateTime, Utc};

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
    pub duration_seconds: i64,
    pub encounter_name: Option<String>,
    pub tyrant_count: i32,
    pub hand_count: i32,
    pub summons: std::collections::HashMap<String, i32>,
    pub lines: Vec<String>,
}

struct AppState {
    settings: Mutex<AppSettings>,
    combat_records: Mutex<Vec<CombatRecord>>,
    last_read_position: Mutex<u64>,
    current_combat_lines: Mutex<Vec<String>>,
    in_combat: Mutex<bool>,
    combat_start_time: Mutex<Option<String>>,
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

    let path = PathBuf::from(log_path);
    if !path.exists() {
        return Err(format!("日志文件不存在: {}", log_path));
    }

    drop(settings);

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

    *state.last_read_position.lock().unwrap() = current_pos;
    Ok(())
}

fn process_line(state: &tauri::State<Arc<AppState>>, line: &str, character_name: &str) {
    let mut in_combat = state.in_combat.lock().unwrap();
    let mut current_lines = state.current_combat_lines.lock().unwrap();
    let mut combat_start = state.combat_start_time.lock().unwrap();

    if line.contains("ENCOUNTER_START") || line.contains("CHALLENGE_MODE_START") {
        *in_combat = true;
        current_lines.clear();
        *combat_start = extract_timestamp(line);
        current_lines.push(line.to_string());
    } else if line.contains("ENCOUNTER_END") || line.contains("CHALLENGE_MODE_END") {
        current_lines.push(line.to_string());
        
        if *in_combat {
            let record = create_combat_record(
                &current_lines,
                combat_start.clone(),
                extract_timestamp(line),
                extract_encounter_name(line),
                character_name,
            );
            
            let mut records = state.combat_records.lock().unwrap();
            records.push(record);
        }
        
        *in_combat = false;
        current_lines.clear();
        *combat_start = None;
    } else if *in_combat {
        current_lines.push(line.to_string());
    } else if line.contains("SPELL_CAST_SUCCESS") && line.contains(character_name) {
        if line.contains("265187") || line.contains("Demonic Tyrant") ||
           line.contains("105174") || line.contains("Hand of Gul'dan") {
            if !*in_combat {
                *in_combat = true;
                *combat_start = extract_timestamp(line);
            }
            current_lines.push(line.to_string());
        }
    } else if line.contains("SPELL_SUMMON") && line.contains(character_name) {
        if !current_lines.is_empty() {
            current_lines.push(line.to_string());
        }
    } else if line.contains("UNIT_DIED") && *in_combat {
        current_lines.push(line.to_string());
        
        let record = create_combat_record(
            &current_lines,
            combat_start.clone(),
            extract_timestamp(line),
            None,
            character_name,
        );
        
        if record.tyrant_count > 0 || record.hand_count > 0 {
            let mut records = state.combat_records.lock().unwrap();
            records.push(record);
        }
        
        *in_combat = false;
        current_lines.clear();
        *combat_start = None;
    }
}

fn create_combat_record(
    lines: &[String],
    start_time: Option<String>,
    end_time: Option<String>,
    encounter_name: Option<String>,
    character_name: &str,
) -> CombatRecord {
    let mut tyrant_count = 0;
    let mut hand_count = 0;
    let mut summons: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

    for line in lines {
        if line.contains("SPELL_CAST_SUCCESS") && line.contains(character_name) {
            if line.contains("265187") || line.contains("Demonic Tyrant") {
                tyrant_count += 1;
            }
            if line.contains("105174") || line.contains("Hand of Gul'dan") {
                hand_count += 1;
            }
        }
        if line.contains("SPELL_SUMMON") && line.contains(character_name) {
            if let Some(demon) = extract_summoned_demon(line) {
                *summons.entry(demon).or_insert(0) += 1;
            }
        }
    }

    let start = start_time.clone().unwrap_or_else(|| "unknown".to_string());
    let end = end_time.clone().unwrap_or_else(|| "unknown".to_string());
    
    let duration = calculate_duration(&start, &end);

    CombatRecord {
        id: format!("{}_{}", start.replace(['/', ':', ' ', '.'], ""), rand_suffix()),
        start_time: start,
        end_time: end,
        duration_seconds: duration,
        encounter_name,
        tyrant_count,
        hand_count,
        summons,
        lines: lines.to_vec(),
    }
}

fn extract_timestamp(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split("  ").collect();
    if !parts.is_empty() {
        Some(parts[0].to_string())
    } else {
        None
    }
}

fn extract_encounter_name(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() > 2 {
        Some(parts[2].trim_matches('"').to_string())
    } else {
        None
    }
}

fn extract_summoned_demon(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() > 9 {
        let demon_name = parts[9].trim_matches('"').to_string();
        if !demon_name.is_empty() && demon_name != "nil" {
            return Some(demon_name);
        }
    }
    None
}

fn calculate_duration(start: &str, end: &str) -> i64 {
    0
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
        current_combat_lines: Mutex::new(Vec::new()),
        in_combat: Mutex::new(false),
        combat_start_time: Mutex::new(None),
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
