import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./Settings.css";

interface AppSettings {
  character_name: string;
  log_path: string;
  include_history: boolean;
}

function Settings() {
  const [settings, setSettings] = useState<AppSettings>({
    character_name: "",
    log_path: "",
    include_history: false,
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
          <label>角色名称 *</label>
          <input
            type="text"
            value={settings.character_name}
            onChange={(e) =>
              setSettings({ ...settings, character_name: e.target.value })
            }
            placeholder="输入你的角色名（必须与日志中一致）"
          />
          <p className="hint">
            角色名必须与战斗日志中记录的名字完全一致
          </p>
        </div>

        <div className="form-group">
          <label>日志文件路径 *</label>
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
            Windows: World of Warcraft/_retail_/Logs/WoWCombatLog.txt
            <br />
            Mac: /Applications/World of Warcraft/_retail_/Logs/WoWCombatLog.txt
            <br />
            <strong>需要在游戏中输入 /combatlog 开启战斗日志</strong>
          </p>
        </div>

        <div className="info-box">
          <h4>使用说明</h4>
          <ol>
            <li>在游戏中输入 <code>/combatlog</code> 开启战斗日志记录</li>
            <li>填写上面的角色名和日志路径</li>
            <li>保存设置后，在主窗口点击「忽略历史」或「包含历史」初始化</li>
            <li>之后每次脱战，新的战斗记录会自动添加到列表</li>
          </ol>
        </div>
      </div>

      <div className="settings-footer">
        <span className="save-status">{saveStatus}</span>
        <button className="save-btn" onClick={handleSave}>
          保存设置
        </button>
      </div>
    </div>
  );
}

export default Settings;
