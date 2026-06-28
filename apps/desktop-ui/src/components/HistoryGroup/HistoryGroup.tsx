import { File, Folder, Send, Undo2 } from "lucide-react";
import type { TransferTaskDto } from "../../localApi/dto";

function dayLabel(value: string) {
  return new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric", weekday: "short" }).format(new Date(value));
}

export default function HistoryGroup({ title, items }: { title: string; items: TransferTaskDto[] }) {
  return (
    <section className="history-group">
      <h2>{title}</h2>
      {items.map((item) => (
        <article key={`${item.task_id}-${item.updated_at}`} className="history-row">
          {item.item_count > 1 ? <Folder size={18} /> : <File size={18} />}
          <div>
            <strong>{item.root_name}</strong>
            <p>{dayLabel(item.updated_at)} · {item.direction === "send" ? "发送" : "接收"}</p>
          </div>
          {item.direction === "send" ? <Send size={16} /> : <Undo2 size={16} />}
          <span className={`badge ${item.status}`}>{item.status}</span>
        </article>
      ))}
    </section>
  );
}
