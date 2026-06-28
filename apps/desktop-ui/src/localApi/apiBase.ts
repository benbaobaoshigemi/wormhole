export function resolveApiBase(): string {
  const envBase = import.meta.env.DEV ? import.meta.env.VITE_WORMHOLE_API_BASE : "";
  return typeof envBase === "string" ? envBase.replace(/\/$/, "") : "";
}

export function resolveApiUrl(path: string): string {
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  return `${resolveApiBase()}${normalizedPath}`;
}

export function resolveEventUrl(path: string): string {
  return resolveApiUrl(path);
}
