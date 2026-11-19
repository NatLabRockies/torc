/**
 * Promise-based confirmation dialog rendered by <ConfirmHost/> in App.svelte.
 *   if (await confirm({ title: "Delete workflow?", message: "…", danger: true })) { … }
 */
export type ConfirmOptions = {
  title: string;
  message?: string;
  confirmLabel?: string;
  danger?: boolean;
};

type Pending = ConfirmOptions & { resolve: (ok: boolean) => void };

export const confirmState = $state<{ current: Pending | null }>({ current: null });

export function confirm(options: ConfirmOptions): Promise<boolean> {
  confirmState.current?.resolve(false);
  return new Promise((resolve) => {
    confirmState.current = { ...options, resolve };
  });
}
