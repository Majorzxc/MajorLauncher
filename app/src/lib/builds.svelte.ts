// Список сборок в окне. Обложки и размеры подгружаются отдельно, после
// списка, — чтобы раздел открывался сразу, даже если сборок много.

import { api, type Entry } from "./api";

export const builds = $state({
  list: [] as Entry[],
  groups: [] as string[],
  loaded: false,
  error: "",
  /// Обложки как data URL; null — обложки нет.
  covers: {} as Record<string, string | null>,
  /// Размер папки, байт.
  sizes: {} as Record<string, number>,
  /// Что сейчас делается со сборкой: «Архивирую…», «Копирую…».
  busy: {} as Record<string, string>,
});

export async function refresh() {
  try {
    const [list, groups] = await Promise.all([api.builds(), api.groups()]);
    builds.list = list;
    builds.groups = groups;
    builds.error = "";
  } catch (e) {
    builds.error = String(e);
  }
  builds.loaded = true;
  for (const b of builds.list) {
    if (b.has_cover && !(b.id in builds.covers)) loadCover(b);
    if (!b.has_cover) builds.covers[b.id] = null;
    if (b.state === "active" && !(b.id in builds.sizes)) {
      api.size(b.id).then((s) => (builds.sizes[b.id] = s)).catch(() => {});
    }
  }
}

export async function loadCover(b: Entry) {
  builds.covers[b.id] = await api.cover(b.id, b.state === "archived").catch(() => null);
}

/// Забыть закэшированное о сборке (после переименования папки, архива и т. п.).
export function forget(id: string) {
  delete builds.covers[id];
  delete builds.sizes[id];
}

/// Выполнить долгую операцию с пометкой на плитке.
export async function withBusy<T>(id: string, label: string, run: () => Promise<T>): Promise<T> {
  builds.busy[id] = label;
  try {
    return await run();
  } finally {
    delete builds.busy[id];
  }
}
