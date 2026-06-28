import React from 'react';
import { useAppState } from '../../store/appState';
import { FileDown, FileUp, CheckCircle2, XCircle, RefreshCw, X } from 'lucide-react';
import { cancelTransfer, retryTransfer } from '../../api/localClient';

export default function TransfersHistoryPanel() {
  const { tasks, history, refreshTasks } = useAppState();

  const handleCancel = async (id: string) => {
    await cancelTransfer(id);
    refreshTasks();
  };

  const handleRetry = async (id: string) => {
    await retryTransfer(id);
    refreshTasks();
  };

  return (
    <div>
      <h3 style={{marginBottom: 16, fontSize: '1.1rem'}}>近期活动</h3>
      
      {tasks.length === 0 && history.length === 0 && (
        <div style={{color: 'var(--text-muted)', textAlign: 'center', padding: '20px'}}>
          暂无活动记录
        </div>
      )}

      <div style={{display: 'flex', flexDirection: 'column', gap: 12}}>
        {/* 活跃任务优先 */}
        {tasks.map(task => {
          const isSend = task.direction === 'send';
          const progress = task.total_size > 0 ? (task.transferred_size / task.total_size) * 100 : 0;
          return (
            <div key={task.task_id} style={{
              background: 'var(--bg-elevated)', padding: '12px 16px', borderRadius: '8px',
              border: '1px solid var(--border-color)', display: 'flex', alignItems: 'center', justifyContent: 'space-between'
            }}>
              <div style={{display: 'flex', alignItems: 'center', gap: 12}}>
                <div style={{color: isSend ? 'var(--accent-primary)' : 'var(--accent-success)'}}>
                  {isSend ? <FileUp size={20} /> : <FileDown size={20} />}
                </div>
                <div>
                  <div style={{fontWeight: 500}}>{task.root_name || '未知文件'}</div>
                  <div style={{fontSize: '0.8rem', color: 'var(--text-secondary)'}}>
                    {task.status === 'transferring' ? '传输中' : 
                     task.status === 'queued' ? '排队中' : 
                     task.status === 'failed' ? '失败' : task.status} • {progress.toFixed(1)}%
                  </div>
                </div>
              </div>
              <div style={{display: 'flex', gap: 8}}>
                {task.status === 'failed' && (
                  <button className="icon-btn" onClick={() => handleRetry(task.task_id)} title="重试"><RefreshCw size={16}/></button>
                )}
                {(task.status === 'transferring' || task.status === 'queued') && (
                  <button className="icon-btn" onClick={() => handleCancel(task.task_id)} title="取消"><X size={16}/></button>
                )}
              </div>
            </div>
          );
        })}

        {/* 历史记录 */}
        {history.slice(0, 5).map(item => {
          const isSend = item.direction === 'send';
          return (
            <div key={item.task_id} style={{
              background: 'transparent', padding: '8px 16px', display: 'flex', alignItems: 'center', gap: 12, opacity: 0.8
            }}>
              <div style={{color: item.status === 'completed' ? 'var(--accent-success)' : 'var(--accent-danger)'}}>
                {item.status === 'completed' ? <CheckCircle2 size={18} /> : <XCircle size={18} />}
              </div>
              <div>
                <div style={{fontWeight: 500, fontSize: '0.95rem'}}>{item.root_name || '未知文件'}</div>
                <div style={{fontSize: '0.8rem', color: 'var(--text-muted)'}}>
                  {isSend ? '发送' : '接收'} • {(item.total_size/1024/1024).toFixed(1)} MB
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
