import { Clipboard, FolderOpen, Laptop, Link2, Send } from "lucide-react";
import { useAppState } from "../../store/appState";
import EdgeEntry from "../../components/EdgeEntry/EdgeEntry";
import TransferCard from "../../components/TransferCard/TransferCard";

export default function Dashboard() {
  const { device, peer, connectionStatus, settings, clipboard, tasks, history, refreshTasks } = useAppState();
  const active = tasks.filter((task) => ["queued", "prepared", "transferring", "retrying"].includes(task.status));
  const recent = history.slice(0, 3);

  return (
    <div className="page-grid">
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
