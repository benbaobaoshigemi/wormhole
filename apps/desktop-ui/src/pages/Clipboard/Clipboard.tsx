import { Power } from "lucide-react";
import ClipboardStatusCard from "../../components/ClipboardStatusCard/ClipboardStatusCard";
import { disableClipboard, enableClipboard } from "../../localApi/localClient";
import { useAppState } from "../../store/appState";

function lastClipboardEvent(kind: string, events: { type: string; ts: string; data: Record<string, unknown> }[]) {
  const event = [...events].reverse().find((item) => item.type.startsWith("clipboard.") && item.data.kind === kind);
  return event ? new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" }).format(new Date(event.ts)) : undefined;
}

export default function ClipboardPage() {
  const { clipboard, events, refreshClipboard } = useAppState();
  const enabled = clipboard?.enabled ?? false;

  return (
    <div className="single-column">
      <section className="section-head split">
        <div>
          <h1>剪贴板</h1>
          <p>自动同步状态来自 daemon，不显示剪贴板正文。</p>
        </div>
        <button className="primary" onClick={() => (enabled ? disableClipboard() : enableClipboard()).then(refreshClipboard)}>
          <Power size={16} />
          {enabled ? "关闭同步" : "开启同步"}
        </button>
      </section>
      <div className="two-col">
        <ClipboardStatusCard kind="text" enabled={enabled && Boolean(clipboard?.text_enabled)} lastSync={lastClipboardEvent("text", events)} />
        <ClipboardStatusCard kind="image" enabled={enabled && Boolean(clipboard?.image_enabled)} lastSync={lastClipboardEvent("image", events)} />
      </div>
      <section className="panel">
        <h2>异常</h2>
        <p className="muted">最近事件中没有真实剪贴板异常。</p>
      </section>
    </div>
  );
}
