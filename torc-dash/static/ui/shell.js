// The dashboard's only hand-written script: preferences, theme, the action menu, and keyboard
// shortcuts. Everything else is Datastar attributes on server-rendered HTML.

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

// The token arrives once in the URL; the server keeps it in a cookie, so drop it from the
// address bar (and history and screenshots).
if (new URLSearchParams(location.search).has("token")) {
  const u = new URL(location.href);
  u.searchParams.delete("token");
  history.replaceState(history.state, "", u);
}

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

/** Ask the server to save a file: navigates to a download URL (the token is in a cookie). */
window.torcDownload = (url) => {
  const a = document.createElement("a");
  // A unique URL per download: the desktop app tracks downloads by URL.
  a.href = `${url}${url.includes("?") ? "&" : "?"}_=${Math.random().toString(36).slice(2)}`;
  a.download = "";
  document.body.append(a);
  a.click();
  a.remove();
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
