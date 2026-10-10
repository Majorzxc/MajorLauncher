// Состояние каталога: куда ставить, что ищем, открытый проект, что уже
// стоит. Живёт между переходами по разделам, чтобы «Назад» возвращал к той
// же выдаче.

import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { api, type CatalogTarget, type Installed, type Loader, type Plan, type ProjectType, type Sort } from "./api";
import { builds, refresh } from "./builds.svelte";
import { nav } from "./nav.svelte";
import { toast } from "./toast.svelte";

/// Куда ставить. «Новая сборка» создаётся при первой установке.
export type Pick =
  | { kind: "build"; id: string }
  | { kind: "new"; name: string; game: string; loader: Loader; loaderVersion: string | null }
  | { kind: "library"; game: string | null; loader: Loader | null };

export const catalog = $state({
  view: "catalog" as "catalog" | "library",
  kind: "mod" as ProjectType,
  query: "",
  sort: "relevance" as Sort,
  all: false,
  pick: null as Pick | null,
  /// Открытый проект (страница в лаунчере).
  project: null as string | null,
  /// Ставить зависимости без окна подтверждения (настройка ядра).
  autoDeps: false,
  /// Окно плана установки ждёт ответа.
  ask: null as null | { plan: Plan; resolve: (ok: boolean) => void },
});

/// Проекты, которые ставятся прямо сейчас, и уже стоящие в выбранной цели.
export const busy = new SvelteSet<string>();
export const installed = new SvelteMap<string, Installed>();

api
  .config()
  .then((c) => (catalog.autoDeps = c.catalog?.auto_deps ?? false))
  .catch(() => {});

export function target(pick: Pick): CatalogTarget {
  switch (pick.kind) {
    case "build":
      return { kind: "build", id: pick.id };
    case "new":
      return { kind: "new", game: pick.game, loader: pick.loader };
    default:
      return { kind: "library", game: pick.game, loader: pick.loader };
  }
}

/// Сборка по умолчанию: выбранная на главной или последняя запущенная.
export function defaultPick(): Pick {
  const active = builds.list.filter((b) => b.state === "active" && !b.quick);
  const id =
    (nav.homeBuild && active.some((b) => b.id === nav.homeBuild) ? nav.homeBuild : null) ??
    [...active].sort((a, b) => (b.last_played ?? 0) - (a.last_played ?? 0))[0]?.id;
  return id ? { kind: "build", id } : { kind: "library", game: null, loader: null };
}

/// Открыть каталог для сборки (кнопка «Каталог» на странице сборки).
export function openCatalog(buildId: string, kind: ProjectType) {
  catalog.pick = { kind: "build", id: buildId };
  catalog.kind = kind;
  catalog.view = "catalog";
  catalog.project = null;
  nav.section = "mods";
  nav.build = null;
}

export async function refreshInstalled() {
  const pick = catalog.pick;
  installed.clear();
  if (!pick || pick.kind === "new") return;
  try {
    for (const i of await api.catalogInstalled(target(pick))) installed.set(i.project_id, i);
  } catch {
    // Сборку удалили — просто ничего не отмечаем.
  }
}

function ask(plan: Plan): Promise<boolean> {
  return new Promise((resolve) => (catalog.ask = { plan, resolve }));
}

/// Поставить проект (и его обязательные зависимости) туда, куда выбрано.
/// `version` — конкретная версия со страницы проекта; без неё — лучшая подходящая.
export async function install(project: string, kind: ProjectType, version: string | null = null) {
  const pick = catalog.pick;
  if (!pick) return;
  busy.add(project);
  try {
    const plan = await api.catalogPlan(target(pick), kind, project, version);
    if (!plan.items.length) {
      toast("Эта версия уже стоит");
      return;
    }
    const simple =
      plan.items.length === 1 && !plan.missing.length && !plan.conflicts.length && !plan.items[0].replaces;
    const quiet = catalog.autoDeps && !plan.missing.length && !plan.conflicts.length;
    if (!simple && !quiet && !(await ask(plan))) return;

    // «Новая сборка» становится настоящей только сейчас.
    if (pick.kind === "new") {
      const b = await api.create(pick.name, pick.game, { loader: pick.loader, version: pick.loaderVersion }, null);
      catalog.pick = { kind: "build", id: b.id };
      await refresh();
      toast(`Создана сборка «${b.name}»`);
    }
    const titles = await api.catalogInstall(
      target(catalog.pick!),
      kind,
      plan.items.map((i) => i.version_id),
    );
    toast(`Установлено: ${titles.join(", ")}`);
    await refreshInstalled();
  } catch (e) {
    toast(String(e), { error: true });
  } finally {
    busy.delete(project);
  }
}

export async function setAutoDeps(on: boolean) {
  catalog.autoDeps = on;
  try {
    await api.setCatalog({ auto_deps: on });
  } catch (e) {
    toast(String(e), { error: true });
  }
}
