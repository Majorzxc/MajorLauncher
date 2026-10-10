// Загрузчики модов (раздел 5.3 ТЗ): названия и подписи для окна. Списки
// версий и установку ведёт ядро.

import type { Instance, Loader } from "./api";

export const LOADERS: { id: Loader; title: string; hint: string }[] = [
  { id: "vanilla", title: "Без загрузчика", hint: "Чистая игра" },
  { id: "fabric", title: "Fabric", hint: "Лёгкий, быстро обновляется" },
  { id: "forge", title: "Forge", hint: "Большие моды, старые версии" },
  { id: "neoforge", title: "NeoForge", hint: "Наследник Forge, с 1.20.2" },
];

const TITLES: Record<Loader, string> = { vanilla: "Ванилла", fabric: "Fabric", forge: "Forge", neoforge: "NeoForge" };

export function loaderTitle(loader: Loader): string {
  return TITLES[loader];
}

/// «Fabric 0.19.5» или «Ванилла».
export function loaderLabel(b: Pick<Instance, "loader" | "loader_version">): string {
  return b.loader === "vanilla" ? TITLES.vanilla : `${TITLES[b.loader]} ${b.loader_version ?? ""}`.trim();
}
