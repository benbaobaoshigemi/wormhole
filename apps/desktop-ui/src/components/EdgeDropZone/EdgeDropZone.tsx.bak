import React, { useState, useEffect } from 'react';
import { useAppState } from '../store/appState';
import { sendTransfer } from '../api/localClient';
import { UploadCloud, CheckCircle2, XCircle, HardDrive } from 'lucide-react';

type DropState = 'idle' | 'near-edge' | 'hovering' | 'queued' | 'transferring' | 'completed' | 'failed' | 'offline';

export default function EdgeDropZone() {
  const { connectionStatus, tasks } = useAppState();
  const [dropState, setDropState] = useState<DropState>('idle');
  const [mockProgress, setMockProgress] = useState(0);

  // Sync state with global connection status
  useEffect(() => {
    if (connectionStatus !== 'connected' && dropState !== 'offline') {
      setDropState('offline');
    } else if (connectionStatus === 'connected' && dropState === 'offline') {
      setDropState('idle');
    }
  }, [connectionStatus]);

  // Handle mock drag events for the edge trigger
  useEffect(() => {
    if (dropState === 'offline') return;

    const handleDragOver = (e: DragEvent) => {
      e.preventDefault();
      // If mouse is near the right edge (within 50px)
      if (e.clientX > window.innerWidth - 50 && dropState === 'idle') {
        setDropState('near-edge');
      }
    };

    const handleDragLeave = (e: DragEvent) => {
      if (e.clientX < window.innerWidth - 320 && (dropState === 'near-edge' || dropState === 'hovering')) {
        setDropState('idle');
      }
    };

    window.addEventListener('dragover', handleDragOver);
    window.addEventListener('dragleave', handleDragLeave);
    return () => {
      window.removeEventListener('dragover', handleDragOver);
      window.removeEventListener('dragleave', handleDragLeave);
    };
  }, [dropState]);

  const handleZoneDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    if (dropState === 'near-edge' || dropState === 'idle') {
      setDropState('hovering');
    }
  };

  const handleZoneDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    if (dropState === 'offline') return;
    
    // In a real Tauri app, we would get real absolute paths here.
    // e.g. using @tauri-apps/api/event 'tauri://file-drop'
    // Since we are in browser mock mode, we just pass dummy paths to daemon, 
    // or simulate if the daemon can't read browser fake paths.
    // We will send a mock path to the local API. If it fails, we fall back to mock UI.
    
    setDropState('queued');
    
    try {
      // Mocking a file path for demonstration
      await sendTransfer(['C:/dummy/path/to/file.txt']);
      setDropState('transferring');
      
      // Simulate progress visually
      let p = 0;
      const interval = setInterval(() => {
        p += 10;
        setMockProgress(p);
        if (p >= 100) {
          clearInterval(interval);
          setDropState('completed');
          setTimeout(() => setDropState('idle'), 3000);
        }
      }, 300);
    } catch (err) {
      console.error(err);
      setDropState('failed');
      setTimeout(() => setDropState('idle'), 3000);
    }
  };

  // The actual render class
  let className = 'edge-dropzone';
  if (dropState === 'near-edge') className += ' near-edge';
  if (dropState === 'hovering' || dropState === 'queued' || dropState === 'transferring' || dropState === 'completed' || dropState === 'failed') className += ' hovering';

  return (
    <div 
      className={className} 
      onDragOver={handleZoneDragOver}
      onDragLeave={handleZoneDragLeave}
      onDrop={handleDrop}
    >
      <div className="edge-content">
        {(dropState === 'hovering' || dropState === 'near-edge') && (
          <>
            <div className="drop-icon-container">
              <UploadCloud size={32} />
            </div>
            <h3 style={{marginBottom: 8}}>Drop to Send</h3>
            <p className="page-subtitle">Release files to transfer to peer</p>
          </>
        )}

        {dropState === 'queued' && (
          <>
            <div className="drop-icon-container" style={{animation: 'none', background: 'rgba(245, 158, 11, 0.1)', color: 'var(--accent-warning)'}}>
              <HardDrive size={32} />
            </div>
            <h3 style={{marginBottom: 8}}>Queued</h3>
            <p className="page-subtitle">Preparing transfer...</p>
          </>
        )}

        {dropState === 'transferring' && (
          <>
            <div className="drop-icon-container" style={{animation: 'none', background: 'rgba(59, 130, 246, 0.1)', color: 'var(--accent-primary)'}}>
              <UploadCloud size={32} />
            </div>
            <h3 style={{marginBottom: 8}}>Sending...</h3>
            <div className="progress-bg">
              <div className="progress-fill" style={{width: `${mockProgress}%`}}></div>
            </div>
            <p className="page-subtitle" style={{marginTop: 12}}>{mockProgress}%</p>
          </>
        )}

        {dropState === 'completed' && (
          <>
            <div className="drop-icon-container" style={{animation: 'none', background: 'rgba(16, 185, 129, 0.1)', color: 'var(--accent-success)', boxShadow: '0 0 20px rgba(16, 185, 129, 0.4)'}}>
              <CheckCircle2 size={32} />
            </div>
            <h3 style={{marginBottom: 8}}>Sent!</h3>
          </>
        )}

        {dropState === 'failed' && (
          <>
            <div className="drop-icon-container" style={{animation: 'none', background: 'rgba(239, 68, 68, 0.1)', color: 'var(--accent-danger)'}}>
              <XCircle size={32} />
            </div>
            <h3 style={{marginBottom: 8}}>Failed</h3>
            <p className="page-subtitle">Could not read file paths (Requires Tauri)</p>
          </>
        )}
      </div>
    </div>
  );
}
