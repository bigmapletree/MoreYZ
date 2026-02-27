import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import "./Overlay.css";

interface CombatRecord {
  id: string;
  start_time: string;
  end_time: string;
  duration_seconds: number;
  encounter_name: string | null;
  tyrant_count: number;
  hand_count: number;
  summons: Record<string, number>;
  lines: string[];
}

interface AppSettings {
  character_name: string;
  log_path: string;
  include_history: boolean;
}

type ViewMode = "init" | "list" | "detail";

function Overlay() {
  const [viewMode, setViewMode] = useState<ViewMode>("init");
  const [records, setRecords] = useState<CombatRecord[]>([]);
  const [selectedRecord, setSelectedRecord] = useState<CombatRecord | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [copySuccess, setCopySuccess] = useState(false);
  const [statusMsg, setStatusMsg] = useState("点击「设置」配置后开始");

  useEffect(() => {
    const unlisten = listen("log-changed", async () => {
      await handleCheckNew();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const loadRecords = async () => {
    try {
      const r = await invoke<CombatRecord[]>("get_combat_records");
      setRecords(r);
    } catch (e) {
      console.error("Failed to load records:", e);
    }
  };

  const handleInitialize = async (includeHistory: boolean) => {
    setIsLoading(true);
    try {
      const settings = await invoke<AppSettings>("get_settings");
      if (!settings.log_path || !settings.character_name) {
        setStatusMsg("请先在设置中配置日志路径和角色名");
        return;
      }

      const msg = await invoke<string>("initialize_log", { includeHistory });
      setStatusMsg(msg);
      await loadRecords();
      setViewMode("list");

      await invoke("start_watching");
    } catch (e) {
      setStatusMsg(`初始化失败: ${e}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleCheckNew = async () => {
    try {
      await invoke<string>("check_new_content");
      await loadRecords();
    } catch (e) {
      console.error("Check new content failed:", e);
    }
  };

  const handleOpenSettings = async () => {
    try {
      await invoke("open_settings_window");
    } catch (e) {
      console.error("Failed to open settings:", e);
    }
  };

  const handleSelectRecord = (record: CombatRecord) => {
    setSelectedRecord(record);
    setViewMode("detail");
  };

  const handleBackToList = () => {
    setSelectedRecord(null);
    setViewMode("list");
  };

  const handleCopy = async () => {
    try {
      let text = "";
      if (viewMode === "detail" && selectedRecord) {
        text = formatRecordDetail(selectedRecord);
      } else {
        text = records.map((r) => formatRecordSummary(r)).join("\n\n");
      }
      await writeText(text);
      setCopySuccess(true);
      setTimeout(() => setCopySuccess(false), 2000);
    } catch (e) {
      console.error("Failed to copy:", e);
    }
  };

  const handleClearRecords = async () => {
    try {
      await invoke("clear_records");
      setRecords([]);
    } catch (e) {
      console.error("Failed to clear:", e);
    }
  };

  const handleStartDrag = async (e: React.MouseEvent) => {
    e.preventDefault();
    const win = getCurrentWindow();
    await win.startDragging();
  };

  const formatRecordSummary = (r: CombatRecord) => {
    const name = r.encounter_name || "野外战斗";
    return `[${r.start_time}] ${name} - 暴君:${r.tyrant_count} 古手:${r.hand_count}`;
  };

  const formatRecordDetail = (r: CombatRecord) => {
    const lines = [
      `=== ${r.encounter_name || "战斗记录"} ===`,
      `开始: ${r.start_time}`,
      `结束: ${r.end_time}`,
      ``,
      `恶魔暴君施放: ${r.tyrant_count} 次`,
      `古尔丹之手施放: ${r.hand_count} 次`,
      ``,
      `召唤恶魔统计:`,
    ];
    for (const [demon, count] of Object.entries(r.summons)) {
      lines.push(`  ${demon}: ${count}`);
    }
    if (Object.keys(r.summons).length === 0) {
      lines.push(`  (无记录)`);
    }
    return lines.join("\n");
  };

  const renderInitView = () => (
    <div className="init-view">
      <h3>MoreYZ 战斗分析</h3>
      <p className="status-msg">{statusMsg}</p>
      <div className="init-buttons">
        <button
          className="init-btn"
          onClick={() => handleInitialize(false)}
          disabled={isLoading}
        >
          {isLoading ? "初始化中..." : "忽略历史，从现在开始"}
        </button>
        <button
          className="init-btn secondary"
          onClick={() => handleInitialize(true)}
          disabled={isLoading}
        >
          包含历史记录
        </button>
      </div>
    </div>
  );

  const renderListView = () => (
    <div className="list-view">
      <div className="list-header">
        <span>战斗记录 ({records.length})</span>
        <button className="clear-btn" onClick={handleClearRecords} title="清空记录">
          🗑️
        </button>
      </div>
      <div className="record-list">
        {records.length === 0 ? (
          <div className="empty-msg">暂无战斗记录，等待脱战...</div>
        ) : (
          records
            .slice()
            .reverse()
            .map((r) => (
              <div
                key={r.id}
                className="record-item"
                onClick={() => handleSelectRecord(r)}
              >
                <div className="record-name">
                  {r.encounter_name || "野外战斗"}
                </div>
                <div className="record-time">{r.start_time}</div>
                <div className="record-stats">
                  <span className="stat tyrant">暴君 {r.tyrant_count}</span>
                  <span className="stat hand">古手 {r.hand_count}</span>
                </div>
              </div>
            ))
        )}
      </div>
    </div>
  );

  const renderDetailView = () => {
    if (!selectedRecord) return null;
    const r = selectedRecord;
    return (
      <div className="detail-view">
        <div className="detail-header">
          <button className="back-btn" onClick={handleBackToList}>
            ← 返回
          </button>
          <span className="detail-title">{r.encounter_name || "战斗详情"}</span>
        </div>
        <div className="detail-content">
          <div className="detail-section">
            <div className="detail-row">
              <span className="label">开始时间:</span>
              <span className="value">{r.start_time}</span>
            </div>
            <div className="detail-row">
              <span className="label">结束时间:</span>
              <span className="value">{r.end_time}</span>
            </div>
          </div>
          <div className="detail-section">
            <div className="detail-row highlight">
              <span className="label">恶魔暴君:</span>
              <span className="value">{r.tyrant_count} 次</span>
            </div>
            <div className="detail-row highlight">
              <span className="label">古尔丹之手:</span>
              <span className="value">{r.hand_count} 次</span>
            </div>
          </div>
          <div className="detail-section">
            <div className="section-title">召唤恶魔统计</div>
            {Object.entries(r.summons).length > 0 ? (
              Object.entries(r.summons).map(([demon, count]) => (
                <div key={demon} className="detail-row">
                  <span className="label">{demon}:</span>
                  <span className="value">{count}</span>
                </div>
              ))
            ) : (
              <div className="empty-msg">无召唤记录</div>
            )}
          </div>
        </div>
      </div>
    );
  };

  return (
    <div className="overlay-container">
      <div className="drag-area" onMouseDown={handleStartDrag}>
        <span className="drag-hint">⋮⋮ MoreYZ</span>
      </div>

      <div className="content-area">
        {viewMode === "init" && renderInitView()}
        {viewMode === "list" && renderListView()}
        {viewMode === "detail" && renderDetailView()}
      </div>

      <div className="button-area">
        <button
          className="overlay-btn settings-btn"
          onClick={handleOpenSettings}
          title="打开设置"
        >
          ⚙️
        </button>
        {viewMode !== "init" && (
          <button
            className="overlay-btn refresh-btn"
            onClick={handleCheckNew}
            disabled={isLoading}
            title="检查新内容"
          >
            🔄
          </button>
        )}
        <button
          className="overlay-btn copy-btn"
          onClick={handleCopy}
          title="复制到剪贴板"
        >
          {copySuccess ? "✅" : "📋"}
        </button>
      </div>
    </div>
  );
}

export default Overlay;
