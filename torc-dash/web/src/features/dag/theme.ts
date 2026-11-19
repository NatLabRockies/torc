/** Chart theming helpers shared by the graph and resource plots. */

export function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Call `cb` whenever the effective theme may have changed. Returns an unsubscribe function. */
export function onThemeChange(cb: () => void): () => void {
  const mq = matchMedia("(prefers-color-scheme: dark)");
  const mo = new MutationObserver(cb);
  mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "data-palette"] });
  mq.addEventListener("change", cb);
  return () => {
    mo.disconnect();
    mq.removeEventListener("change", cb);
  };
}
