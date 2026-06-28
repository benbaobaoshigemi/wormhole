import React from 'react';
import { useAppState } from '../store/appState';
import { enableClipboard, disableClipboard } from '../api/localClient';
import { Clipboard as ClipIcon, Type, Image as ImageIcon, ToggleLeft, ToggleRight } from 'lucide-react';

export default function Clipboard() {
  const { settings, clipboard, refreshSettings } = useAppState();

  const handleToggleText = async () => {
    if (settings?.clipboard?.text_enabled) {
      await disableClipboard('text');
    } else {
      await enableClipboard('text');
    }
    refreshSettings();
  };

  const handleToggleImage = async () => {
    if (settings?.clipboard?.image_enabled) {
      await disableClipboard('image');
    } else {
      await enableClipboard('image');
    }
    refreshSettings();
  };

  return (
    <>
      <div className="page-header">
        <h1 className="page-title">Clipboard</h1>
        <p className="page-subtitle">Sync text and images across devices seamlessly</p>
      </div>
      
      <div className="page-body">
        <div className="card" style={{display: 'flex', flexDirection: 'column', gap: 24}}>
          
          <div style={{display: 'flex', alignItems: 'center', justifyContent: 'space-between', paddingBottom: 24, borderBottom: '1px solid var(--border-color)'}}>
            <div style={{display: 'flex', alignItems: 'center', gap: 16}}>
              <div style={{padding: 12, background: 'rgba(59, 130, 246, 0.1)', borderRadius: 12, color: 'var(--accent-primary)'}}>
                <Type size={24} />
              </div>
              <div>
                <div style={{fontWeight: 600, fontSize: '1.1rem'}}>Text Synchronization</div>
                <div className="page-subtitle">Automatically sync copied text to your peer</div>
                {clipboard.syncedTextHash && (
                  <div style={{fontSize: '0.8rem', color: 'var(--accent-success)', marginTop: 4}}>
                    Last synced: {clipboard.syncedTextHash.substring(0, 16)}...
                  </div>
                )}
              </div>
            </div>
            <button onClick={handleToggleText} style={{color: settings?.clipboard?.text_enabled ? 'var(--accent-success)' : 'var(--text-muted)'}}>
              {settings?.clipboard?.text_enabled ? <ToggleRight size={40} /> : <ToggleLeft size={40} />}
            </button>
          </div>

          <div style={{display: 'flex', alignItems: 'center', justifyContent: 'space-between'}}>
            <div style={{display: 'flex', alignItems: 'center', gap: 16}}>
              <div style={{padding: 12, background: 'rgba(59, 130, 246, 0.1)', borderRadius: 12, color: 'var(--accent-primary)'}}>
                <ImageIcon size={24} />
              </div>
              <div>
                <div style={{fontWeight: 600, fontSize: '1.1rem'}}>Image Synchronization</div>
                <div className="page-subtitle">Automatically sync copied images to your peer</div>
                {clipboard.syncedImageHash && (
                  <div style={{fontSize: '0.8rem', color: 'var(--accent-success)', marginTop: 4}}>
                    Last synced: {clipboard.syncedImageHash.substring(0, 16)}...
                  </div>
                )}
              </div>
            </div>
            <button onClick={handleToggleImage} style={{color: settings?.clipboard?.image_enabled ? 'var(--accent-success)' : 'var(--text-muted)'}}>
              {settings?.clipboard?.image_enabled ? <ToggleRight size={40} /> : <ToggleLeft size={40} />}
            </button>
          </div>

        </div>
      </div>
    </>
  );
}
