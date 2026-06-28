import { Trash2 } from "lucide-react";
import HistoryGroup from "../../components/HistoryGroup/HistoryGroup";
import { clearHistory } from "../../localApi/localClient";
import { useAppState } from "../../store/appState";
import type { TransferTaskDto } from "../../localApi/dto";

function groupByDay(items: TransferTaskDto[]) {
  return items.reduce<Record<string, TransferTaskDto[]>>((groups, item) => {
    const key = new Date(item.updated_at).toISOString().slice(0, 10);
    groups[key] = groups[key] ?? [];
    groups[key].push(item);
    return groups;
  }, {});
}

export default function HistoryPage() {
  const { history, refreshHistory } = useAppState();
  const groups = groupByDay(history);

  return (
    <div className="single-column">
      <section className="section-head split">
        <div>
          <h1>历史</h1>
          <p>来自真实传输历史。</p>
        </div>
        <button className="quiet" onClick={() => clearHistory().then(refreshHistory)} disabled={history.length === 0}>
          <Trash2 size={16} />
          清理历史
        </button>
      </section>
      {history.length === 0 && <p className="empty panel">暂无真实历史记录。</p>}
      {Object.entries(groups).map(([day, items]) => <HistoryGroup key={day} title={day} items={items} />)}
    </div>
  );
}
