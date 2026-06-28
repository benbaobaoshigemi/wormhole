import React, { createContext, ReactNode, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { openEventStream } from "../localApi/events";
import {
  fetchClipboardStatus,
  fetchHistory,
  fetchSettings,
  fetchState,
  fetchTasks,
} from "../localApi/localClient";
import type {
  ClipboardStatusDto,
  ConnectionStatus,
  PublicDevice,
  PublicSettingsDto,
  StateDto,
  TransferTaskDto,
  WormholeEvent,
} from "../localApi/dto";

type DaemonStatus = "loading" | "online" | "offline";

interface AppStateValue {
  daemonStatus: DaemonStatus;
  localApiUrl: string;
  device: PublicDevice | null;
  connectionStatus: ConnectionStatus;
  peer: PublicDevice | null;
  settings: PublicSettingsDto | null;
  clipboard: ClipboardStatusDto | null;
  tasks: TransferTaskDto[];
  history: TransferTaskDto[];
  events: WormholeEvent[];
  lastError: string | null;
  refreshState: () => Promise<void>;
  refreshTasks: () => Promise<void>;
  refreshHistory: () => Promise<void>;
  refreshSettings: () => Promise<void>;
  refreshClipboard: () => Promise<void>;
}

const AppStateContext = createContext<AppStateValue | undefined>(undefined);

function mergeTask(tasks: TransferTaskDto[], patch: Partial<TransferTaskDto> & { task_id?: unknown }) {
  if (typeof patch.task_id !== "string") return tasks;
  return tasks.map((task) => (task.task_id === patch.task_id ? { ...task, ...patch } : task));
}

function localApiUrl() {
  return `${window.location.origin}/local`;
}

export function AppStateProvider({ children }: { children: ReactNode }) {
  const [daemonStatus, setDaemonStatus] = useState<DaemonStatus>("loading");
  const [device, setDevice] = useState<PublicDevice | null>(null);
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>("unconfigured");
  const [peer, setPeer] = useState<PublicDevice | null>(null);
  const [settings, setSettings] = useState<PublicSettingsDto | null>(null);
  const [clipboard, setClipboard] = useState<ClipboardStatusDto | null>(null);
  const [tasks, setTasks] = useState<TransferTaskDto[]>([]);
  const [history, setHistory] = useState<TransferTaskDto[]>([]);
  const [events, setEvents] = useState<WormholeEvent[]>([]);
  const [lastError, setLastError] = useState<string | null>(null);

  const applyState = useCallback((state: StateDto) => {
    setDaemonStatus("online");
    setDevice(state.device);
    setConnectionStatus(state.status);
    setPeer(state.peer ?? null);
    setSettings(state.settings);
    setClipboard(state.clipboard);
    setTasks(state.tasks);
    setEvents(state.events);
    setLastError(null);
  }, []);

  const refreshState = useCallback(async () => {
    try {
      applyState(await fetchState());
    } catch (error) {
      setDaemonStatus("offline");
      setLastError(error instanceof Error ? error.message : String(error));
    }
  }, [applyState]);

  const refreshTasks = useCallback(async () => {
    setTasks(await fetchTasks());
  }, []);

  const refreshHistory = useCallback(async () => {
    setHistory(await fetchHistory());
  }, []);

  const refreshSettings = useCallback(async () => {
    setSettings(await fetchSettings());
  }, []);

  const refreshClipboard = useCallback(async () => {
    setClipboard(await fetchClipboardStatus());
  }, []);

  useEffect(() => {
    void refreshState().then(refreshHistory);
  }, [refreshHistory, refreshState]);

  useEffect(() => {
    if (daemonStatus !== "online") return;
    return openEventStream(
      (event) => {
        setEvents((current) => [...current.slice(-99), event]);
        if (event.type === "connection.changed") {
          void refreshState();
        }
        if (event.type === "settings.updated") {
          const next = event.data.settings as PublicSettingsDto | undefined;
          if (next) setSettings(next);
        }
        if (event.type === "transfer.progress") {
          setTasks((current) => mergeTask(current, event.data as Partial<TransferTaskDto>));
        }
        if (
          event.type === "transfer.started" ||
          event.type === "transfer.cancelled" ||
          event.type === "transfer.completed" ||
          event.type === "transfer.failed"
        ) {
          void refreshTasks();
        }
        if (event.type === "transfer.completed" || event.type === "transfer.failed") {
          void refreshHistory();
        }
        if (event.type.startsWith("clipboard.")) {
          void refreshClipboard();
        }
      },
      () => {
        void refreshState();
      },
    );
  }, [daemonStatus, refreshClipboard, refreshHistory, refreshState, refreshTasks]);

  const value = useMemo<AppStateValue>(
    () => ({
      daemonStatus,
      localApiUrl: localApiUrl(),
      device,
      connectionStatus,
      peer,
      settings,
      clipboard,
      tasks,
      history,
      events,
      lastError,
      refreshState,
      refreshTasks,
      refreshHistory,
      refreshSettings,
      refreshClipboard,
    }),
    [
      clipboard,
      connectionStatus,
      daemonStatus,
      device,
      events,
      history,
      lastError,
      peer,
      refreshClipboard,
      refreshHistory,
      refreshSettings,
      refreshState,
      refreshTasks,
      settings,
      tasks,
    ],
  );

  return <AppStateContext.Provider value={value}>{children}</AppStateContext.Provider>;
}

export function useAppState() {
  const context = useContext(AppStateContext);
  if (!context) throw new Error("useAppState must be used within AppStateProvider");
  return context;
}
