// Типизированные вызовы ядра. Названия и поля совпадают с командами в
// app/src-tauri/src (settings.rs, builds.rs).

import { call } from "./tauri";
import type { Appearance } from "./theme.svelte";

export interface Config {
  data_dir: string | null;
  appearance: Appearance;
}

export interface DirCheck {
  path: string;
  free_bytes: number | null;
  has_data: boolean;
  writable: boolean;
  warnings: string[];
}

export interface Instance {
  id: string;
  name: string;
  version: string;
  loader: "vanilla";
  group: string | null;
  pinned: boolean;
  created: number;
  last_played: number | null;
  playtime: number;
}

export interface Entry extends Instance {
  state: "active" | "archived";
  has_cover: boolean;
  archive_size: number | null;
}

export interface TrashEntry {
  trash_id: string;
  instance: Instance;
  deleted_at: number;
  size: number;
}

export interface VersionInfo {
  id: string;
  kind: "release" | "snapshot";
  date: string;
}

export interface Patch {
  name?: string;
  /// null — убрать из группы.
  group?: string | null;
  pinned?: boolean;
}

export const TRASH_DAYS = 30;

export const api = {
  config: () => call<Config>("get_config"),
  dataDirSuggestion: () => call<DirCheck>("data_dir_suggestion"),
  inspectDir: (path: string) => call<DirCheck>("inspect_dir", { path }),
  setDataDir: (path: string) => call<Config>("set_data_dir", { path }),
  openDataDir: () => call<void>("open_data_dir"),

  versions: (snapshots: boolean) => call<VersionInfo[]>("versions_list", { snapshots }),
  builds: () => call<Entry[]>("builds_list"),
  groups: () => call<string[]>("builds_groups"),
  create: (name: string, version: string, group: string | null) =>
    call<Instance>("build_create", { name, version, group }),
  update: (id: string, patch: Patch) => call<Instance>("build_update", { id, patch }),
  duplicate: (id: string) => call<Instance>("build_duplicate", { id }),
  trash: (id: string) => call<void>("build_trash", { id }),
  archive: (id: string) => call<void>("build_archive", { id }),
  unarchive: (id: string) => call<Instance>("build_unarchive", { id }),
  size: (id: string) => call<number>("build_size", { id }),
  openFolder: (id: string) => call<void>("build_open_folder", { id }),
  cover: (id: string, archived: boolean) => call<string | null>("build_cover", { id, archived }),
  setCover: (id: string, pngBase64: string) => call<void>("build_set_cover", { id, pngBase64 }),
  removeCover: (id: string) => call<void>("build_remove_cover", { id }),
  readImage: (path: string) => call<string>("read_image", { path }),
  addGroup: (name: string) => call<void>("group_add", { name }),
  deleteGroup: (name: string) => call<void>("group_delete", { name }),
  trashList: () => call<TrashEntry[]>("trash_list"),
  restore: (trashId: string) => call<Instance>("trash_restore", { trashId }),
  purge: (trashId: string) => call<void>("trash_purge", { trashId }),
};
