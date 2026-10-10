import type { IconName } from "./icons";

export type SectionId = "home" | "builds" | "mods" | "servers" | "skins" | "stats" | "settings";

export interface Section {
  id: SectionId;
  title: string;
  icon: IconName;
  /// Где раздел появится, пока он не готов (для заглушки).
  stage?: string;
  about?: string;
}

/// Разделы боковой панели (раздел 6.1 ТЗ): сверху основные, снизу — служебные.
export const TOP: Section[] = [
  { id: "home", title: "Главная", icon: "home" },
  { id: "builds", title: "Сборки", icon: "builds" },
  { id: "mods", title: "Моды", icon: "mods", stage: "5", about: "Каталог Modrinth и библиотека модов." },
  { id: "servers", title: "Серверы", icon: "servers", stage: "9", about: "Общий список серверов и их статус." },
  { id: "skins", title: "Скины", icon: "skins", stage: "8", about: "3D-просмотр, библиотека скинов, плащи." },
];

export const BOTTOM: Section[] = [
  { id: "stats", title: "Статистика", icon: "stats" },
  { id: "settings", title: "Настройки", icon: "settings" },
];

export const ALL = [...TOP, ...BOTTOM];
