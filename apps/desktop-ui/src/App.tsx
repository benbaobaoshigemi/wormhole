import { useState } from "react";
import { Activity, Clipboard, Clock, Gauge, History, Settings, SlidersHorizontal } from "lucide-react";
import { useAppState } from "./store/appState";
import Dashboard from "./pages/Dashboard/Dashboard";
import Onboarding from "./pages/Onboarding/Onboarding";
import Transfers from "./pages/Transfers/Transfers";
import HistoryPage from "./pages/History/History";
import ClipboardPage from "./pages/Clipboard/Clipboard";
import SettingsPage from "./pages/Settings/Settings";
import Diagnostics from "./pages/Diagnostics/Diagnostics";

type Page = "dashboard" | "transfers" | "history" | "clipboard" | "settings" | "diagnostics";

const pages: Array<{ id: Page; label: string; icon: typeof Gauge }> = [
  { id: "dashboard", label: "总览", icon: Gauge },
  { id: "transfers", label: "传输", icon: Activity },
  { id: "history", label: "历史", icon: History },
  { id: "clipboard", label: "剪贴板", icon: Clipboard },
  { id: "settings", label: "设置", icon: SlidersHorizontal },
  { id: "diagnostics", label: "诊断", icon: Settings },
];

function App() {
  const { device, connectionStatus, daemonStatus } = useAppState();
  const [page, setPage] = useState<Page>("dashboard");
  const [onboarded, setOnboarded] = useState(() => localStorage.getItem("wormhole_onboarding_complete") === "true");

  if (!onboarded) {
    return <Onboarding onComplete={() => setOnboarded(true)} />;
  }

  const PageComponent =
    page === "dashboard"
      ? Dashboard
      : page === "transfers"
        ? Transfers
        : page === "history"
          ? HistoryPage
          : page === "clipboard"
            ? ClipboardPage
            : page === "settings"
              ? SettingsPage
              : Diagnostics;

  return (
    <div className="app">
      <header className="topbar">
        <div>
          <div className="brand">Wormhole</div>
          <div className="device-line">{device?.device_name ?? "等待 daemon"} · {device?.platform ?? "local"}</div>
        </div>
        <nav className="nav-tabs" aria-label="Wormhole sections">
          {pages.map((item) => {
            const Icon = item.icon;
            return (
              <button key={item.id} className={page === item.id ? "active" : ""} onClick={() => setPage(item.id)}>
                <Icon size={16} />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>
        <div className={`status-pill ${connectionStatus}`}>
          <Clock size={14} />
          {daemonStatus === "online" ? connectionStatus.replace("_", " ") : "daemon offline"}
        </div>
      </header>
      <main>
        <PageComponent />
      </main>
    </div>
  );
}

export default App;
