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
    // Check local storage for onboarding state
    const isComplete = localStorage.getItem('wormhole_onboarding_complete') === 'true';
    setOnboardingComplete(isComplete);
  }, []);

  if (daemonStatus === 'loading') {
    return (
      <div className="shell-loading">
        <RefreshCw className="spinner" size={32} />
        <div>Connecting to Wormhole Service...</div>
      </div>
    );
  }

  if (daemonStatus === 'error') {
    return (
      <div className="shell-error">
        <h2>Daemon Unreachable</h2>
        <p>Could not connect to the local Wormhole service.</p>
        <button className="btn-primary" onClick={() => window.location.reload()}>Retry Connection</button>
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
            <span className="my-device">{settings?.device_name || 'My PC'}</span>
            {connectionStatus === 'connected' && (
              <>
                <span className="device-separator">→</span>
                <span className="peer-device">{peer?.device_name || 'Peer'}</span>
              </>
            )}
            {connectionStatus !== 'connected' && (
              <span className="peer-device muted">Waiting for connection...</span>
            )}
          </div>
        </div>
        <div className="header-right">
          <button className="icon-btn" onClick={() => setShowSettings(!showSettings)}>
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
