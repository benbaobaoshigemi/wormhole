import React, { useState } from 'react';
import { useAppState } from './store/appState';
import { LayoutDashboard, Send, Clock, ClipboardPaste, Settings, Loader2 } from 'lucide-react';
import Dashboard from './pages/Dashboard';
import Transfers from './pages/Transfers';
import History from './pages/History';
import Clipboard from './pages/Clipboard';
import SettingsPage from './pages/Settings';
import EdgeDropZone from './components/EdgeDropZone';
import TransferOverlay from './components/TransferOverlay';

function App() {
  const { daemonStatus, connectionStatus } = useAppState();
  const [currentPage, setCurrentPage] = useState('dashboard');

  if (daemonStatus === 'loading') {
    return (
      <div className="loading-screen">
        <Loader2 className="spinner" size={48} />
        <h2>Wormhole is starting...</h2>
        <p className="page-subtitle">Connecting to local daemon</p>
      </div>
    );
  }

  if (daemonStatus === 'error') {
    return (
      <div className="loading-screen">
        <div style={{color: 'var(--accent-danger)', marginBottom: 16}}>
          <Settings size={48} />
        </div>
        <h2>Connection Failed</h2>
        <p className="page-subtitle">Could not connect to the local daemon at port 53317.</p>
        <button className="btn-secondary" onClick={() => window.location.reload()}>Retry</button>
      </div>
    );
  }

  const renderPage = () => {
    switch(currentPage) {
      case 'dashboard': return <Dashboard />;
      case 'transfers': return <Transfers />;
      case 'history': return <History />;
      case 'clipboard': return <Clipboard />;
      case 'settings': return <SettingsPage />;
      default: return <Dashboard />;
    }
  };

  const navItems = [
    { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
    { id: 'transfers', label: 'Transfers', icon: Send },
    { id: 'history', label: 'History', icon: Clock },
    { id: 'clipboard', label: 'Clipboard', icon: ClipboardPaste },
    { id: 'settings', label: 'Settings', icon: Settings },
  ];

  return (
    <div className="app-container">
      <div className="sidebar">
        <div className="sidebar-logo">
          <div style={{width: 24, height: 24, borderRadius: '50%', background: 'var(--accent-primary)'}} />
          Wormhole
        </div>
        
        <div className="sidebar-nav">
          {navItems.map(item => (
            <button 
              key={item.id}
              className={`nav-item ${currentPage === item.id ? 'active' : ''}`}
              onClick={() => setCurrentPage(item.id)}
            >
              <item.icon size={18} />
              {item.label}
            </button>
          ))}
        </div>

        {/* Status indicator at the bottom of sidebar */}
        <div style={{padding: '24px', borderTop: '1px solid var(--border-color)', display: 'flex', alignItems: 'center', gap: '12px'}}>
          <div className={`status-pill status-${connectionStatus}`}>
            <span style={{width: 8, height: 8, borderRadius: '50%', backgroundColor: 'currentColor'}}></span>
            {connectionStatus}
          </div>
        </div>
      </div>

      <div className="main-content">
        {renderPage()}
      </div>

      <EdgeDropZone />
      <TransferOverlay />
    </div>
  );
}

export default App;
