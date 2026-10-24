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

// Часы для «Запуск 0:14»: тикают только пока игра запускается.
const clock = $state({ now: Date.now() });
let ticker: ReturnType<typeof setInterval> | null = null;
function tick(s: PlayStatus) {
  const need = s.phase === "starting" && !!s.since;
  if (need && !ticker) ticker = setInterval(() => (clock.now = Date.now()), 1000);
  if (!need && ticker) {
    clearInterval(ticker);
    ticker = null;
  }
  clock.now = Date.now();
}

/// Сколько секунд идёт запуск игры (с момента старта Java).
function elapsed(s: PlayStatus): number {
  return s.since ? Math.max(0, Math.floor((clock.now - s.since) / 1000)) : 0;
}

/// Проценты запуска по времени прошлой загрузки сборки. Дальше 95 % не идёт:
/// окно игры может появиться и позже обычного. null — сборка ещё не запускалась.
export function launchPercent(s: PlayStatus): number | null {
  if (s.phase !== "starting" || !s.since || !s.expected) return null;
  return Math.min(95, Math.floor((elapsed(s) * 100) / s.expected));
}

/// Подробность к кнопке «Играть»: что делается сейчас.
export function stageLabel(id: string): string | null {
  const s = play.status;
  if (s.build !== id || s.phase === "idle" || s.phase === "running") return null;
  const parts = [s.stage];
  if (s.phase === "starting" && s.since) {
    const t = elapsed(s);
    parts.push(`${Math.floor(t / 60)}:${String(t % 60).padStart(2, "0")}`);
  }
  return parts.filter(Boolean).join(" · ") || null;
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
    case "starting": {
      const p = launchPercent(s);
      return p === null ? "Запуск…" : `Запуск ${p}%`;
    }
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
    await listen<PlayStatus>("play", (e) => {
      play.status = e.payload;
      tick(e.payload);
    });
    await listen<Finished>("play-finished", (e) => finished(e.payload));
  }
  const s = await api.playStatus();
  play.status = s.status;
  tick(s.status);
  if (s.finished) finished(s.finished);
}
