import React from 'react';
import { useAppState } from '../store/appState';
import { Laptop, Cpu, Send, ClipboardPaste } from 'lucide-react';

export default function Dashboard() {
  const { connectionStatus, peer, settings, clipboard, tasks } = useAppState();

  const activeTasks = tasks.filter(t => t.status !== 'completed' && t.status !== 'failed' && t.status !== 'cancelled');

  return (
    <>
      <div className="page-header">
        <h1 className="page-title">Dashboard</h1>
        <p className="page-subtitle">Overview of your Wormhole connection</p>
      </div>
      
      <div className="page-body">
        {connectionStatus !== 'connected' ? (
          <div className="card" style={{ textAlign: 'center', padding: '60px 20px' }}>
            <Cpu size={48} color="var(--text-muted)" style={{margin: '0 auto 16px'}} />
            <h3 style={{marginBottom: 8}}>Not Connected to Peer</h3>
            <p className="page-subtitle">Waiting for a peer to connect on the local network...</p>
          </div>
        ) : (
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))', gap: '24px' }}>
            
            <div className="card" style={{borderTop: '4px solid var(--accent-primary)'}}>
              <div className="card-title"><Laptop size={20} /> Connected Peer</div>
              <div style={{fontSize: '1.25rem', fontWeight: 600, marginBottom: 4}}>{peer?.device_name || 'Unknown Device'}</div>
              <div className="page-subtitle" style={{marginBottom: 16}}>{peer?.platform} • {peer?.host}:{peer?.port}</div>
              <div className="status-pill status-connected">Online</div>
            </div>

            <div className="card">
              <div className="card-title"><Send size={20} /> Active Transfers</div>
              <div style={{fontSize: '2rem', fontWeight: 700, marginBottom: 8}}>{activeTasks.length}</div>
              <p className="page-subtitle">Tasks currently in progress or queued.</p>
            </div>

            <div className="card">
              <div className="card-title"><ClipboardPaste size={20} /> Clipboard Sync</div>
              <div style={{display: 'flex', flexDirection: 'column', gap: 12, marginTop: 16}}>
                <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center'}}>
                  <span>Text Sync</span>
                  {settings?.clipboard?.text_enabled ? <span className="status-pill status-connected">Enabled</span> : <span className="status-pill status-disconnected">Disabled</span>}
                </div>
                <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center'}}>
                  <span>Image Sync</span>
                  {settings?.clipboard?.image_enabled ? <span className="status-pill status-connected">Enabled</span> : <span className="status-pill status-disconnected">Disabled</span>}
                </div>
              </div>
            </div>

          </div>
        )}
      </div>
    </>
  );
}
