// ИИ-разбор (раздел 5.8 ТЗ): настройки для кнопок «Разобрать с ИИ» и ответы,
// которые лаунчер получил сам после игры (событие «ai-answer»).

import { listen } from "@tauri-apps/api/event";
import { api, type AiAnswer, type AiSettings } from "./api";
import { openBuild } from "./nav.svelte";
import { inTauri } from "./tauri";
import { toast } from "./toast.svelte";

export interface AiEvent {
  build: string;
  answer: AiAnswer | null;
  error: string | null;
}

export const ai = $state({
  settings: null as AiSettings | null,
  /// Последний ответ, который лаунчер получил сам.
  last: null as AiEvent | null,
  /// Сколько окон сейчас ждут ответ по сборке — им уведомление не нужно.
  waiting: {} as Record<string, number>,
});

export function loadAi() {
  api
    .config()
    .then((c) => (ai.settings = c.ai ?? null))
    .catch(() => {});
}

export async function connectAi() {
  loadAi();
  if (!inTauri) return;
  await listen<AiEvent>("ai-answer", (e) => {
    ai.last = e.payload;
    if (ai.waiting[e.payload.build]) return;
    if (e.payload.answer) {
      toast("ИИ разобрал последнюю игру", {
        action: { label: "Подробнее", run: () => openBuild(e.payload.build, "logs") },
      });
    } else if (e.payload.error) {
      toast(`ИИ-разбор после игры не удался: ${e.payload.error}`, { error: true });
    }
  });
}
