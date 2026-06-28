import React, { useState } from 'react';
import { useAppState } from '../../store/appState';
import { Laptop, Cpu, Send, ClipboardPaste, Clock, FolderOpen } from 'lucide-react';
import { requestFilePath } from '../../platform/filePathProvider';
import { sendTransfer } from '../../api/localClient';
import TransfersHistoryPanel from './TransfersHistoryPanel';
import './WormholeStage.css';

export default function WormholeStage() {
  const { connectionStatus, settings, peer, clipboard } = useAppState();
  const [showHistory, setShowHistory] = useState(false);

  const handleSendFile = async () => {
    const paths = await requestFilePath();
    if (paths && paths.length > 0) {
      await sendTransfer(paths);
    }
  };

  const openReceiveDir = async () => {
    // Attempt to open the receive directory if we have a way to do it natively
    // Without Tauri, we can just alert
    alert(`Receive Directory:\n${settings?.receive_dir || 'Not configured'}`);
  };

  return (
    <div className="wormhole-stage">
      <div className="stage-devices">
        
        {/* Local Device */}
        <div className="device-card local">
          <div className="device-icon">
            <Laptop size={48} />
          </div>
          <div className="device-info">
            <h3>{settings?.device_name || 'My PC'}</h3>
            <p>This Device</p>
          </div>
        </div>

        {/* The Wormhole Connection */}
        <div className={`connection-bridge ${connectionStatus}`}>
          <div className="bridge-line">
            <div className="bridge-glow"></div>
          </div>
          <div className="bridge-status">
            {connectionStatus === 'connected' ? (
              <span className="status-badge connected">Connected</span>
            ) : (
              <span className="status-badge disconnected">Waiting for Peer</span>
            )}
          </div>
          
          {connectionStatus === 'connected' && (
            <div className="bridge-services">
              <div className="service-indicator" title="Clipboard Text Sync">
                <ClipboardPaste size={14} color={settings?.clipboard?.text_enabled ? 'var(--accent-success)' : 'var(--text-muted)'} />
              </div>
              <div className="service-indicator" title="Clipboard Image Sync">
                <ClipboardPaste size={14} color={settings?.clipboard?.image_enabled ? 'var(--accent-success)' : 'var(--text-muted)'} />
              </div>
            </div>
          )}
        </div>

        {/* Remote Device */}
        <div className={`device-card remote ${connectionStatus !== 'connected' ? 'offline' : ''}`}>
          <div className="device-icon">
            {connectionStatus === 'connected' ? <Laptop size={48} /> : <Cpu size={48} />}
          </div>
          <div className="device-info">
            <h3>{peer?.device_name || 'No Peer'}</h3>
            <p>{peer ? `${peer.platform} • ${peer.port}` : 'Offline'}</p>
          </div>
        </div>

      </div>

      <div className="stage-actions">
        <button className="action-card primary" onClick={handleSendFile} disabled={connectionStatus !== 'connected'}>
          <Send size={24} />
          <span>Send Files</span>
        </button>
        <button className="action-card secondary" onClick={openReceiveDir}>
          <FolderOpen size={24} />
          <span>Received Files</span>
        </button>
        <button className="action-card secondary" onClick={() => setShowHistory(!showHistory)}>
          <Clock size={24} />
          <span>{showHistory ? 'Hide History' : 'View History'}</span>
        </button>
      </div>

      {showHistory && (
        <div className="stage-history-container">
          <TransfersHistoryPanel />
        </div>
      )}
    </div>
  );
}
