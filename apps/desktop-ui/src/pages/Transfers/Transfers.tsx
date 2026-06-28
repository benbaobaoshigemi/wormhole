import { Activity } from "lucide-react";
import TransferCard from "../../components/TransferCard/TransferCard";
import { useAppState } from "../../store/appState";

export default function Transfers() {
  const { tasks, refreshTasks } = useAppState();
  const active = tasks.filter((task) => ["queued", "prepared", "transferring", "retrying"].includes(task.status));
  const rest = tasks.filter((task) => !active.includes(task));

  return (
    <div className="single-column">
      <section className="section-head">
        <Activity size={24} />
        <div>
          <h1>传输</h1>
          <p>所有任务来自 daemon 实时状态。</p>
        </div>
      </section>
      {active.length === 0 && rest.length === 0 && <p className="empty panel">当前没有真实任务。</p>}
      {active.map((task) => <TransferCard key={task.task_id} task={task} onChanged={refreshTasks} />)}
      {rest.map((task) => <TransferCard key={task.task_id} task={task} onChanged={refreshTasks} />)}
    </div>
  );
}
