// Живой лог игры («Настройки → Запуск игры → Для опытных»): ядро присылает
// вывод игры пачками, пока она идёт. Держим только хвост — окно не пухнет.
// У каждой строки свой номер: при обрезке хвоста окно убирает только
// старые строки, а не перерисовывает все.

import { listen } from "@tauri-apps/api/event";
import { inTauri } from "./tauri";

const LIMIT = 3000;
const TRIM = 500;

export interface LogLine {
  n: number;
  text: string;
}

export const gameLog = $state({ lines: [] as LogLine[], open: false });
let next = 0;

export async function connectGameLog() {
  if (!inTauri) return;
  await listen<string[]>("game-log", (e) => {
    let lines = gameLog.lines.concat(e.payload.map((text) => ({ n: next++, text })));
    // Режем пачкой, а не по строке на каждое сообщение.
    if (lines.length > LIMIT + TRIM) lines = lines.slice(lines.length - LIMIT);
    gameLog.lines = lines;
    gameLog.open = true;
  });
}

/// Новый запуск — лог с чистого листа.
export function resetGameLog() {
  gameLog.lines = [];
}
