let eventSource: EventSource | null = null;
type EventHandler = (event: any) => void;
const listeners = new Map<string, Set<EventHandler>>();

export function connectEvents() {
  if (eventSource) return;

  eventSource = new EventSource('/local/events');

  eventSource.onmessage = (msg) => {
    try {
      const event = JSON.parse(msg.data);
      const type = event.type;
      if (listeners.has(type)) {
        listeners.get(type)!.forEach(handler => handler(event.data));
      }
      // Also trigger a wildcard listener if needed
      if (listeners.has('*')) {
        listeners.get('*')!.forEach(handler => handler(event));
      }
    } catch (e) {
      console.error('Failed to parse event:', e);
    }
  };

  eventSource.onerror = (e) => {
    console.error('EventSource failed:', e);
    // Automatic reconnect is handled by EventSource natively, 
    // but we can inform the app about connection issues
    if (listeners.has('connection.error')) {
      listeners.get('connection.error')!.forEach(handler => handler(e));
    }
  };
}

export function disconnectEvents() {
  if (eventSource) {
    eventSource.close();
    eventSource = null;
  }
}

export function subscribeEvent(type: string, handler: EventHandler) {
  if (!listeners.has(type)) {
    listeners.set(type, new Set());
  }
  listeners.get(type)!.add(handler);
  return () => {
    const set = listeners.get(type);
    if (set) {
      set.delete(handler);
    }
  };
}
