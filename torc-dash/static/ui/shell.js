// The dashboard's main hand-written script: preferences, theme, the action menu, and keyboard
// shortcuts. editor.js sets up the spec editor; everything else is Datastar attributes on
// server-rendered HTML.

// Preferences live in localStorage (per origin); the page's signals start from them.
window.torcPrefs = (() => {
  try {
    return JSON.parse(localStorage.getItem("torc-dash-prefs")) || {};
  } catch {
    return {};
  }
})();

window.torcSave = (prefs) => {
  try {
    localStorage.setItem("torc-dash-prefs", JSON.stringify(prefs));
  } catch {
    // Preferences are a convenience; ignore storage failures.
  }
};

window.torcTheme = (theme, palette) => {
  const d = document.documentElement.dataset;
  if (!theme || theme === "system") delete d.theme;
  else d.theme = theme;
  if (!palette || palette === "default") delete d.palette;
  else d.palette = palette;
};
torcTheme(torcPrefs.theme, torcPrefs.palette);

// The dashboard token (required on TCP) arrives once as `?token=` in the URL torc-dash prints. It
// is kept in localStorage, which is per origin (host and port), so other servers on localhost never
// see it (a cookie would be sent to every port), and sent with every request to torc-dash.
const TOKEN_KEY = "torc-dash-token";
const token = (() => {
  const url = new URL(location.href);
  const fromUrl = url.searchParams.get("token");
  if (fromUrl === null) {
    try {
      return localStorage.getItem(TOKEN_KEY) || "";
    } catch {
      return "";
    }
  }
  try {
    localStorage.setItem(TOKEN_KEY, fromUrl);
    // Saved, so drop it from the address bar (and from history and screenshots).
    url.searchParams.delete("token");
    history.replaceState(history.state, "", url);
  } catch {
    // No storage: keep it in the URL so a reload still works.
  }
  return fromUrl;
})();

// Datastar requests go through fetch: add the token to same-origin ones, and explain a refusal.
const nativeFetch = window.fetch.bind(window);
window.fetch = async (input, init = {}) => {
  const url = new URL(input instanceof Request ? input.url : input, location.href);
  if (url.origin !== location.origin) return nativeFetch(input, init);
  const headers = new Headers(init.headers || (input instanceof Request ? input.headers : undefined));
  if (token) headers.set("X-Torc-Dash-Token", token);
  const response = await nativeFetch(input, { ...init, headers });
  if (response.status === 401 && url.pathname === "/ui/tab") {
    const content = document.getElementById("content");
    if (content) {
      content.innerHTML =
        '<div class="empty"><h3>Missing or wrong dashboard token</h3><p>Open the dashboard with ' +
        "the URL torc-dash printed at startup (it ends in ?token=...).</p></div>";
    }
  }
  return response;
};

/** `path` with the token as a query parameter, for navigations, which cannot send headers. */
const withToken = (path) =>
  token ? `${path}${path.includes("?") ? "&" : "?"}token=${encodeURIComponent(token)}` : path;

/** Tab named in the URL hash (#/details). */
window.torcTab = () => location.hash.replace(/^#\/?/, "").split(/[/?]/)[0] || "workflows";

// Action menus: a trigger calls torcAnchor(el) and fetches the items into #menu; the response
// runs torcMenu() to open it under the trigger (in the top layer, so tables can't clip it).
let anchor = null;
window.torcAnchor = (el) => (anchor = el.getBoundingClientRect());
window.torcMenu = () => {
  const m = document.getElementById("menu");
  if (!anchor || !m) return;
  m.style.right = `${innerWidth - anchor.right}px`;
  const below = innerHeight - anchor.bottom > 340;
  m.style.top = below ? `${anchor.bottom + 4}px` : "auto";
  m.style.bottom = below ? "auto" : `${innerHeight - anchor.top + 4}px`;
  m.showPopover();
};

/** Ask the server to save a file: navigates to a download URL. */
window.torcDownload = (url) => {
  const a = document.createElement("a");
  // A unique URL per download: the desktop app tracks downloads by URL.
  a.href = withToken(`${url}${url.includes("?") ? "&" : "?"}_=${Math.random().toString(36).slice(2)}`);
  a.download = "";
  document.body.append(a);
  a.click();
  a.remove();
};

// The spec editor (CodeMirror, about 140 KB gzipped) loads on first use of the editor tab. The
// tab's markup calls these; they do nothing until it has loaded.
let editor = null;
window.torcEditor = {
  mount: (el) => import("/static/ui/editor.js").then((m) => (editor = m.mount(el))),
  sync: (text, format, readOnly) => editor?.sync(text, format, readOnly),
  select: (from, to) => editor?.select(from, to),
  cursor: () => editor?.cursor() ?? 0,
};

const KEYS = { 1: "workflows", 2: "details", 3: "dag", 4: "events", 5: "debugging", 6: "resources", e: "editor", 7: "chat", 8: "config", g: "dag" };
const fire = (name) => document.body.dispatchEvent(new CustomEvent(name));

addEventListener("keydown", (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey || e.defaultPrevented) return;
  const t = e.target;
  // Esc leaves a text field so the single-key shortcuts work again (dialogs handle their own).
  if (e.key === "Escape" && t.matches("input, textarea, select") && !t.closest("dialog[open]")) {
    t.blur();
    return;
  }
  if (t.closest("input, textarea, select, [contenteditable], dialog[open]")) return;
  if (KEYS[e.key]) location.hash = `/${KEYS[e.key]}`;
  else if (e.key === "Backspace" && torcTab() === "dag") history.back();
  else if (e.key === "r") fire("torc-refresh");
  else if (e.key === "t") fire("torc-theme");
  else if (e.key === "n") fire("torc-create");
  else if (e.key === "?") document.getElementById("shortcuts").showModal();
  else if (e.key === "/") document.querySelector("#content input.filter")?.focus();
  else return;
  e.preventDefault();
});
