// Типизированные вызовы ядра. Названия и поля совпадают с командами в
// app/src-tauri/src (settings.rs, builds.rs).

import { call } from "./tauri";
import type { Appearance } from "./theme.svelte";

export interface GameSettings {
  on_launch: "close" | "minimize" | "keep";
  /// null — «авто», по объёму ОЗУ.
  memory_mb: number | null;
  jvm_args: string;
}

export interface Config {
  data_dir: string | null;
  appearance: Appearance;
  game: GameSettings;
}

export interface BuildSettings {
  memory_mb: number | null;
  jvm_args: string | null;
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
  settings?: BuildSettings;
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
  settings?: BuildSettings;
}

export interface Account {
  id: string;
  kind: "offline" | "ely";
  name: string;
  is_default: boolean;
}

export type ElyLogin = { status: "done"; accounts: Account[] } | { status: "need_code" };

export type Folder = "mods" | "resourcepacks" | "shaderpacks" | "screenshots";

export interface FileEntry {
  file: string;
  name: string;
  size: number;
  modified: number;
  enabled: boolean;
  is_dir: boolean;
}

export interface World {
  folder: string;
  name: string;
  size: number;
  last_played: number;
  icon: string | null;
}

export interface LogFile {
  file: string;
  kind: "log" | "crash";
  size: number;
  modified: number;
}

export type OpenTarget =
  | { kind: "folder"; name: string }
  | { kind: "file"; folder: Folder; file: string }
  | { kind: "world"; folder: string }
  | { kind: "log"; file: string };

export type PlaceKind = "menu" | "world" | "server";

export interface StatsFilter {
  build?: string | null;
  kind?: PlaceKind | null;
  place?: string | null;
  utc_offset_min: number;
}

export interface Summary {
  total: number;
  sessions: number;
  longest: number;
  since: number | null;
  builds: { id: string; name: string; seconds: number; sessions: number }[];
  places: { kind: PlaceKind; name?: string; seconds: number }[];
  days: { day: number; seconds: number }[];
}

export type Phase = "idle" | "preparing" | "downloading" | "starting" | "running";

export interface PlayStatus {
  build: string | null;
  phase: Phase;
  done: number;
  total: number;
}

export interface Finished {
  build: string;
  name: string;
  seconds: number;
  code: number | null;
  crashed: boolean;
  no_window: boolean;
  tail: string[];
  warning: string | null;
}

export const TRASH_DAYS = 30;

export const api = {
  config: () => call<Config>("get_config"),
  dataDirSuggestion: () => call<DirCheck>("data_dir_suggestion"),
  inspectDir: (path: string) => call<DirCheck>("inspect_dir", { path }),
  setDataDir: (path: string) => call<Config>("set_data_dir", { path }),
  openDataDir: () => call<void>("open_data_dir"),
  setGame: (game: GameSettings) => call<Config>("set_game", { game }),
  systemMemory: () => call<{ total_mb: number | null; auto_mb: number }>("system_memory"),

  accounts: () => call<Account[]>("accounts_list"),
  setDefaultAccount: (id: string) => call<Account[]>("account_set_default", { id }),
  addOffline: (nick: string) => call<Account[]>("account_add_offline", { nick }),
  addEly: (login: string, password: string, code: string | null) =>
    call<ElyLogin>("account_add_ely", { login, password, code }),
  removeAccount: (id: string) => call<Account[]>("account_remove", { id }),

  play: (id: string) => call<void>("build_play", { id }),
  playStatus: () => call<{ status: PlayStatus; finished: Finished | null }>("play_status"),

  files: (id: string, folder: Folder) => call<FileEntry[]>("build_files", { id, folder }),
  setEnabled: (id: string, folder: Folder, file: string, enabled: boolean) =>
    call<string>("build_set_enabled", { id, folder, file, enabled }),
  worlds: (id: string) => call<World[]>("build_worlds", { id }),
  logs: (id: string) => call<LogFile[]>("build_logs", { id }),
  thumbnail: (id: string, file: string) => call<string>("build_thumbnail", { id, file }),
  coverFromScreenshot: (id: string, file: string) => call<void>("build_cover_from_screenshot", { id, file }),
  open: (id: string, target: OpenTarget) => call<void>("build_open", { id, target }),
  stats: (filter: StatsFilter) => call<Summary>("stats_summary", { filter }),

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
