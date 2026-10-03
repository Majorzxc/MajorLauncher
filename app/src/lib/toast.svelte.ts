// Короткие уведомления внизу справа: «Сборка в корзине», «Не удалось…».

export interface Toast {
  id: number;
  text: string;
  error?: boolean;
  action?: { label: string; run: () => void };
}

export const toasts = $state<Toast[]>([]);
let next = 1;

export function toast(text: string, opts: Omit<Toast, "id" | "text"> = {}) {
  const id = next++;
  toasts.push({ id, text, ...opts });
  setTimeout(() => dismiss(id), opts.error ? 7000 : 4500);
}

export function dismiss(id: number) {
  const i = toasts.findIndex((t) => t.id === id);
  if (i >= 0) toasts.splice(i, 1);
}

/// Выполнить действие и показать ошибку уведомлением, если оно не удалось.
export async function attempt<T>(what: string, run: () => Promise<T>): Promise<T | undefined> {
  try {
    return await run();
  } catch (e) {
    toast(`${what}: ${e}`, { error: true });
    return undefined;
  }
}
