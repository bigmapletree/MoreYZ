import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import "./Overlay.css";

interface AppSettings {
  character_name: string;
  log_path: string;
  custom_code: string;
}

function Overlay() {
  const [analysisResult, setAnalysisResult] = useState<string>("等待分析...\n\n点击「设置」配置日志路径和角色名");
  const [isLoading, setIsLoading] = useState(false);
  const [copySuccess, setCopySuccess] = useState(false);

  useEffect(() => {
    const unlisten = listen("log-changed", async () => {
      await handleAnalyze();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleOpenSettings = async () => {
    try {
      await invoke("open_settings_window");
    } catch (e) {
      console.error("Failed to open settings:", e);
    }
  };

  const handleAnalyze = async () => {
    setIsLoading(true);
    try {
      const settings = await invoke<AppSettings>("get_settings");
      if (!settings.log_path) {
        setAnalysisResult("请先设置日志文件路径");
        return;
      }

      await invoke("read_combat_log");
      const result = await invoke<string>("analyze_log");
      setAnalysisResult(result);
    } catch (e) {
      setAnalysisResult(`分析失败: ${e}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleCopy = async () => {
    try {
      await writeText(analysisResult);
      setCopySuccess(true);
      setTimeout(() => setCopySuccess(false), 2000);
    } catch (e) {
      console.error("Failed to copy:", e);
    }
  };

  const handleStartDrag = async (e: React.MouseEvent) => {
    e.preventDefault();
    const win = getCurrentWindow();
    await win.startDragging();
  };

  return (
    <div className="overlay-container">
      <div className="drag-area" onMouseDown={handleStartDrag}>
        <span className="drag-hint">⋮⋮ 拖动</span>
      </div>

      <div className="content-area">
        <pre className="analysis-text">{analysisResult}</pre>
      </div>

      <div className="button-area">
        <button
          className="overlay-btn settings-btn"
          onClick={handleOpenSettings}
          title="打开设置"
        >
          ⚙️
        </button>
        <button
          className="overlay-btn refresh-btn"
          onClick={handleAnalyze}
          disabled={isLoading}
          title="刷新分析"
        >
          {isLoading ? "⏳" : "🔄"}
        </button>
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
