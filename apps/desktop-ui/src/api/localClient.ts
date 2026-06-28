const BASE_URL = '/local';

export async function fetchState() {
  const res = await fetch(`${BASE_URL}/state`);
  if (!res.ok) throw new Error('Failed to fetch state');
  return res.json();
}

export async function fetchSettings() {
  const res = await fetch(`${BASE_URL}/settings`);
  if (!res.ok) throw new Error('Failed to fetch settings');
  return res.json();
}

export async function updateSettings(settings: any) {
  const res = await fetch(`${BASE_URL}/settings/update`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(settings)
  });
  if (!res.ok) throw new Error('Failed to update settings');
  return res.json();
}

export async function sendTransfer(paths: string[]) {
  const res = await fetch(`${BASE_URL}/transfer/send`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ paths })
  });
  if (!res.ok) throw new Error('Failed to send transfer');
  return res.json();
}

export async function cancelTransfer(taskId: string) {
  const res = await fetch(`${BASE_URL}/transfer/cancel`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ task_id: taskId })
  });
  if (!res.ok) throw new Error('Failed to cancel transfer');
  return res.json();
}

export async function retryTransfer(taskId: string) {
  const res = await fetch(`${BASE_URL}/transfer/retry`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ task_id: taskId })
  });
  if (!res.ok) throw new Error('Failed to retry transfer');
  return res.json();
}

export async function fetchTasks() {
  const res = await fetch(`${BASE_URL}/transfer/tasks`);
  if (!res.ok) throw new Error('Failed to fetch tasks');
  return res.json();
}

export async function fetchHistory() {
  const res = await fetch(`${BASE_URL}/transfer/history`);
  if (!res.ok) throw new Error('Failed to fetch history');
  return res.json();
}

export async function clearHistory() {
  const res = await fetch(`${BASE_URL}/transfer/history/clear`, {
    method: 'POST'
  });
  if (!res.ok) throw new Error('Failed to clear history');
  return res.json();
}

export async function fetchClipboardStatus() {
  const res = await fetch(`${BASE_URL}/clipboard/status`);
  if (!res.ok) throw new Error('Failed to fetch clipboard status');
  return res.json();
}

export async function enableClipboard(kind: 'text' | 'image' | 'both') {
  const res = await fetch(`${BASE_URL}/clipboard/enable`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ kind })
  });
  if (!res.ok) throw new Error('Failed to enable clipboard');
  return res.json();
}

export async function disableClipboard(kind: 'text' | 'image' | 'both') {
  const res = await fetch(`${BASE_URL}/clipboard/disable`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ kind })
  });
  if (!res.ok) throw new Error('Failed to disable clipboard');
  return res.json();
}
