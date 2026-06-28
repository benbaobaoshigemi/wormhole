import { resolveApiUrl } from "./apiBase";
import type {
  ClipboardStatusDto,
  PublicSettingsDto,
  SettingsUpdateRequest,
  StateDto,
  TransferTaskDto,
} from "./dto";

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(resolveApiUrl(path), {
    ...init,
    headers: {
      ...(init?.body ? { "Content-Type": "application/json" } : {}),
      ...init?.headers,
    },
  });
  if (!response.ok) {
    const text = await response.text().catch(() => "");
    throw new Error(text || `${response.status} ${response.statusText}`);
  }
  return response.json() as Promise<T>;
}

export function fetchState(): Promise<StateDto> {
  return request<StateDto>("/local/state");
}

export function fetchSettings(): Promise<PublicSettingsDto> {
  return request<PublicSettingsDto>("/local/settings");
}

export function updateSettings(update: SettingsUpdateRequest): Promise<PublicSettingsDto> {
  return request<PublicSettingsDto>("/local/settings/update", {
    method: "POST",
    body: JSON.stringify(update),
  });
}

export function fetchTasks(): Promise<TransferTaskDto[]> {
  return request<TransferTaskDto[]>("/local/transfer/tasks");
}

export function fetchHistory(): Promise<TransferTaskDto[]> {
  return request<TransferTaskDto[]>("/local/transfer/history");
}

export function clearHistory(): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>("/local/transfer/history/clear", { method: "POST" });
}

export function cancelTransfer(taskId: string): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>("/local/transfer/cancel", {
    method: "POST",
    body: JSON.stringify({ task_id: taskId }),
  });
}

export function retryTransfer(taskId: string): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>("/local/transfer/retry", {
    method: "POST",
    body: JSON.stringify({ task_id: taskId }),
  });
}

export function fetchClipboardStatus(): Promise<ClipboardStatusDto> {
  return request<ClipboardStatusDto>("/local/clipboard/status");
}

export function enableClipboard(): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>("/local/clipboard/enable", { method: "POST" });
}

export function disableClipboard(): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>("/local/clipboard/disable", { method: "POST" });
}
