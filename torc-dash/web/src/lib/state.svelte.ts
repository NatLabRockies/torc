/**
 * Global app state shared across features. Import `app` and read/write fields directly; they are
 * Svelte 5 runes, so components re-render when they change.
 *
 * Data loading convention: features reload in an $effect that reads `app.refreshTick` (and
 * `app.workflowId` if relevant). Keep showing the previous data while a refresh is in flight;
 * only show a loading state when there is nothing to show yet.
 */

export const TABS = [
  { id: "workflows", label: "Workflows", key: "1" },
  { id: "editor", label: "Spec editor", key: "e" },
  { id: "details", label: "Details", key: "2" },
  { id: "dag", label: "Graph", key: "3" },
  { id: "events", label: "Events", key: "4" },
  { id: "debugging", label: "Debugging", key: "5" },
  { id: "resources", label: "Resources", key: "6" },
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

function tabFromHash(): TabId | null {
  const id = location.hash.replace(/^#\/?/, "").split(/[/?]/)[0];
  return TABS.some((t) => t.id === id) ? (id as TabId) : null;
}

class AppState {
  tab = $state<TabId>(tabFromHash() ?? "workflows");
  workflowId = $state<number | null>(Number(load("torc-workflow-id")) || null);

  user = $state<string | null>(null);
  version = $state<string | null>(null);
  /** null until the first health check finishes. */
  connected = $state<boolean | null>(null);

  refreshSeconds = $state(Number(load("torc-refresh-interval")) || 30);
  /** Increments on every auto-refresh and manual refresh. Read it in an $effect to reload. */
  refreshTick = $state(0);
  showAllUsers = $state(load("torc-show-all-users") === "true");
  theme = $state<ThemeMode>((load("torc-theme-mode") as ThemeMode) ?? "system");

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
      if (t) this.tab = t;
    });
  }

  /** Switch tabs, optionally selecting a workflow first. */
  go(tab: TabId, workflowId?: number | null) {
    if (workflowId !== undefined) this.selectWorkflow(workflowId);
    this.tab = tab;
    if (tabFromHash() !== tab) location.hash = `/${tab}`;
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
