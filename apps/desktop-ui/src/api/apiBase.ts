// Resolves the API base URL based on environment variables or defaults to relative path.
// This allows the Vite proxy to handle '/local' seamlessly during dev, 
// and allows overriding in production or Tauri env if needed.

export function resolveApiBase(): string {
  // If Vite env variable is set, use it (e.g., VITE_WORMHOLE_API_BASE=http://127.0.0.1:53317)
  // Otherwise, default to relative path '/local', which works with Vite proxy and Tauri proxy
  const envBase = import.meta.env.VITE_WORMHOLE_API_BASE;
  if (envBase) {
    return envBase.replace(/\/$/, '');
  }
  return '';
}

export function resolveEventUrl(path: string): string {
  const base = resolveApiBase();
  // if base is empty, path must start with '/' (e.g. '/local/events')
  if (!base && !path.startsWith('/')) {
    return `/${path}`;
  }
  return `${base}${path}`;
}
