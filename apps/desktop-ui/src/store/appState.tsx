import React, { createContext, useContext, useEffect, useState, ReactNode } from 'react';
import { fetchState, fetchTasks, fetchHistory, fetchSettings } from '../api/localClient';
import { connectEvents, disconnectEvents, subscribeEvent } from '../api/events';

interface GlobalState {
  daemonStatus: 'loading' | 'connected' | 'error';
  connectionStatus: 'disconnected' | 'connecting' | 'connected';
  peer: any | null;
  tasks: any[];
  history: any[];
  settings: any | null;
  clipboard: {
    syncedTextHash: string | null;
    syncedImageHash: string | null;
  };
  refreshTasks: () => void;
  refreshHistory: () => void;
  refreshSettings: () => void;
}

const AppStateContext = createContext<GlobalState | undefined>(undefined);

export function AppStateProvider({ children }: { children: ReactNode }) {
  const [daemonStatus, setDaemonStatus] = useState<'loading' | 'connected' | 'error'>('loading');
  const [connectionStatus, setConnectionStatus] = useState<'disconnected' | 'connecting' | 'connected'>('disconnected');
  const [peer, setPeer] = useState<any | null>(null);
  const [tasks, setTasks] = useState<any[]>([]);
  const [history, setHistory] = useState<any[]>([]);
  const [settings, setSettings] = useState<any | null>(null);
  const [clipboard, setClipboard] = useState<{syncedTextHash: string|null, syncedImageHash: string|null}>({
    syncedTextHash: null,
    syncedImageHash: null
  });

  const refreshTasks = async () => {
    try {
      const data = await fetchTasks();
      setTasks(data.tasks || []);
    } catch (e) { console.error('refreshTasks failed', e); }
  };

  const refreshHistory = async () => {
    try {
      const data = await fetchHistory();
      setHistory(data.history || []);
    } catch (e) { console.error('refreshHistory failed', e); }
  };

  const refreshSettings = async () => {
    try {
      const data = await fetchSettings();
      setSettings(data);
    } catch (e) { console.error('refreshSettings failed', e); }
  };

  useEffect(() => {
    async function init() {
      try {
        const state = await fetchState();
        setDaemonStatus('connected');
        setConnectionStatus(state.state.connection_status);
        setPeer(state.state.peer || null);
        setSettings(state.config || null);
        
        await refreshTasks();
        await refreshHistory();
        
        connectEvents();
      } catch (err) {
        console.error('Failed to init state:', err);
        setDaemonStatus('error');
      }
    }
    init();

    return () => {
      disconnectEvents();
    };
  }, []);

  useEffect(() => {
    if (daemonStatus !== 'connected') return;

    const unsubs = [
      subscribeEvent('connection.changed', (data: any) => {
        setConnectionStatus(data.status);
        if (data.peer) setPeer(data.peer);
        else if (data.status === 'disconnected') setPeer(null);
      }),
      subscribeEvent('transfer.started', () => refreshTasks()),
      subscribeEvent('transfer.progress', () => refreshTasks()), // In a real app we might optimize this to update locally
      subscribeEvent('transfer.completed', () => { refreshTasks(); refreshHistory(); }),
      subscribeEvent('transfer.failed', () => { refreshTasks(); refreshHistory(); }),
      subscribeEvent('transfer.cancelled', () => { refreshTasks(); refreshHistory(); }),
      subscribeEvent('clipboard.synced', (data: any) => {
        setClipboard(prev => ({
          ...prev,
          syncedTextHash: data.kind === 'text' ? data.hash : prev.syncedTextHash,
          syncedImageHash: data.kind === 'image' ? data.hash : prev.syncedImageHash,
        }));
      })
    ];

    return () => unsubs.forEach(unsub => unsub());
  }, [daemonStatus]);

  const value = {
    daemonStatus,
    connectionStatus,
    peer,
    tasks,
    history,
    settings,
    clipboard,
    refreshTasks,
    refreshHistory,
    refreshSettings
  };

  return (
    <AppStateContext.Provider value={value}>
      {children}
    </AppStateContext.Provider>
  );
}

export function useAppState() {
  const context = useContext(AppStateContext);
  if (context === undefined) {
    throw new Error('useAppState must be used within a AppStateProvider');
  }
  return context;
}
