import { FolderOpen } from "lucide-react";
import { useAppState } from "../../store/appState";

export default function Diagnostics() {
  const { daemonStatus, localApiUrl, connectionStatus, peer, settings, events, lastError } = useAppState();
  const recent = [...events].reverse().slice(0, 8);

  return (
    <div className="single-column">
      <section className="section-head">
        <FolderOpen size={24} />
        <div><h1>诊断</h1><p>运行状态和最近事件。</p></div>
      </section>
      <section className="diagnostics-grid">
        <div className="panel"><h2>daemon</h2><p>{daemonStatus}</p></div>
        <div className="panel"><h2>local API</h2><p>{localApiUrl}</p></div>
        <div className="panel"><h2>对端</h2><p>{connectionStatus} · {peer?.device_name ?? settings?.peer_host ?? "-"}</p></div>
        <div className="panel"><h2>版本</h2><p>0.1.0</p></div>
        <div className="panel"><h2>日志目录</h2><p>请从托盘 / 菜单栏打开日志目录或导出日志。</p></div>
        <div className="panel"><h2>最近错误</h2><p>{lastError ?? "无真实错误记录"}</p></div>
      </section>
      <section className="panel">
        <h2>最近事件</h2>
        {recent.length ? recent.map((event) => <p key={`${event.ts}-${event.type}`}>{event.ts} · {event.type}</p>) : <p className="empty">暂无事件。</p>}
      </section>
    </div>
  );
}
