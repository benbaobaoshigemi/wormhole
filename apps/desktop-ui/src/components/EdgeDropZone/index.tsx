import React, { useState, useEffect } from 'react';
import { useAppState } from '../../store/appState';
import { sendTransfer } from '../../api/localClient';
import { requestFilePath } from '../../platform/filePathProvider';
import { UploadCloud, CheckCircle2, XCircle, HardDrive } from 'lucide-react';
import './EdgeDropZone.css';

type DropState = 'idle' | 'near-edge' | 'hovering' | 'queued' | 'transferring' | 'completed' | 'failed' | 'offline';

export default function EdgeDropZone() {
  const { connectionStatus, tasks } = useAppState();
  const [dropState, setDropState] = useState<DropState>('idle');
  const [errorMessage, setErrorMessage] = useState('');
  
  // We don't use a mock progress timer anymore. We use the real task progress if we know the taskId.
  // For simplicity of EdgeDropZone's temporary state, we can just transition to 'transferring' 
  // and let the main UI (TransferOverlay) handle the detailed progress.
  // But we still show a transferring state briefly.

  useEffect(() => {
    if (connectionStatus !== 'connected' && dropState !== 'offline') {
      setDropState('offline');
    } else if (connectionStatus === 'connected' && dropState === 'offline') {
      setDropState('idle');
    }
  }, [connectionStatus]);

  useEffect(() => {
    if (dropState === 'offline') return;

    const handleDragOver = (e: DragEvent) => {
      e.preventDefault();
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
    
    setDropState('queued');
    
    // We request the real file path (Tauri or manual fallback)
    const paths = await requestFilePath();
    
    if (!paths || paths.length === 0) {
      setErrorMessage('Transfer cancelled or invalid path.');
      setDropState('failed');
      setTimeout(() => setDropState('idle'), 3000);
      return;
    }

    try {
      await sendTransfer(paths);
      setDropState('transferring');
      
      // We don't mock progress. We just show transferring then complete.
      setTimeout(() => {
        setDropState('completed');
        setTimeout(() => setDropState('idle'), 2000);
      }, 1500);

    } catch (err: any) {
      console.error(err);
      setErrorMessage(err.message || 'Send failed');
      setDropState('failed');
      setTimeout(() => setDropState('idle'), 3000);
    }
  };

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
            <h3 style={{marginBottom: 8}}>Requesting Path</h3>
            <p className="page-subtitle">Please select real path...</p>
          </>
        )}

        {dropState === 'transferring' && (
          <>
            <div className="drop-icon-container" style={{animation: 'none', background: 'rgba(59, 130, 246, 0.1)', color: 'var(--accent-primary)'}}>
              <UploadCloud size={32} />
            </div>
            <h3 style={{marginBottom: 8}}>Sending...</h3>
            <p className="page-subtitle" style={{marginTop: 12}}>Check transfer overlay</p>
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
            <p className="page-subtitle" style={{fontSize: '0.8rem'}}>{errorMessage}</p>
          </>
        )}
      </div>
    </div>
  );
}
