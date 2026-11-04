// Обучение (раздел 6.4 ТЗ): подсказки с подсветкой элементов. Шаги — данные
// (lib/tours.ts), что игрок уже видел — хранит ядро (config.json).

import { api, type Tutorial, type TutorialAction } from "./api";
import { go } from "./nav.svelte";
import { TOURS, type TourId } from "./tours";

export const tutorial = $state({
  state: { onboarded: true, enabled: false, seen: [], offer_until: null } as Tutorial,
  /// Идущий тур (шаги ведёт components/Tour.svelte).
  active: null as null | { id: TourId },
});

export function setTutorial(t: Tutorial | undefined) {
  if (t) tutorial.state = t;
}

async function act(action: TutorialAction) {
  try {
    setTutorial((await api.tutorial(action)).tutorial);
  } catch {
    // Не сохранилось — подсказки покажутся ещё раз, ничего страшного.
  }
}

/// Раздел открыт: показать его подсказки, если обучение включено и их ещё не видели.
export function sectionOpened(id: TourId) {
  const t = tutorial.state;
  if (tutorial.active?.id === id) return;
  tutorial.active = null;
  if (t.onboarded && t.enabled && !t.seen.includes(id) && TOURS[id]) tutorial.active = { id };
}

/// Тур закончен или пропущен — сам он больше не появится.
export function finishTour() {
  const a = tutorial.active;
  tutorial.active = null;
  if (!a) return;
  // Сразу, не дожидаясь ядра: иначе раздел успел бы открыть тот же тур снова.
  tutorial.state.seen = [...tutorial.state.seen, a.id];
  act({ action: "seen", id: a.id });
}

/// Мастер первого запуска пройден.
export const finishOnboarding = (tour: boolean) => act({ action: "finish", tutorial: tour });

/// «Пройти обучение»: основной тур сейчас, подсказки разделов — при их открытии.
export async function restartTutorial() {
  await act({ action: "restart" });
  go("home");
  tutorial.active = { id: "home" };
}

/// «Выключить подсказки».
export function stopTutorial() {
  tutorial.active = null;
  act({ action: "stop" });
}

export const hideOffer = () => act({ action: "hide_offer" });

/// Кнопка «Пройти обучение» в шапке: неделю после отказа.
export function offerShown(): boolean {
  const until = tutorial.state.offer_until;
  return until !== null && until * 1000 > Date.now() && !tutorial.active;
}
