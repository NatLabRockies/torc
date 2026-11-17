/** Global toasts: toast.success("Saved"), toast.error(err), toast.info("…"). */
import { errorText } from "../format";

export type ToastKind = "success" | "error" | "info";
type Toast = { id: number; kind: ToastKind; message: string };

let nextId = 0;
export const toasts = $state<Toast[]>([]);

/** `ms` null: stays until dismissed. */
function push(kind: ToastKind, message: string, ms: number | null) {
  const id = nextId++;
  toasts.push({ id, kind, message });
  if (ms !== null) setTimeout(() => dismiss(id), ms);
}

export function dismiss(id: number) {
  const i = toasts.findIndex((t) => t.id === id);
  if (i >= 0) toasts.splice(i, 1);
}

export const toast = {
  success: (message: string) => push("success", message, 3500),
  info: (message: string) => push("info", message, 3500),
  // Errors stay so they can be read and copied.
  error: (e: unknown) => push("error", errorText(e), null),
};
