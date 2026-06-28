import React, { useState } from 'react';
import { useAppState } from '../../store/appState';
import { Laptop, Cpu, Send, ClipboardPaste, Clock, FolderOpen } from 'lucide-react';
import { requestFilePath, requestDirectoryPath } from '../../platform/filePathProvider';
import { sendTransfer } from '../../api/localClient';
import TransfersHistoryPanel from './TransfersHistoryPanel';
import './WormholeStage.css';

export default function WormholeStage() {
  const { connectionStatus, settings, peer } = useAppState();
  const [showHistory, setShowHistory] = useState(false);

  const handleSendFile = async () => {
    const paths = await requestFilePath();
    if (paths && paths.length > 0) {
      await sendTransfer(paths);
    }
  };

  const handleSendFolder = async () => {
    const paths = await requestDirectoryPath();
    if (paths && paths.length > 0) {
      await sendTransfer(paths);
    }
  };

  return (
    <div className="wormhole-stage">
      <div className="stage-devices">
        
        {/* 本地设备 */}
        <div className="device-card local">
          <div className="device-icon">
            <Laptop size={48} />
          </div>
          <div className="device-info">
            <h3>{settings?.device_name || '我的电脑'}</h3>
            <p>本机</p>
          </div>
        </div>

        {/* 虫洞连接桥 */}
        <div className={`connection-bridge ${connectionStatus}`}>
          <div className="bridge-line">
            <div className="bridge-glow"></div>
          </div>
          <div className="bridge-status">
            {connectionStatus === 'connected' ? (
              <span className="status-badge connected">已连接</span>
            ) : (
              <span className="status-badge disconnected">等待配对</span>
            )}
          </div>
          
          {connectionStatus === 'connected' && (
            <div className="bridge-services">
              <div className="service-indicator" title="文本剪贴板同步">
                <ClipboardPaste size={14} color={settings?.clipboard?.text_enabled ? 'var(--accent-success)' : 'var(--text-muted)'} />
              </div>
              <div className="service-indicator" title="图片剪贴板同步">
                <ClipboardPaste size={14} color={settings?.clipboard?.image_enabled ? 'var(--accent-success)' : 'var(--text-muted)'} />
              </div>
            </div>
          )}
        </div>

        {/* 远程设备 */}
        <div className={`device-card remote ${connectionStatus !== 'connected' ? 'offline' : ''}`}>
          <div className="device-icon">
            {connectionStatus === 'connected' ? <Laptop size={48} /> : <Cpu size={48} />}
          </div>
          <div className="device-info">
            <h3>{peer?.device_name || '无对端设备'}</h3>
            <p>{peer ? `${peer.platform} • ${peer.port}` : '离线'}</p>
          </div>
        </div>

      </div>

      <div className="stage-actions">
        <button className="action-card primary" onClick={handleSendFile} disabled={connectionStatus !== 'connected'}>
          <Send size={24} />
          <span>发送文件</span>
        </button>
        <button className="action-card primary" onClick={handleSendFolder} disabled={connectionStatus !== 'connected'}>
          <FolderOpen size={24} />
          <span>发送文件夹</span>
        </button>
        <button className="action-card secondary" onClick={() => setShowHistory(!showHistory)}>
          <Clock size={24} />
          <span>{showHistory ? '隐藏传输记录' : '查看传输记录'}</span>
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
