import { resolveEventUrl } from "./apiBase";
import type { WormholeEvent } from "./dto";

export type EventHandler = (event: WormholeEvent) => void;
export type EventErrorHandler = () => void;

export function openEventStream(onEvent: EventHandler, onError: EventErrorHandler): () => void {
  const source = new EventSource(resolveEventUrl("/local/events"));

  source.onmessage = (message) => {
    try {
      onEvent(JSON.parse(message.data) as WormholeEvent);
    } catch (error) {
      console.error("Unable to parse Wormhole event", error);
    }
  };

  source.onerror = () => {
    onError();
  };

  return () => source.close();
}
