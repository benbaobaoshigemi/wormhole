import React, { useEffect, useState } from 'react';
import { useAppState } from '../store/appState';
import { FileDown, X } from 'lucide-react';
import { cancelTransfer } from '../api/localClient';

export default function TransferOverlay() {
  const { tasks, refreshTasks } = useAppState();
  const [activeReceive, setActiveReceive] = useState<any | null>(null);

  useEffect(() => {
    // Find the most recent active receiving task
    const receiving = tasks.find(t => t.direction === 'receive' && (t.status === 'transferring' || t.status === 'queued'));
    
    // Auto-dismiss logic if completed/failed, but for simplicity we just hide it when not active
    setActiveReceive(receiving || null);
  }, [tasks]);

  if (!activeReceive) return null;

  const progress = activeReceive.total_size > 0 ? (activeReceive.transferred_size / activeReceive.total_size) * 100 : 0;

  const handleCancel = async () => {
    await cancelTransfer(activeReceive.task_id);
    refreshTasks();
  };

  return (
    <div style={{
      position: 'fixed',
      bottom: 40,
      right: 40,
      width: 380,
      background: 'var(--glass-bg)',
      backdropFilter: 'blur(16px)',
      border: '1px solid var(--glass-border)',
      borderRadius: 'var(--border-radius)',
      padding: '24px',
      boxShadow: 'var(--shadow-lg)',
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
            background: 'rgba(16, 185, 129, 0.1)', color: 'var(--accent-success)',
            display: 'flex', alignItems: 'center', justifyContent: 'center'
          }}>
            <FileDown size={20} />
          </div>
          <div>
            <h3 style={{fontSize: '1.05rem', margin: 0}}>Receiving Files</h3>
            <p className="page-subtitle" style={{margin: 0, fontSize: '0.85rem'}}>From {activeReceive.peer_device_id?.substring(0,8) || 'Peer'}</p>
          </div>
        </div>
        <button onClick={handleCancel} style={{color: 'var(--text-muted)'}}><X size={20} /></button>
      </div>

      <div style={{background: 'var(--bg-base)', padding: '12px', borderRadius: 'var(--border-radius-sm)', marginBottom: 16}}>
        <div style={{fontWeight: 600, fontSize: '0.95rem', marginBottom: 4, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis'}}>
          {activeReceive.root_name || 'Unknown item'}
        </div>
        <div style={{display: 'flex', justifyContent: 'space-between', fontSize: '0.8rem', color: 'var(--text-secondary)'}}>
          <span>{activeReceive.item_count} items</span>
          <span>{(activeReceive.transferred_size / 1024 / 1024).toFixed(1)} / {(activeReceive.total_size / 1024 / 1024).toFixed(1)} MB</span>
        </div>
      </div>

      <div style={{display: 'flex', justifyContent: 'space-between', fontSize: '0.85rem', marginBottom: 8}}>
        <span style={{color: 'var(--text-secondary)', textTransform: 'capitalize'}}>{activeReceive.status}...</span>
        <span>{progress.toFixed(1)}%</span>
      </div>
      
      <div className="progress-bg" style={{marginTop: 0}}>
        <div className="progress-fill" style={{width: `${progress}%`, background: 'var(--accent-success)'}}></div>
      </div>
    </div>
  );
}
