import React, { useState, useEffect } from 'react';
import { useAppState } from '../../store/appState';
import { sendTransfer } from '../../api/localClient';
import { requestFilePath } from '../../platform/filePathProvider';
import { listen } from '@tauri-apps/api/event';
import { UploadCloud, CheckCircle2, XCircle, HardDrive } from 'lucide-react';
import './EdgeDropZone.css';

type DropState = 'idle' | 'near-edge' | 'hovering' | 'queued' | 'transferring' | 'completed' | 'failed' | 'offline';

export default function EdgeDropZone() {
  const { connectionStatus } = useAppState();
  const [dropState, setDropState] = useState<DropState>('idle');
  const [errorMessage, setErrorMessage] = useState('');

  useEffect(() => {
    if (connectionStatus !== 'connected' && dropState !== 'offline') {
      setDropState('offline');
    } else if (connectionStatus === 'connected' && dropState === 'offline') {
      setDropState('idle');
    }
  }, [connectionStatus]);

  useEffect(() => {
    if (dropState === 'offline') return;

    // Browser native drag to trigger UI
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

    // Tauri native drop event for real absolute paths
    const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;
    let unlistenTauriDrop: any;
    
    if (isTauri) {
      listen('tauri://drag-drop', async (event: any) => {
        // payload in v2 is usually { paths: string[], position: {x,y} }
        const paths = Array.isArray(event.payload) ? event.payload : event.payload?.paths;
        if (paths && paths.length > 0) {
          setDropState('queued');
          try {
            await sendTransfer(paths);
            setDropState('transferring');
            setTimeout(() => {
              setDropState('completed');
              setTimeout(() => setDropState('idle'), 2000);
            }, 1500);
          } catch (err: any) {
            setErrorMessage(err.message || '发送失败');
            setDropState('failed');
            setTimeout(() => setDropState('idle'), 3000);
          }
        }
      }).then(unlisten => {
        unlistenTauriDrop = unlisten;
      });
    }

    return () => {
      window.removeEventListener('dragover', handleDragOver);
      window.removeEventListener('dragleave', handleDragLeave);
      if (unlistenTauriDrop) unlistenTauriDrop();
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

  // This handles the browser fallback if Tauri drop event didn't fire (or developer manual path)
  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    if (dropState === 'offline') return;
    
    const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;
    if (isTauri) {
      // If we're in Tauri, the tauri://drag-drop event will handle this.
      // We don't want to double trigger.
      return;
    }

    setDropState('queued');
    
    const paths = await requestFilePath();
    
    if (!paths || paths.length === 0) {
      setErrorMessage('已取消或路径无效');
      setDropState('failed');
      setTimeout(() => setDropState('idle'), 3000);
      return;
    }

    try {
      await sendTransfer(paths);
      setDropState('transferring');
      setTimeout(() => {
        setDropState('completed');
        setTimeout(() => setDropState('idle'), 2000);
      }, 1500);

    } catch (err: any) {
      console.error(err);
      setErrorMessage(err.message || '发送失败');
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
            <h3 style={{marginBottom: 8}}>松开发送</h3>
            <p className="page-subtitle" style={{color: 'var(--text-secondary)'}}>将文件投递至对端设备</p>
          </>
        )}

        {dropState === 'queued' && (
          <>
            <div className="drop-icon-container" style={{animation: 'none', background: 'rgba(245, 158, 11, 0.1)', color: 'var(--accent-warning)'}}>
              <HardDrive size={32} />
            </div>
            <h3 style={{marginBottom: 8}}>解析路径...</h3>
          </>
        )}

        {dropState === 'transferring' && (
          <>
            <div className="drop-icon-container" style={{animation: 'none', background: 'rgba(59, 130, 246, 0.1)', color: 'var(--accent-primary)'}}>
              <UploadCloud size={32} />
            </div>
            <h3 style={{marginBottom: 8}}>正在发送</h3>
            <p className="page-subtitle" style={{marginTop: 12, color: 'var(--text-secondary)'}}>请查看传输面板</p>
          </>
        )}

        {dropState === 'completed' && (
          <>
            <div className="drop-icon-container" style={{animation: 'none', background: 'rgba(16, 185, 129, 0.1)', color: 'var(--accent-success)', boxShadow: '0 0 20px rgba(16, 185, 129, 0.4)'}}>
              <CheckCircle2 size={32} />
            </div>
            <h3 style={{marginBottom: 8}}>发送成功!</h3>
          </>
        )}

        {dropState === 'failed' && (
          <>
            <div className="drop-icon-container" style={{animation: 'none', background: 'rgba(239, 68, 68, 0.1)', color: 'var(--accent-danger)'}}>
              <XCircle size={32} />
            </div>
            <h3 style={{marginBottom: 8}}>发送失败</h3>
            <p className="page-subtitle" style={{fontSize: '0.8rem', color: 'var(--text-secondary)'}}>{errorMessage}</p>
          </>
        )}
      </div>
    </div>
  );
}
