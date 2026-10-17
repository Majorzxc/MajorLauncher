// Запуск игры: состояние кнопки «Играть» и полосы загрузки. Ядро присылает
// события «play»; когда игра закрылась — «play-finished» (или итог ждёт
// заново открытое окно в play_status).

import { listen } from "@tauri-apps/api/event";
import { api, type ModIssue, type Finished, type PlayStatus } from "./api";
import { refresh } from "./builds.svelte";
import { duration } from "./format";
import { inTauri } from "./tauri";
import { toast } from "./toast.svelte";

export const play = $state({
  status: { build: null, phase: "idle", done: 0, total: 0 } as PlayStatus,
  /// Игра закрылась с ошибкой — окно с подробностями.
  crash: null as Finished | null,
  /// Запуск остановлен: в сборке проблемы, которые не дадут игре запуститься.
  blocked: null as null | { id: string; version: string | null; issues: ModIssue[] },
});

export function percent(s: PlayStatus): number {
  return s.total > 0 ? Math.min(100, Math.floor((s.done * 100) / s.total)) : 0;
}

/// Подпись кнопки «Играть» для сборки `id`; null — можно запускать.
export function busyLabel(id: string): string | null {
  const s = play.status;
  if (s.phase === "idle") return null;
  if (s.build !== id) return "Запущена другая сборка";
  switch (s.phase) {
    case "preparing":
      return "Подготовка…";
    case "verifying":
      return "Проверка файлов…";
    case "downloading":
      return `Загрузка ${percent(s)}%`;
    case "installing":
      return `Ставим загрузчик ${s.done}/${s.total}`;
    case "starting":
      return "Запуск…";
    default:
      return "Игра запущена";
  }
}

/// Запустить сборку; `version` — для «Быстрой игры». Если в сборке есть
/// проблемы, которые не дадут игре запуститься, — окно спросит «Всё равно играть?».
export async function start(id: string, version: string | null = null, force = false) {
  try {
    const blocked = await api.play(id, version, force);
    if (blocked?.length) play.blocked = { id, version, issues: blocked };
  } catch (e) {
    toast(`Не удалось запустить: ${e}`, { error: true });
  }
  refresh();
}

function finished(f: Finished) {
  refresh();
  if (f.crashed) play.crash = f;
  else toast(`Игра закрыта: ${duration(f.seconds)} в «${f.name}»`);
  if (f.warning) toast(f.warning, { error: true });
}

export async function connect() {
  if (inTauri) {
    await listen<PlayStatus>("play", (e) => (play.status = e.payload));
    await listen<Finished>("play-finished", (e) => finished(e.payload));
  }
  const s = await api.playStatus();
  play.status = s.status;
  if (s.finished) finished(s.finished);
}
