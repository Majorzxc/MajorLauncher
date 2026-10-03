// Вызовы ядра. В окне лаунчера — настоящие команды Tauri. В обычном браузере
// (`npm run dev`) — заглушки из mock.ts, чтобы проверять вёрстку без сборки
// приложения. В релизную сборку заглушки не попадают: ветка с ними вырезается.

import { invoke } from "@tauri-apps/api/core";

export const inTauri = "__TAURI_INTERNALS__" in window;

export function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) return invoke<T>(cmd, args);
  if (import.meta.env.DEV) return import("./mock").then((m) => m.mock<T>(cmd, args));
  return Promise.reject(new Error("окно открыто не из лаунчера"));
}
