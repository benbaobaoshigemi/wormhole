import { Clipboard, FolderOpen, Laptop, Link2, Send, AlertCircle } from "lucide-react";
import { useAppState } from "../../store/appState";
import EdgeEntry from "../../components/EdgeEntry/EdgeEntry";
import TransferCard from "../../components/TransferCard/TransferCard";

export default function Dashboard() {
  const { device, peer, connectionStatus, settings, clipboard, tasks, history, diagnostics, refreshTasks } = useAppState();
  const active = tasks.filter((task) => ["queued", "prepared", "transferring", "retrying"].includes(task.status));
  const recent = history.slice(0, 3);

  const showFirewallWarning = device?.platform === "windows" && diagnostics && diagnostics.firewall_status !== "ok" && diagnostics.firewall_status !== "unknown";
  const showConnectionWarning = connectionStatus === "peer_offline" && diagnostics?.incoming_traffic_received;

  return (
    <div className="page-grid">
      {showFirewallWarning && (
        <section className="panel warning-banner" style={{ gridColumn: "1 / -1", display: "flex", alignItems: "center", gap: "12px", borderLeft: "4px solid #e11d48", backgroundColor: "#fff1f2", color: "#9f1239", padding: "12px 16px", borderRadius: "6px" }}>
          <AlertCircle size={20} />
          <div>
            <strong>防火墙限制：</strong>
            {diagnostics.firewall_status === "public_network"
              ? "当前网络不是专用网络 (Private)。Wormhole 限制仅在专用网络和本地子网下被连接，当前网络下可能无法被对端连接。"
              : "Windows 入站防火墙未放行，对端可能无法连接本机，文件发送可能失败。请重新运行或尝试修复防火墙规则。"}
          </div>
        </section>
      )}

      {showConnectionWarning && (
        <section className="panel warning-banner" style={{ gridColumn: "1 / -1", display: "flex", alignItems: "center", gap: "12px", borderLeft: "4px solid #f59e0b", backgroundColor: "#fef3c7", color: "#92400e", padding: "12px 16px", borderRadius: "6px" }}>
          <AlertCircle size={20} />
          <div>
            <strong>连接警告：</strong>
            收到过对端请求，但本机无法主动连接对端。文件发送可能失败，请检查对端防火墙、地址和端口。
          </div>
        </section>
      )}

      <section className="connection-map">
        <article className="device-tile">
          <Laptop size={30} />
          <strong>{device?.device_name ?? "本机"}</strong>
          <p>{device?.platform ?? "local"} · {settings?.port ?? "-"}</p>
        </article>
        <div className={`link-line ${connectionStatus}`}>
          <Link2 size={22} />
          <span>{connectionStatus.replace("_", " ")}</span>
        </div>
        <article className="device-tile">
          <Laptop size={30} />
          <strong>{peer?.device_name ?? settings?.peer_name ?? "对端设备"}</strong>
          <p>{peer ? `${peer.platform} · ${peer.port}` : `${settings?.peer_host ?? "-"}:${settings?.peer_port ?? "-"}`}</p>
        </article>
      </section>

      <section className="panel hero-action">
        <div className="panel-title"><Send size={18} /><h2>发送文件</h2></div>
        <p>浏览器控制中心没有真实本机路径权限。请从系统托盘或菜单栏选择“发送文件 / 发送文件夹”。</p>
        <span className="badge neutral">真实路径由原生 launcher 提供</span>
      </section>

      <section className="metric-row">
        <article className="metric"><FolderOpen size={18} /><strong>{active.length}</strong><span>当前传输</span></article>
        <article className="metric"><Clipboard size={18} /><strong>{clipboard?.text_enabled ? "开" : "关"}</strong><span>文本剪贴板</span></article>
        <article className="metric"><Clipboard size={18} /><strong>{clipboard?.image_enabled ? "开" : "关"}</strong><span>图片剪贴板</span></article>
      </section>

      <section className="panel">
        <div className="panel-title"><FolderOpen size={18} /><h2>当前传输</h2></div>
        {active.length ? active.slice(0, 2).map((task) => <TransferCard key={task.task_id} task={task} onChanged={refreshTasks} />) : <p className="empty">没有正在进行的真实传输。</p>}
      </section>

      <section className="panel">
        <div className="panel-title"><FolderOpen size={18} /><h2>最近结果</h2></div>
        {recent.length ? recent.map((item) => <p key={`${item.task_id}-${item.updated_at}`} className="result-line">{item.root_name} · {item.status}</p>) : <p className="empty">暂无真实历史记录。</p>}
      </section>

      <EdgeEntry />
    </div>
  );
}
