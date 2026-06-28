import React from 'react';
import { useAppState } from '../store/appState';
import { cancelTransfer, retryTransfer } from '../api/localClient';
import { Play, Square, RefreshCw, FileText } from 'lucide-react';

export default function Transfers() {
  const { tasks, refreshTasks } = useAppState();

  const handleCancel = async (id: string) => {
    await cancelTransfer(id);
    refreshTasks();
  };

  const handleRetry = async (id: string) => {
    await retryTransfer(id);
    refreshTasks();
  };

  return (
    <>
      <div className="page-header">
        <h1 className="page-title">Transfers</h1>
        <p className="page-subtitle">Manage active and queued file transfers</p>
      </div>
      
      <div className="page-body">
        {tasks.length === 0 ? (
          <div className="card" style={{textAlign: 'center', padding: '60px 20px'}}>
            <FileText size={48} color="var(--text-muted)" style={{margin: '0 auto 16px'}} />
            <h3>No Active Transfers</h3>
            <p className="page-subtitle">Drag and drop files to the edge of your screen to send.</p>
          </div>
        ) : (
          <div style={{display: 'flex', flexDirection: 'column', gap: 16}}>
            {tasks.map((task: any) => {
              const progress = task.total_size > 0 ? (task.transferred_size / task.total_size) * 100 : 0;
              const isSending = task.direction === 'send';
              
              return (
                <div key={task.task_id} className="card" style={{padding: '20px'}}>
                  <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12}}>
                    <div style={{display: 'flex', alignItems: 'center', gap: 12}}>
                      <div style={{
                        width: 40, height: 40, borderRadius: 8, 
                        background: isSending ? 'rgba(59, 130, 246, 0.1)' : 'rgba(16, 185, 129, 0.1)',
                        color: isSending ? 'var(--accent-primary)' : 'var(--accent-success)',
                        display: 'flex', alignItems: 'center', justifyContent: 'center'
                      }}>
                        <FileText size={20} />
                      </div>
                      <div>
                        <div style={{fontWeight: 600}}>{task.root_name || 'Unknown'}</div>
                        <div className="page-subtitle" style={{fontSize: '0.85rem'}}>
                          {isSending ? 'Sending to peer' : 'Receiving from peer'} • {(task.total_size / 1024 / 1024).toFixed(2)} MB
                        </div>
                      </div>
                    </div>
                    
                    <div style={{display: 'flex', gap: 8}}>
                      {task.status === 'failed' && (
                        <button className="btn-secondary" onClick={() => handleRetry(task.task_id)}>
                          <RefreshCw size={16} /> Retry
                        </button>
                      )}
                      {(task.status === 'transferring' || task.status === 'queued') && (
                        <button className="btn-secondary" onClick={() => handleCancel(task.task_id)}>
                          <Square size={16} /> Cancel
                        </button>
                      )}
                    </div>
                  </div>

                  <div style={{display: 'flex', justifyContent: 'space-between', fontSize: '0.85rem', marginBottom: 8}}>
                    <span style={{textTransform: 'capitalize', color: task.status === 'failed' ? 'var(--accent-danger)' : 'var(--text-secondary)'}}>
                      {task.status} {task.error ? `- ${task.error}` : ''}
                    </span>
                    <span>{progress.toFixed(1)}%</span>
                  </div>
                  
                  <div className="progress-bg">
                    <div 
                      className="progress-fill" 
                      style={{
                        width: `${progress}%`,
                        background: task.status === 'failed' ? 'var(--accent-danger)' : 'var(--accent-primary)'
                      }} 
                    />
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </>
  );
}
