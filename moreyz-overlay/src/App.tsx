import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Overlay from "./components/Overlay";
import Settings from "./components/Settings";
import "./App.css";

function App() {
  const [windowLabel, setWindowLabel] = useState<string>("");

  useEffect(() => {
    const getWindowLabel = async () => {
      const win = getCurrentWindow();
      setWindowLabel(win.label);
    };
    getWindowLabel();
  }, []);

  if (windowLabel === "settings") {
    return <Settings />;
  }

  return <Overlay />;
}

export default App;
