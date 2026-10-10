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
});

export function go(section: SectionId) {
  nav.section = section;
  nav.build = null;
}

export function openBuild(id: string) {
  nav.section = "builds";
  nav.build = id;
}
