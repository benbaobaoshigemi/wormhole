import { FolderOpen, Settings, Link2, ShieldAlert, FileText, Activity } from "lucide-react";
import { useAppState } from "../../store/appState";

export default function Diagnostics() {
  const { diagnostics, events } = useAppState();
  const recent = [...events].reverse().slice(0, 10);
  const firewallStatus =
    diagnostics?.incoming_traffic_received &&
    (diagnostics.firewall_status === "missing_rule" || diagnostics.firewall_status === "stale_program_path" || diagnostics.firewall_status === "unknown")
      ? "ok"
      : diagnostics?.firewall_status;

  return (
    <div className="single-column" style={{ maxWidth: "800px", margin: "0 auto", padding: "16px" }}>
      <section className="section-head" style={{ marginBottom: "24px" }}>
        <FolderOpen size={24} />
        <div>
          <h1>系统诊断</h1>
          <p>检查局域网通信、配置文件以及防火墙放行状态。</p>
        </div>
      </section>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(320px, 1fr))", gap: "16px", marginBottom: "24px" }}>
        {/* Core Daemon Status */}
        <section className="panel" style={{ padding: "16px" }}>
          <div className="panel-title" style={{ display: "flex", alignItems: "center", gap: "8px", marginBottom: "12px", borderBottom: "1px solid #e5e7eb", paddingBottom: "8px" }}>
            <Settings size={18} />
            <h2>Daemon 进程信息</h2>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "8px", fontSize: "14px" }}>
            <div><strong style={{ color: "#6b7280" }}>运行路径:</strong> <span style={{ fontFamily: "monospace", wordBreak: "break-all" }}>{diagnostics?.daemon_path ?? "加载中..."}</span></div>
            <div><strong style={{ color: "#6b7280" }}>配置目录:</strong> <span style={{ fontFamily: "monospace", wordBreak: "break-all" }}>{diagnostics?.config_path ?? "加载中..."}</span></div>
            <div><strong style={{ color: "#6b7280" }}>网络绑定:</strong> <span>{diagnostics?.bind_host ?? "0.0.0.0"}:{diagnostics?.local_port ?? "-"}</span></div>
          </div>
        </section>

        {/* Network & Firewall */}
        <section className="panel" style={{ padding: "16px" }}>
          <div className="panel-title" style={{ display: "flex", alignItems: "center", gap: "8px", marginBottom: "12px", borderBottom: "1px solid #e5e7eb", paddingBottom: "8px" }}>
            <ShieldAlert size={18} />
            <h2>网络分类与防火墙</h2>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "8px", fontSize: "14px" }}>
            <div><strong style={{ color: "#6b7280" }}>网络分类:</strong> <span>{diagnostics?.network_profile ?? "加载中..."}</span></div>
            <div>
              <strong style={{ color: "#6b7280" }}>防火墙状态:</strong>{" "}
              <span
                style={{
                  fontWeight: "bold",
                  color:
                    firewallStatus === "ok"
                      ? "#10b981"
                      : firewallStatus === "public_network"
                      ? "#f59e0b"
                      : "#ef4444",
                }}
              >
                {firewallStatus === "ok" && "已验证连通 (Ok)"}
                {firewallStatus === "missing_rule" && "规则缺失 (missing_rule)"}
                {firewallStatus === "stale_program_path" && "路径过期 (stale_program_path)"}
                {firewallStatus === "blocked_by_rule" && "被拦截 (blocked_by_rule)"}
                {firewallStatus === "public_network" && "公用网络限流 (public_network)"}
                {firewallStatus === "unknown" && "未知 (unknown)"}
                {!diagnostics && "读取中..."}
              </span>
            </div>
            <div><strong style={{ color: "#6b7280" }}>收到对端通信:</strong> <span>{diagnostics?.incoming_traffic_received ? "是 (有 incoming traffic)" : "否 (无 traffic)"}</span></div>
          </div>
        </section>

        {/* Connection Peer */}
        <section className="panel" style={{ padding: "16px" }}>
          <div className="panel-title" style={{ display: "flex", alignItems: "center", gap: "8px", marginBottom: "12px", borderBottom: "1px solid #e5e7eb", paddingBottom: "8px" }}>
            <Link2 size={18} />
            <h2>对端目标配置</h2>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "8px", fontSize: "14px" }}>
            <div><strong style={{ color: "#6b7280" }}>对端目标:</strong> <span>{diagnostics?.peer_host ?? "未配置"}:{diagnostics?.peer_port ?? "-"}</span></div>
            {diagnostics?.last_handshake_error && (
              <div style={{ marginTop: "6px", padding: "8px", backgroundColor: "#fffbeb", borderLeft: "3px solid #f59e0b", color: "#b45309", fontSize: "13px", wordBreak: "break-all" }}>
                <strong>握手失败记录:</strong> {diagnostics.last_handshake_error}
              </div>
            )}
          </div>
        </section>

        {/* Transfer Errors */}
        <section className="panel" style={{ padding: "16px" }}>
          <div className="panel-title" style={{ display: "flex", alignItems: "center", gap: "8px", marginBottom: "12px", borderBottom: "1px solid #e5e7eb", paddingBottom: "8px" }}>
            <Activity size={18} />
            <h2>最近传输异常</h2>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "8px", fontSize: "14px" }}>
            {diagnostics?.last_transfer_error_code ? (
              <div style={{ padding: "8px", backgroundColor: "#fef2f2", borderLeft: "3px solid #ef4444", color: "#991b1b", fontSize: "13px" }}>
                <div><strong>错误码:</strong> <code>{diagnostics.last_transfer_error_code}</code></div>
                <div style={{ marginTop: "4px", wordBreak: "break-all" }}><strong>详情:</strong> {diagnostics.last_transfer_error_message}</div>
              </div>
            ) : (
              <div style={{ color: "#6b7280", fontStyle: "italic" }}>近期无真实文件传输失败错误。</div>
            )}
          </div>
        </section>
      </div>

      {/* Recent Events Log */}
      <section className="panel" style={{ padding: "16px" }}>
        <div className="panel-title" style={{ display: "flex", alignItems: "center", gap: "8px", marginBottom: "12px", borderBottom: "1px solid #e5e7eb", paddingBottom: "8px" }}>
          <FileText size={18} />
          <h2>最近事件总线日志</h2>
        </div>
        {recent.length ? (
          <div style={{ display: "flex", flexDirection: "column", gap: "8px", maxHeight: "250px", overflowY: "auto", fontFamily: "monospace", fontSize: "13px" }}>
            {recent.map((event, idx) => (
              <div key={`${event.ts}-${event.type}-${idx}`} style={{ padding: "4px 8px", backgroundColor: "#f9fafb", borderBottom: "1px solid #f3f4f6" }}>
                <span style={{ color: "#9ca3af" }}>[{event.ts.split("T")[1]?.slice(0, 8) || event.ts}]</span>{" "}
                <span style={{ color: "#2563eb", fontWeight: "bold" }}>{event.type}</span>{" "}
                <span style={{ color: "#4b5563" }}>{JSON.stringify(event.data)}</span>
              </div>
            ))}
          </div>
        ) : (
          <p className="empty">暂无事件推送。</p>
        )}
      </section>
    </div>
  );
}
