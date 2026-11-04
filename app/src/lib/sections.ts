import type { IconName } from "./icons";

export type SectionId = "home" | "builds" | "mods" | "servers" | "skins" | "stats" | "settings";

export interface Section {
  id: SectionId;
  title: string;
  icon: IconName;
}

/// Разделы боковой панели (раздел 6.1 ТЗ): сверху основные, снизу — служебные.
export const TOP: Section[] = [
  { id: "home", title: "Главная", icon: "home" },
  { id: "builds", title: "Сборки", icon: "builds" },
  { id: "mods", title: "Моды", icon: "mods" },
  { id: "servers", title: "Серверы", icon: "servers" },
  { id: "skins", title: "Скины", icon: "skins" },
];

export const BOTTOM: Section[] = [
  { id: "stats", title: "Статистика", icon: "stats" },
  { id: "settings", title: "Настройки", icon: "settings" },
];

export const ALL = [...TOP, ...BOTTOM];
