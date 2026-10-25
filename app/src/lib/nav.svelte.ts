// Где сейчас окно: раздел боковой панели и, если открыта, страница сборки.

import { ALL, type SectionId } from "./sections";

// Раздел из адреса (#settings, #build=id) — для снимков в браузере; в окне
// лаунчера адрес не меняется.
const hash = location.hash.slice(1);
const fromHash = ALL.find((s) => s.id === hash)?.id;
const buildFromHash = hash.startsWith("build=") ? hash.slice(6) : null;

export const nav = $state({
  section: (buildFromHash ? "builds" : (fromHash ?? "home")) as SectionId,
  /// Открытая страница сборки (раздел «Сборки»).
  build: buildFromHash,
  /// Сборка, выбранная на главном экране; null — последняя запущенная.
  homeBuild: null as string | null,
  /// Вкладка, с которой открыть страницу сборки (одноразово).
  buildTab: null as string | null,
});

/// Последняя открытая вкладка каждой сборки: возврат на страницу сборки
/// (в том числе кнопками «назад» и «вперёд») открывает её же.
export const lastTab: Record<string, string> = {};

/// Какую страницу настроек открыть при переходе (одноразово).
export const settingsPage = $state({
  value: null as null | "profile" | "look" | "game" | "catalog" | "optimize" | "updates" | "data" | "about",
});
/// Окно «Оптимизировать» для сборки — открывается из любого места.
export const optimizing = $state({ build: null as null | { id: string; name: string } });

export function openSettings(page: NonNullable<typeof settingsPage.value>) {
  settingsPage.value = page;
  go("settings");
}

export function go(section: SectionId) {
  nav.section = section;
  nav.build = null;
}

export function openBuild(id: string, tab: string | null = null) {
  nav.section = "builds";
  nav.build = id;
  nav.buildTab = tab;
}
