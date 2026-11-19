/**
 * Global app state shared across features. Import `app` and read/write fields directly; they are
 * Svelte 5 runes, so components re-render when they change.
 *
 * Data loading convention: features reload in an $effect that reads `app.refreshTick` (and
 * `app.workflowId` if relevant). Keep showing the previous data while a refresh is in flight;
 * only show a loading state when there is nothing to show yet.
 */

import { api } from "./api/client";
import { errorText } from "./format";

export const TABS = [
  { id: "workflows", label: "Workflows", key: "1" },
  { id: "details", label: "Details", key: "2" },
  { id: "dag", label: "Graph", key: "3" },
  { id: "events", label: "Events", key: "4" },
  { id: "debugging", label: "Debugging", key: "5" },
  { id: "resources", label: "Resources", key: "6" },
  { id: "editor", label: "Spec editor", key: "e" },
  { id: "chat", label: "AI Chat", key: "7" },
  { id: "config", label: "Settings", key: "8" },
] as const;

export type TabId = (typeof TABS)[number]["id"];

/** Tabs that operate on the selected workflow; the top bar shows the workflow picker for these. */
export const WORKFLOW_TABS: ReadonlySet<TabId> = new Set([
  "details",
  "dag",
  "events",
  "debugging",
  "resources",
]);

export type ThemeMode = "system" | "light" | "dark";

/** Named dark palettes (defined in app.css); "default" is the built-in dark theme. */
export const DARK_PALETTES = [
  { id: "default", label: "Default" },
  { id: "warm", label: "Warm Gray" },
  { id: "neutral", label: "Neutral" },
  { id: "dimmed", label: "Dimmed" },
  { id: "nord", label: "Nord" },
  { id: "dracula", label: "Dracula" },
  { id: "midnight", label: "Midnight Blue" },
  { id: "high-contrast", label: "High Contrast" },
  { id: "monokai", label: "Monokai" },
] as const;
export type DarkPalette = (typeof DARK_PALETTES)[number]["id"];

function load(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function save(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    // Preferences are a convenience; ignore storage failures.
  }
}

/** Saved auto-refresh interval; 0 means off. Missing or invalid values fall back to 30 seconds. */
function savedRefreshSeconds(): number {
  const saved = load("torc-refresh-interval");
  const n = saved === null ? NaN : Number(saved);
  return Number.isFinite(n) && n >= 0 ? n : 30;
}

function tabFromHash(): TabId | null {
  const id = location.hash.replace(/^#\/?/, "").split(/[/?]/)[0];
  return TABS.some((t) => t.id === id) ? (id as TabId) : null;
}

// Carry over the pre-Svelte dashboard's dark-mode choice (torc-dark-mode / torc-theme) once.
if (load("torc-theme-mode") === null && load("torc-dark-mode") !== null) {
  const dark = load("torc-dark-mode") === "true";
  save("torc-theme-mode", dark ? "dark" : "light");
  const old = load("torc-theme");
  if (dark && old && DARK_PALETTES.some((p) => p.id === old)) save("torc-dark-palette", old);
  if (dark) document.documentElement.dataset.theme = "dark";
  else document.documentElement.dataset.theme = "light";
  if (dark && old) document.documentElement.dataset.palette = old;
}

class AppState {
  tab = $state<TabId>(tabFromHash() ?? "workflows");
  /** Tab shown before the current one, for the Graph tab's Back button. */
  previousTab = $state<TabId | null>(null);
  workflowId = $state<number | null>(Number(load("torc-workflow-id")) || null);

  user = $state<string | null>(null);
  /** True once /api/user has answered or failed; until then workflow lists wait for it. */
  userChecked = $state(false);
  /** Access-group filter for the Workflows tab and the workflow picker. */
  accessGroup = $state("");
  version = $state<string | null>(null);
  /** null until the first health check finishes. */
  connected = $state<boolean | null>(null);
  /** Why the last health check failed, if it did. */
  connectionError = $state<string | null>(null);

  refreshSeconds = $state(savedRefreshSeconds());
  /** Increments on every auto-refresh and manual refresh. Read it in an $effect to reload. */
  refreshTick = $state(0);
  showAllUsers = $state(load("torc-show-all-users") === "true");
  theme = $state<ThemeMode>((load("torc-theme-mode") as ThemeMode) ?? "system");
  darkPalette = $state<DarkPalette>((load("torc-dark-palette") as DarkPalette) ?? "default");

  /** Job details modal (owned by features/details). Set via openJob(). */
  jobDetailsId = $state<number | null>(null);
  /** File viewer modal (owned by features/details). Set via openFile(). */
  fileViewer = $state<{ path: string; title?: string } | null>(null);
  /** Create-workflow modal (owned by features/create). */
  createOpen = $state(false);
  /** Unseen event count shown on the sidebar (owned by features/events). */
  eventBadge = $state(0);
  /** Keyboard shortcut help overlay. */
  showShortcuts = $state(false);

  constructor() {
    window.addEventListener("hashchange", () => {
      const t = tabFromHash();
      if (t) this.setTab(t);
    });
  }

  /** Switch tabs, optionally selecting a workflow first. */
  go(tab: TabId, workflowId?: number | null) {
    if (workflowId !== undefined) this.selectWorkflow(workflowId);
    this.setTab(tab);
    if (tabFromHash() !== tab) location.hash = `/${tab}`;
  }

  private setTab(tab: TabId) {
    if (tab !== this.tab) this.previousTab = this.tab;
    this.tab = tab;
  }

  /** Return to the previous tab, if there is one. */
  back() {
    if (this.previousTab) this.go(this.previousTab);
  }

  selectWorkflow(id: number | null) {
    this.workflowId = id;
    save("torc-workflow-id", id === null ? null : String(id));
  }

  /** Whether the latest refresh came from the auto-refresh timer (plain field, not reactive). */
  lastRefreshAuto = false;

  refresh(auto = false) {
    this.lastRefreshAuto = auto;
    this.refreshTick++;
  }

  /**
   * Query for the workflow lists (Workflows tab and picker), or null while /api/user is pending.
   * An access group spans all owners; if the user lookup failed, list everything as main did.
   */
  workflowQuery(): { user?: string; access_group?: string } | null {
    const group = this.accessGroup || undefined;
    if (this.showAllUsers || group) return { access_group: group };
    if (this.user) return { user: this.user };
    return this.userChecked ? {} : null;
  }

  /** Ping torc-server (through torc-dash) and record the result. */
  async checkConnection(): Promise<boolean> {
    try {
      const r = await api.GET("/ping");
      this.connected = r.response.ok;
      this.connectionError = r.response.ok ? null : `HTTP ${r.response.status} ${r.response.statusText}`;
    } catch (e) {
      this.connected = false;
      this.connectionError = errorText(e);
    }
    return this.connected;
  }

  openJob(jobId: number) {
    this.jobDetailsId = jobId;
  }

  openFile(path: string, title?: string) {
    this.fileViewer = { path, title };
  }

  setTheme(mode: ThemeMode) {
    this.theme = mode;
    save("torc-theme-mode", mode === "system" ? null : mode);
    if (mode === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = mode;
  }

  setDarkPalette(palette: DarkPalette) {
    this.darkPalette = palette;
    save("torc-dark-palette", palette === "default" ? null : palette);
    if (palette === "default") delete document.documentElement.dataset.palette;
    else document.documentElement.dataset.palette = palette;
  }

  setRefreshSeconds(seconds: number) {
    this.refreshSeconds = seconds;
    save("torc-refresh-interval", String(seconds));
  }

  setShowAllUsers(value: boolean) {
    this.showAllUsers = value;
    save("torc-show-all-users", String(value));
  }
}

export const app = new AppState();
