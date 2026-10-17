// Панель «Загрузки»: что лаунчер качает или проверяет (ядро шлёт весь список
// событием `tasks`).

import { listen } from "@tauri-apps/api/event";
import { api, type Task } from "./api";
import { inTauri } from "./tauri";

export const tasks = $state({ list: [] as Task[], open: false });

export const running = () => tasks.list.filter((t) => t.state === "running");

/// Доля готового, 0–100; null — размер неизвестен (проверка, подготовка).
export function share(t: Task): number | null {
  return t.total > 0 ? Math.min(100, Math.floor((t.done * 100) / t.total)) : null;
}

let started = false;
export async function connectTasks() {
  if (started) return;
  started = true;
  if (inTauri) await listen<Task[]>("tasks", (e) => (tasks.list = e.payload));
  tasks.list = await api.tasksList().catch(() => []);
}
