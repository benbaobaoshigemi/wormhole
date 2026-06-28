import { RotateCcw, StopCircle } from "lucide-react";
import { cancelTransfer, retryTransfer } from "../../localApi/localClient";
import type { TransferTaskDto } from "../../localApi/dto";

function bytes(value: number) {
  if (!value) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  return `${(value / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function percent(task: TransferTaskDto) {
  if (task.total_size === 0) return task.status === "completed" ? 100 : 0;
  return Math.min(100, Math.round((task.transferred_size / task.total_size) * 100));
}

export default function TransferCard({ task, onChanged }: { task: TransferTaskDto; onChanged: () => void }) {
  const active = ["queued", "prepared", "transferring", "retrying"].includes(task.status);
  const progress = percent(task);
  return (
    <article className={`transfer-card ${active ? "active" : ""} ${task.status}`}>
      <div className="transfer-head">
        <div>
          <strong>{task.root_name}</strong>
          <p>{task.direction === "send" ? "本机 -> 对端" : "对端 -> 本机"} · {task.item_count} 项 · {bytes(task.total_size)}</p>
        </div>
        <span className={`badge ${task.status}`}>{task.status}</span>
      </div>
      <div className="progress-track"><div style={{ width: `${progress}%` }} /></div>
      <div className="transfer-meta">
        <span>{bytes(task.transferred_size)} / {bytes(task.total_size)}</span>
        <span>{bytes(task.speed_bytes_per_sec)}/s</span>
        <span>{task.eta_seconds ? `${task.eta_seconds}s` : "ETA -"}</span>
      </div>
      {task.error && <p className="error-text">{task.error}</p>}
      <div className="card-actions">
        {active && (
          <button onClick={() => cancelTransfer(task.task_id).then(onChanged)}>
            <StopCircle size={16} />
            取消
          </button>
        )}
        {task.status === "failed" && (
          <button onClick={() => retryTransfer(task.task_id).then(onChanged)}>
            <RotateCcw size={16} />
            重试
          </button>
        )}
      </div>
    </article>
  );
}
