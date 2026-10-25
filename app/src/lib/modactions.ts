// Кнопки у предупреждений про моды (раздел 5.6 ТЗ): что сделать по каждому.
// Простое (выключить, удалить, доверять) делает ядро; установку и поиск —
// каталог; загрузчик — настройки сборки.

import { api, type ModAction } from "./api";
import { catalog, install, openCatalog } from "./catalog.svelte";
import { openBuild, optimizing } from "./nav.svelte";
import { attempt, toast } from "./toast.svelte";

export function actionLabel(a: ModAction): string {
  switch (a.kind) {
    case "disable":
      return "Выключить мод";
    case "delete":
      return "Удалить файл";
    case "trust":
      return "Доверять этому файлу";
    case "install":
      return a.title.length < 30 ? `Скачать ${a.title}` : "Скачать";
    case "search":
      return "Найти в каталоге";
    case "loader":
      return "Сменить загрузчик";
    case "redownload":
      return "Скачать заново";
    case "disable_extra":
      return a.disable.length > 1 ? "Выключить лишние" : "Выключить лишнюю";
    case "update":
      return "Обновить";
    case "memory":
      return "Изменить память";
    case "optimize":
      return "Оптимизировать";
    case "power_settings":
      return "Настройки питания";
  }
}

/// Выполнить действие для сборки `id`. Возвращает true, если стоит перечитать отчёт.
export async function runAction(id: string, a: ModAction, name: string | null = null): Promise<boolean> {
  switch (a.kind) {
    case "disable":
    case "delete":
    case "trust":
    case "disable_extra":
      return !!(await attempt("Моды", () => api.modsApply(id, a).then(() => true)));
    case "install":
      catalog.pick = { kind: "build", id };
      await install(a.project, "mod");
      return true;
    case "search":
      openCatalog(id, "mod");
      catalog.query = a.query;
      return false;
    case "loader":
      openBuild(id, "settings");
      return false;
    case "redownload": {
      const ok = await attempt("Моды", () =>
        api.catalogInstall({ kind: "build", id }, "mod", [a.version_id], "Скачать заново").then(() => true),
      );
      if (ok) toast("Файл скачан заново");
      return !!ok;
    }
    case "memory":
      openBuild(id, "settings");
      return false;
    case "optimize":
      optimizing.build = { id, name: name ?? "сборка" };
      return false;
    case "power_settings":
      await attempt("Настройки питания", () => api.openPowerSettings());
      return false;
    case "update": {
      const titles = await attempt("Обновление", () => api.modsUpdate(id, [[a.version_id, a.file]], name ? `Обновление: ${name}` : null));
      if (titles) toast(`Обновлено: ${titles.join(", ")}`);
      return !!titles;
    }
  }
}
