use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub character_name: String,
    pub log_path: String,
    pub custom_code: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            character_name: String::new(),
            log_path: String::new(),
            custom_code: r#"-- 自定义分析代码
-- 可用变量: lines (日志行数组), character_name (角色名)
-- 返回分析结果字符串

local result = {}
local tyrant_count = 0
local hand_count = 0
local summons = {}

for _, line in ipairs(lines) do
    if line:match("SPELL_CAST_SUCCESS") and line:match(character_name) then
        if line:match("265187") or line:match("Demonic Tyrant") then
            tyrant_count = tyrant_count + 1
        end
        if line:match("105174") or line:match("Hand of Gul'dan") then
            hand_count = hand_count + 1
        end
    end
    if line:match("SPELL_SUMMON") and line:match(character_name) then
        local demon = line:match("SPELL_SUMMON.-,\"([^\"]+)\"")
        if demon then
            summons[demon] = (summons[demon] or 0) + 1
        end
    end
end

table.insert(result, "=== 战斗分析 ===")
table.insert(result, "恶魔暴君施放: " .. tyrant_count .. " 次")
table.insert(result, "古尔丹之手施放: " .. hand_count .. " 次")
table.insert(result, "")
table.insert(result, "召唤恶魔统计:")
for demon, count in pairs(summons) do
    table.insert(result, "  " .. demon .. ": " .. count)
end

return table.concat(result, "\n")"#
                .to_string(),
        }
    }
}

struct AppState {
    settings: Mutex<AppSettings>,
    analysis_result: Mutex<String>,
    last_log_content: Mutex<String>,
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
fn get_analysis_result(state: tauri::State<Arc<AppState>>) -> String {
    state.analysis_result.lock().unwrap().clone()
}

#[tauri::command]
fn read_combat_log(state: tauri::State<Arc<AppState>>) -> Result<String, String> {
    let settings = state.settings.lock().unwrap();
    let log_path = &settings.log_path;

    if log_path.is_empty() {
        return Err("请先设置日志文件路径".to_string());
    }

    let path = PathBuf::from(log_path);
    if !path.exists() {
        return Err(format!("日志文件不存在: {}", log_path));
    }

    let content = fs::read(&path).map_err(|e| format!("读取文件失败: {}", e))?;

    let (decoded, _, _) = encoding_rs::UTF_8.decode(&content);
    let content_str = decoded.to_string();

    drop(settings);
    *state.last_log_content.lock().unwrap() = content_str.clone();

    Ok(content_str)
}

#[tauri::command]
fn analyze_log(state: tauri::State<Arc<AppState>>) -> Result<String, String> {
    let settings = state.settings.lock().unwrap();
    let log_content = state.last_log_content.lock().unwrap();

    if log_content.is_empty() {
        return Err("没有日志内容，请先读取日志".to_string());
    }

    let character_name = &settings.character_name;
    let lines: Vec<&str> = log_content.lines().collect();

    let mut tyrant_count = 0;
    let mut hand_count = 0;
    let mut summons: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

    for line in &lines {
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

    let mut result = Vec::new();
    result.push("=== 战斗分析 ===".to_string());
    result.push(format!("角色: {}", character_name));
    result.push(format!("日志行数: {}", lines.len()));
    result.push(String::new());
    result.push(format!("恶魔暴君施放: {} 次", tyrant_count));
    result.push(format!("古尔丹之手施放: {} 次", hand_count));
    result.push(String::new());
    result.push("召唤恶魔统计:".to_string());

    for (demon, count) in &summons {
        result.push(format!("  {}: {}", demon, count));
    }

    if summons.is_empty() {
        result.push("  (无召唤记录)".to_string());
    }

    let analysis = result.join("\n");

    drop(settings);
    drop(log_content);
    *state.analysis_result.lock().unwrap() = analysis.clone();

    Ok(analysis)
}

fn extract_summoned_demon(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() > 17 {
        let demon_name = parts[17].trim_matches('"').to_string();
        if !demon_name.is_empty() {
            return Some(demon_name);
        }
    }
    None
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
        analysis_result: Mutex::new(String::new()),
        last_log_content: Mutex::new(String::new()),
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
            get_analysis_result,
            read_combat_log,
            analyze_log,
            start_watching,
            open_settings_window,
            close_settings_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
