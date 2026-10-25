// Запуск игры: состояние кнопки «Играть» и полосы загрузки. Ядро присылает
// события «play»; когда игра закрылась — «play-finished» (или итог ждёт
// заново открытое окно в play_status).

import { listen } from "@tauri-apps/api/event";
import { api, QUICK_ID, type ModIssue, type Finished, type PlayStatus, type RiskyWorld } from "./api";
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
  /// Миры сборки открывали в более новой версии — спросить перед запуском.
  risky: null as null | { id: string; version: string | null; worlds: RiskyWorld[] },
});

/// Можно ли сейчас отменить запуск этой сборки (до появления окна игры).
export function cancellable(id: string): boolean {
  const s = play.status;
  return s.build === id && ["preparing", "downloading", "installing", "starting"].includes(s.phase);
}

/// «Отменить» на кнопке «Играть».
export async function cancel() {
  try {
    await api.playCancel();
    toast("Запуск отменён");
  } catch (e) {
    toast(String(e), { error: true });
  }
}

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
    case "running":
      return s.restart ? "Перезапуск после выхода" : "Игра запущена";
    case "starting": {
      const p = launchPercent(s);
      return p === null ? "Запуск…" : `Запуск ${p}%`;
    }
    default:
      return "Игра запущена";
  }
}

/// Запустить сборку; `version` — для «Быстрой игры». Если миры сборки открывали
/// в более новой версии — окно спросит, сохранить ли копию и играть (`worldsOk`
/// — уже спросили). Если в сборке есть проблемы, которые не дадут игре
/// запуститься, — окно спросит «Всё равно играть?» (`force` — уже спросили).
export async function start(id: string, version: string | null = null, force = false, worldsOk = false) {
  try {
    if (!force && !worldsOk) {
      const worlds = await api.worldsCheck(id, id === QUICK_ID ? version : null).catch(() => []);
      if (worlds.length) {
        play.risky = { id, version, worlds };
        return;
      }
    }
    const blocked = await api.play(id, version, force);
    if (blocked?.length) play.blocked = { id, version, issues: blocked };
  } catch (e) {
    toast(`Не удалось запустить: ${e}`, { error: true });
  }
  refresh();
}

function finished(f: Finished) {
  refresh();
  if (f.warning) toast(f.warning, { error: true });
  // Об отмене окно уже сказало, когда игрок нажал «Отменить».
  if (f.cancelled) return;
  if (f.crashed) play.crash = f;
  else toast(`Игра закрыта: ${duration(f.seconds)} в «${f.name}»`);
}

/// Перезапуск не начался: в сборке проблемы, которые не дадут игре запуститься.
function blockedRestart(b: { id: string; issues: ModIssue[] }) {
  play.blocked = { id: b.id, version: null, issues: b.issues };
}

export async function connect() {
  if (inTauri) {
    await listen<PlayStatus>("play", (e) => {
      play.status = e.payload;
      tick(e.payload);
    });
    await listen<Finished>("play-finished", (e) => finished(e.payload));
    await listen<{ id: string; issues: ModIssue[] }>("play-blocked", (e) => blockedRestart(e.payload));
  }
  const s = await api.playStatus();
  play.status = s.status;
  tick(s.status);
  if (s.finished) finished(s.finished);
  if (s.blocked) blockedRestart(s.blocked);
}
