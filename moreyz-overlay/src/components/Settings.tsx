import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./Settings.css";

interface AppSettings {
  character_name: string;
  log_path: string;
  custom_code: string;
}

function Settings() {
  const [settings, setSettings] = useState<AppSettings>({
    character_name: "",
    log_path: "",
    custom_code: "",
  });
  const [saveStatus, setSaveStatus] = useState<string>("");

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const s = await invoke<AppSettings>("get_settings");
      setSettings(s);
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  };

  const handleSave = async () => {
    try {
      await invoke("save_settings", { settings });
      setSaveStatus("✅ 已保存");
      setTimeout(() => setSaveStatus(""), 2000);
    } catch (e) {
      setSaveStatus(`❌ 保存失败: ${e}`);
    }
  };

  const handleBrowseLog = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Combat Log",
            extensions: ["txt"],
          },
        ],
      });
      if (selected) {
        setSettings({ ...settings, log_path: selected as string });
      }
    } catch (e) {
      console.error("Failed to browse:", e);
    }
  };

  const handleClose = async () => {
    try {
      await invoke("close_settings_window");
    } catch (e) {
      console.error("Failed to close:", e);
    }
  };

  const handleStartWatching = async () => {
    try {
      await invoke("start_watching");
      setSaveStatus("✅ 开始监控日志文件");
      setTimeout(() => setSaveStatus(""), 2000);
    } catch (e) {
      setSaveStatus(`❌ 监控失败: ${e}`);
    }
  };

  return (
    <div className="settings-container">
      <div className="settings-header">
        <h2>MoreYZ 设置</h2>
        <button className="close-btn" onClick={handleClose}>
          ✕
        </button>
      </div>

      <div className="settings-content">
        <div className="form-group">
          <label>角色名称</label>
          <input
            type="text"
            value={settings.character_name}
            onChange={(e) =>
              setSettings({ ...settings, character_name: e.target.value })
            }
            placeholder="输入你的角色名..."
          />
        </div>

        <div className="form-group">
          <label>日志文件路径</label>
          <div className="path-input">
            <input
              type="text"
              value={settings.log_path}
              onChange={(e) =>
                setSettings({ ...settings, log_path: e.target.value })
              }
              placeholder="WoWCombatLog.txt 路径..."
            />
            <button className="browse-btn" onClick={handleBrowseLog}>
              浏览...
            </button>
          </div>
          <p className="hint">
            通常位于: World of Warcraft/_retail_/Logs/WoWCombatLog.txt
            <br />
            需要在游戏中输入 /combatlog 开启战斗日志
          </p>
        </div>

        <div className="form-group">
          <label>自定义分析代码 (Lua)</label>
          <textarea
            value={settings.custom_code}
            onChange={(e) =>
              setSettings({ ...settings, custom_code: e.target.value })
            }
            placeholder="-- 自定义 Lua 分析代码..."
            rows={12}
          />
          <p className="hint">
            注意: 当前版本使用 Rust 内置分析，自定义代码功能待后续版本实现
          </p>
        </div>
      </div>

      <div className="settings-footer">
        <span className="save-status">{saveStatus}</span>
        <div className="footer-buttons">
          <button className="watch-btn" onClick={handleStartWatching}>
            开始监控
          </button>
          <button className="save-btn" onClick={handleSave}>
            保存设置
          </button>
        </div>
      </div>
    </div>
  );
}

export default Settings;
