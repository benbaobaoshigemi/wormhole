import React from 'react';
import { useAppState } from '../store/appState';
import { clearHistory } from '../api/localClient';
import { Trash2, FileText, CheckCircle2, XCircle } from 'lucide-react';

export default function History() {
  const { history, refreshHistory } = useAppState();

  const handleClear = async () => {
    await clearHistory();
    refreshHistory();
  };

  return (
    <>
      <div className="page-header" style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center'}}>
        <div>
          <h1 className="page-title">History</h1>
          <p className="page-subtitle">Past file transfers</p>
        </div>
        {history.length > 0 && (
          <button className="btn-secondary" onClick={handleClear}>
            <Trash2 size={16} /> Clear History
          </button>
        )}
      </div>
      
      <div className="page-body">
        {history.length === 0 ? (
          <div className="card" style={{textAlign: 'center', padding: '60px 20px'}}>
            <FileText size={48} color="var(--text-muted)" style={{margin: '0 auto 16px'}} />
            <h3>No History</h3>
            <p className="page-subtitle">Your past transfers will appear here.</p>
          </div>
        ) : (
          <div style={{display: 'flex', flexDirection: 'column', gap: 12}}>
            {history.map((item: any) => (
              <div key={item.task_id} className="card" style={{padding: '16px', display: 'flex', alignItems: 'center', justifyContent: 'space-between'}}>
                <div style={{display: 'flex', alignItems: 'center', gap: 16}}>
                  {item.status === 'completed' ? (
                    <CheckCircle2 size={24} color="var(--accent-success)" />
                  ) : (
                    <XCircle size={24} color="var(--accent-danger)" />
                  )}
                  <div>
                    <div style={{fontWeight: 600}}>{item.root_name || 'Unknown Item'}</div>
                    <div className="page-subtitle" style={{fontSize: '0.85rem', display: 'flex', gap: 12}}>
                      <span>{item.direction === 'send' ? 'Sent' : 'Received'}</span>
                      <span>{(item.total_size / 1024 / 1024).toFixed(2)} MB</span>
                      <span>{new Date(item.updated_at).toLocaleString()}</span>
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </>
  );
}
