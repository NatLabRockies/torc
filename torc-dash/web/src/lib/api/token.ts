/**
 * The dashboard token torc-dash requires on /api and /torc-service requests when it listens on
 * TCP. It arrives once as `?token=` in the URL torc-dash prints (or Torc Desktop opens), and is
 * kept in localStorage, which is per origin (host and port), so other dashboards never see it.
 */
const KEY = "torc-dash-token";

function load(): string {
  const url = new URL(location.href);
  const fromUrl = url.searchParams.get("token");
  if (fromUrl === null) {
    try {
      return localStorage.getItem(KEY) ?? "";
    } catch {
      return "";
    }
  }
  try {
    localStorage.setItem(KEY, fromUrl);
    // Saved, so drop it from the address bar (and from history and screenshots).
    url.searchParams.delete("token");
    history.replaceState(history.state, "", url);
  } catch {
    // No storage: keep it in the URL so a reload still works.
  }
  return fromUrl;
}

export const token = load();

/** Headers to add to torc-dash requests. */
export const tokenHeaders: Record<string, string> = token
  ? { "X-Torc-Dash-Token": token }
  : {};

/** `path` with the token as a query parameter, for EventSource, which cannot send headers. */
export function withToken(path: string): string {
  if (!token) return path;
  return `${path}${path.includes("?") ? "&" : "?"}token=${encodeURIComponent(token)}`;
}
