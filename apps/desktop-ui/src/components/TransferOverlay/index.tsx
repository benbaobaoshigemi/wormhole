import React, { useEffect, useState } from 'react';
import { useAppState } from '../../store/appState';
import { FileDown, FileUp, X } from 'lucide-react';
import { cancelTransfer } from '../../api/localClient';

export default function TransferOverlay() {
  const { tasks, refreshTasks } = useAppState();
  const [activeTransfer, setActiveTransfer] = useState<any | null>(null);

  useEffect(() => {
    // 寻找最近的活跃任务
    const active = tasks.find(t => t.status === 'transferring' || t.status === 'queued');
    setActiveTransfer(active || null);
  }, [tasks]);

  if (!activeTransfer) return null;

  const isSend = activeTransfer.direction === 'send';
  const progress = activeTransfer.total_size > 0 ? (activeTransfer.transferred_size / activeTransfer.total_size) * 100 : 0;

  const handleCancel = async () => {
    await cancelTransfer(activeTransfer.task_id);
    refreshTasks();
  };

  const getStatusText = (status: string) => {
    switch(status) {
      case 'queued': return '排队中...';
      case 'transferring': return '传输中...';
      case 'completed': return '已完成';
      case 'failed': return '失败';
      default: return status;
    }
  };

  return (
    <div className="glass-panel" style={{
      position: 'fixed',
      bottom: 24,
      right: 24,
      width: 360,
      borderRadius: 'var(--border-radius)',
      padding: '20px',
      zIndex: 1000,
      animation: 'slideUp 0.3s cubic-bezier(0.4, 0, 0.2, 1)'
    }}>
      <style>{`
        @keyframes slideUp {
          from { opacity: 0; transform: translateY(20px); }
          to { opacity: 1; transform: translateY(0); }
        }
      `}</style>

      <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 16}}>
        <div style={{display: 'flex', gap: 12, alignItems: 'center'}}>
          <div style={{
            width: 40, height: 40, borderRadius: '50%', 
            background: isSend ? 'rgba(59, 130, 246, 0.1)' : 'rgba(16, 185, 129, 0.1)', 
            color: isSend ? 'var(--accent-primary)' : 'var(--accent-success)',
            display: 'flex', alignItems: 'center', justifyContent: 'center'
          }}>
            {isSend ? <FileUp size={20} /> : <FileDown size={20} />}
          </div>
          <div>
            <h3 style={{fontSize: '1.05rem', margin: 0, fontWeight: 600}}>
              {isSend ? '正在发送' : '正在接收'}
            </h3>
            <p className="page-subtitle" style={{margin: 0, fontSize: '0.85rem', color: 'var(--text-secondary)'}}>
              {isSend ? '发往' : '来自'} {activeTransfer.peer_device_id?.substring(0,8) || '对端设备'}
            </p>
          </div>
        </div>
        <button className="icon-btn" onClick={handleCancel} title="取消"><X size={18} /></button>
      </div>

      <div style={{background: 'var(--bg-base)', padding: '12px', borderRadius: 'var(--border-radius-sm)', marginBottom: 16}}>
        <div style={{fontWeight: 500, fontSize: '0.9rem', marginBottom: 4, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis'}}>
          {activeTransfer.root_name || '未知项'}
        </div>
        <div style={{display: 'flex', justifyContent: 'space-between', fontSize: '0.8rem', color: 'var(--text-secondary)'}}>
          <span>{activeTransfer.item_count} 个项目</span>
          <span>{(activeTransfer.transferred_size / 1024 / 1024).toFixed(1)} / {(activeTransfer.total_size / 1024 / 1024).toFixed(1)} MB</span>
        </div>
      </div>

      <div style={{display: 'flex', justifyContent: 'space-between', fontSize: '0.85rem', marginBottom: 8}}>
        <span style={{color: 'var(--text-secondary)'}}>{getStatusText(activeTransfer.status)}</span>
        <span>{progress.toFixed(1)}%</span>
      </div>
      
      <div style={{
        height: 6,
        background: 'rgba(255, 255, 255, 0.1)',
        borderRadius: 3,
        overflow: 'hidden'
      }}>
        <div style={{
          height: '100%',
          width: `${progress}%`, 
          background: isSend ? 'var(--accent-primary)' : 'var(--accent-success)',
          transition: 'width 0.3s ease',
          boxShadow: isSend ? '0 0 10px var(--accent-primary-glow)' : '0 0 10px rgba(16,185,129,0.4)'
        }}></div>
      </div>
    </div>
  );
}
