import { ClipboardCheck, Image, Type } from "lucide-react";

export default function ClipboardStatusCard({
  kind,
  enabled,
  lastSync,
}: {
  kind: "text" | "image";
  enabled: boolean;
  lastSync?: string;
}) {
  const Icon = kind === "text" ? Type : Image;
  return (
    <article className="clipboard-card">
      <div className="panel-title">
        <Icon size={18} />
        <h2>{kind === "text" ? "文本同步" : "图片同步"}</h2>
      </div>
      <div className={enabled ? "big-state ok" : "big-state off"}>
        <ClipboardCheck size={22} />
        {enabled ? "自动同步正在运行" : "已关闭"}
      </div>
      <p className="muted">最后同步：{lastSync ?? "暂无真实同步事件"}</p>
      <p className="muted">剪贴板正文不会在控制中心展示。</p>
    </article>
  );
}
