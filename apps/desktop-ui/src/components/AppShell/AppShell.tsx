import React, { useState, useEffect } from 'react';
import { useAppState } from '../../store/appState';
import { Settings, RefreshCw } from 'lucide-react';
import WormholeStage from '../WormholeStage/WormholeStage';
import PreferencePanel from '../PreferencePanel/PreferencePanel';
import OnboardingFlow from '../OnboardingFlow/OnboardingFlow';
import EdgeDropZone from '../EdgeDropZone';
import TransferOverlay from '../TransferOverlay';
import './AppShell.css';

export default function AppShell() {
  const { daemonStatus, connectionStatus, settings, peer } = useAppState();
  const [showSettings, setShowSettings] = useState(false);
  const [onboardingComplete, setOnboardingComplete] = useState(false);

  useEffect(() => {
    const isComplete = localStorage.getItem('wormhole_onboarding_complete') === 'true';
    setOnboardingComplete(isComplete);
  }, []);

  if (daemonStatus === 'loading') {
    return (
      <div className="shell-loading">
        <RefreshCw className="spinner" size={32} />
        <div>正在连接至 Wormhole 核心服务...</div>
      </div>
    );
  }

  if (daemonStatus === 'error') {
    return (
      <div className="shell-error">
        <h2>核心服务无法访问</h2>
        <p>无法连接本地的 Wormhole 守护进程，请确保后端已启动。</p>
        <button className="btn-primary" onClick={() => window.location.reload()}>重试连接</button>
      </div>
    );
  }

  if (!onboardingComplete) {
    return <OnboardingFlow onComplete={() => setOnboardingComplete(true)} />;
  }

  return (
    <div className="app-shell">
      <header className="shell-header">
        <div className="header-left">
          <div className={`status-dot status-${connectionStatus}`}></div>
          <div className="device-names">
            <span className="my-device">{settings?.device_name || '我的电脑'}</span>
            {connectionStatus === 'connected' && (
              <>
                <span className="device-separator">→</span>
                <span className="peer-device">{peer?.device_name || '对端设备'}</span>
              </>
            )}
            {connectionStatus !== 'connected' && (
              <span className="peer-device muted">等待对方连接...</span>
            )}
          </div>
        </div>
        <div className="header-right">
          <button className="icon-btn" onClick={() => setShowSettings(!showSettings)} title="偏好设置">
            <Settings size={18} />
          </button>
        </div>
      </header>

      <main className="shell-main">
        <WormholeStage />
      </main>

      {showSettings && (
        <PreferencePanel onClose={() => setShowSettings(false)} />
      )}

      <EdgeDropZone />
      <TransferOverlay />
    </div>
  );
}
