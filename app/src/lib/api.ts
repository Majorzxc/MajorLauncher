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
  catalog?: CatalogSettings;
  optimize?: OptimizeSettings;
  updates?: UpdateSettings;
  ai?: AiSettings;
  skins?: SkinSettings;
  tutorial?: Tutorial;
}

/// Первый запуск и обучение (раздел 6.4 ТЗ).
export interface Tutorial {
  /// Мастер первого запуска пройден.
  onboarded: boolean;
  /// Подсказки разделов показываются при первом открытии.
  enabled: boolean;
  /// Уже показанные туры.
  seen: string[];
  /// До какого времени (Unix, с) кнопка «Пройти обучение» видна в шапке.
  offer_until: number | null;
}

export type TutorialAction =
  | { action: "finish"; tutorial: boolean }
  | { action: "seen"; id: string }
  | { action: "restart" | "stop" | "hide_offer" };

/// «Скины»: программа-редактор и спорные скины в каталоге.
export interface SkinSettings {
  editor: string | null;
  show_sensitive: boolean;
}

/// Скин из библиотеки.
export interface Skin {
  id: string;
  name: string;
  slim: boolean;
  source: string;
  page?: string | null;
  added: number;
}

export type NickSource = "mojang" | "ely";

/// Текстуры игрока по нику.
export interface Textures {
  name: string;
  skin: string | null;
  slim: boolean;
  cape: string | null;
}

export type SkinCatalogId = "ely" | "mineskin";

export interface SkinCatalogQuery {
  catalog: SkinCatalogId;
  search: string;
  sort: string;
  kind: string;
  slim_only: boolean;
  page: string;
  show_sensitive: boolean;
}

export interface CatalogSkin {
  id: string;
  url: string;
  name: string | null;
  slim: boolean | null;
  tags: string[];
  likes: number | null;
  wearers: number | null;
  page: string | null;
}

export interface SkinCatalogPage {
  items: CatalogSkin[];
  next: string | null;
  total: number | null;
  hidden: number;
}

/// Сервис ИИ-разбора.
export type AiProvider = "anthropic" | "openai" | "gemini" | "openrouter" | "ollama" | "lmstudio" | "custom";

/// «Настройки → ИИ-разбор».
export interface AiSettings {
  enabled: boolean;
  provider: AiProvider;
  /// Пусто — модель по умолчанию (есть только у Claude).
  model: string;
  /// Пусто — адрес сервиса по умолчанию.
  url: string;
  preview: boolean;
  auto_unknown: boolean;
  auto_critical: boolean;
  auto_warnings: boolean;
  daily_limit: number;
}

/// Ответ ИИ.
export interface AiAnswer {
  at: number;
  /// «Claude · claude-opus-5-5».
  by: string;
  /// Markdown.
  text: string;
  truncated: boolean;
  /// Спросил сам лаунчер после игры.
  auto: boolean;
}

/// Что уйдёт в сервис.
export interface AiPrepared {
  text: string;
  bytes: number;
  tokens: number;
  to: string;
}

/// Разбор последней игры сборки.
export interface ProblemSession {
  at: number;
  analysis: Analysis;
  crashed?: boolean;
  ai?: AiAnswer | null;
}

/// «Настройки → Обновления».
export interface UpdateSettings {
  check: boolean;
  auto_mods: boolean;
  auto_packs: boolean;
  /// Проверять новую версию лаунчера.
  launcher: boolean;
  /// Скачивать её самому (ставится по кнопке).
  launcher_download: boolean;
  channel: "stable" | "beta";
}

/// Новая версия лаунчера на GitHub (подпись уже проверило ядро).
export interface LauncherRelease {
  version: string;
  beta: boolean;
  notes: string;
  published: string | null;
  page: string;
  size: number;
}

export interface LauncherUpdate {
  current: string;
  available: LauncherRelease | null;
  /// Установщик скачан и проверен.
  ready: boolean;
  checked: number | null;
  busy: boolean;
  error: string | null;
}

export interface PackUpdate {
  version_id: string;
  version_number: string;
  current: string | null;
  title: string | null;
}

/// Что нового у сборки (фоновая проверка).
export interface BuildUpdates {
  build: string;
  name: string;
  mods: number;
  titles: string[];
  pack: PackUpdate | null;
}

/// Сборка из каталога: откуда и есть ли новая версия.
export interface PackInfo {
  origin: { project_id: string | null; version_id: string | null; version_number: string | null; title: string | null } | null;
  update: PackUpdate | null;
}

/// Что делает кнопка «Оптимизировать» («Настройки → Оптимизация»).
export interface OptimizeSettings {
  hardware: boolean;
  mods: boolean;
  memory: boolean;
  gpu: boolean;
  system: boolean;
  graphics: boolean;
  fps: boolean;
  intro_seen: boolean;
}

export interface Hardware {
  ram_total_mb: number | null;
  ram_free_mb: number | null;
  cores: number;
  cpu: string | null;
  gpus: { name: string; discrete: boolean; driver: string | null; driver_date: string | null }[];
  laptop: boolean;
  power_saver: boolean;
}

/// Отчёт оптимизации: что сделано, пропущено и почему.
export interface OptimizeReport {
  steps: { title: string; state: "done" | "skipped" | "info" | "failed"; text: string }[];
  snapshot: string;
}

export interface CatalogSettings {
  /// Ставить обязательные зависимости без окна подтверждения.
  auto_deps: boolean;
  /// Переводить описания с Modrinth на русский (включает игрок).
  translate?: boolean;
}

// Каталог Modrinth (раздел 5.4 ТЗ).
export type ProjectType = "mod" | "resourcepack" | "shader" | "modpack";
export type Sort = "relevance" | "downloads" | "follows" | "newest" | "updated";

/// Куда ставить: в сборку или в библиотеку (фильтры — вручную).
export type CatalogTarget =
  | { kind: "build"; id: string }
  | { kind: "new"; game: string; loader: Loader }
  | { kind: "library"; game: string | null; loader: Loader | null };

export interface SearchRequest {
  target: CatalogTarget;
  kind: ProjectType;
  query: string;
  sort: Sort;
  offset: number;
  all: boolean;
  categories: string[];
  environment: "client" | "server" | null;
  open_source: boolean;
}

export interface Hit {
  id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  icon_url: string | null;
  downloads: number;
  follows: number;
  display_categories: string[];
  date_modified: string;
}

export interface SearchResult {
  hits: Hit[];
  total_hits: number;
}

export interface Project {
  id: string;
  slug: string;
  title: string;
  description: string;
  body: string;
  project_type: ProjectType;
  icon_url: string | null;
  downloads: number;
  followers: number;
  categories: string[];
  loaders: string[];
  game_versions: string[];
  license: { id: string; name: string } | null;
  source_url: string | null;
  issues_url: string | null;
  wiki_url: string | null;
  discord_url: string | null;
  donation_urls: { platform: string; url: string }[];
  gallery: { url: string; raw_url: string | null; title: string | null; description: string | null; featured: boolean }[];
  client_side: string;
  updated: string;
}

export interface CatalogVersion {
  id: string;
  project_id: string;
  name: string;
  version_number: string;
  game_versions: string[];
  loaders: string[];
  version_type: "release" | "beta" | "alpha";
  date_published: string;
  downloads: number;
  files: { filename: string; size: number; primary: boolean }[];
  compatible: boolean;
}

export interface Installed {
  project_id: string;
  version_id: string;
  file: string;
  enabled: boolean;
}

export interface PlanItem {
  project_id: string;
  version_id: string;
  title: string;
  icon_url: string | null;
  version_number: string;
  size: number;
  dependency: boolean;
  replaces: string | null;
}

// Моды в сборке (разделы 5.4 и 5.6 ТЗ).
export interface KnownMod {
  project_id: string;
  version_id: string;
  title: string;
  version_number: string;
  icon_url: string | null;
}

export interface ModInfo {
  file: string;
  name: string;
  version: string;
  enabled: boolean;
  size: number;
  sha1: string;
  loaders: string[];
  modrinth: KnownMod | null;
  trusted: boolean;
  pinned: boolean;
  update: { version_id: string; version_number: string } | null;
  malware: string | null;
  /// Подозрительный код (5.4): что делает и где.
  suspect: { strength: "strong" | "weak"; what: string; at: string }[];
}

export type ModAction =
  | { kind: "disable"; file: string }
  | { kind: "delete"; file: string }
  | { kind: "trust"; sha1: string }
  | { kind: "install"; project: string; title: string }
  | { kind: "search"; query: string }
  | { kind: "loader" }
  | { kind: "redownload"; version_id: string; file: string }
  | { kind: "disable_extra"; disable: string[] }
  | { kind: "update"; version_id: string; file: string }
  | { kind: "memory" }
  | { kind: "optimize" }
  | { kind: "power_settings" }
  | { kind: "driver" }
  | { kind: "enable"; file: string }
  | { kind: "backups" };

export interface ModIssue {
  level: "block" | "warn" | "tip";
  text: string;
  file: string | null;
  action: ModAction | null;
}

export interface ModsReport {
  mods: ModInfo[];
  issues: ModIssue[];
  online: boolean;
}

export interface Plan {
  items: PlanItem[];
  present: string[];
  missing: string[];
  conflicts: string[];
  optional: string[];
}

export interface BuildSettings {
  memory_mb: number | null;
  jvm_args: string | null;
  /// Свои настройки игры вместо общих.
  own_options?: boolean;
  /// Свой список серверов вместо общего.
  own_servers?: boolean;
}

/// Папка данных при открытии окна.
export interface DataStatus {
  /// Папки нет ни по сохранённому пути, ни на других дисках.
  missing: string | null;
  /// Диск сменил букву: старый и новый путь.
  moved: [string, string] | null;
}

export interface DirCheck {
  path: string;
  free_bytes: number | null;
  has_data: boolean;
  writable: boolean;
  warnings: string[];
}

export interface MovePlan {
  /// Куда на самом деле (в непустую папку — во вложенную MajorLauncherData).
  to: string;
  same_disk: boolean;
  free: number | null;
  error: string | null;
}

export interface MoveProgress {
  stage: "scan" | "copy" | "verify";
  done: number;
  total: number;
}

export interface Moved {
  method: "renamed" | "copied";
  old: string | null;
  old_size: number;
}

export interface Instance {
  id: string;
  name: string;
  version: string;
  loader: Loader;
  /// Версия загрузчика; у ванили нет.
  loader_version?: string | null;
  group: string | null;
  pinned: boolean;
  created: number;
  last_played: number | null;
  playtime: number;
  settings?: BuildSettings;
  /// «Быстрая игра»: версия выбирается при запуске.
  quick?: boolean;
}

export interface Entry extends Instance {
  state: "active" | "archived";
  has_cover: boolean;
  archive_size: number | null;
  /// Включённых модов; у архивных — null.
  mods: number | null;
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
  loader?: LoaderChoice;
}

export type Loader = "vanilla" | "fabric" | "forge" | "neoforge";

export interface LoaderChoice {
  loader: Loader;
  version: string | null;
}

export interface LoaderVersion {
  version: string;
  stable: boolean;
  recommended: boolean;
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
  /// В какой версии открывали последним.
  version: string | null;
  icon: string | null;
}

export const QUICK_ID = "_quick";

export interface RiskyWorld {
  folder: string;
  name: string;
  version: string;
}

export type Group = "view" | "controls" | "keys" | "sound" | "chat" | "interface" | "language" | "graphics" | "packs";

export type SettingKind =
  | { type: "bool" }
  | { type: "slider"; min: number; max: number; step: number; mul: number; add: number; unit: string }
  | { type: "int"; min: number; max: number; special: [number, string] | null }
  | { type: "choice"; options: [string, string][]; quoted: boolean };

export interface Setting {
  key: string;
  group: Group;
  label: string;
  hint?: string;
  kind: SettingKind;
  default: string;
  since?: string;
}

export interface OptionsView {
  settings: Setting[];
  values: Record<string, string>;
  shared: Group[];
  seeded_from: string | null;
  keys: number;
  packs: string[] | null;
}

export interface LibraryItem {
  folder: "resourcepacks" | "shaderpacks";
  file: string;
  size: number;
  added: number;
  all_builds: boolean;
  builds: string[];
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
  /// Только сессии с этого момента (секунды с 1970); null — всё время.
  from?: number | null;
}

export interface Summary {
  total: number;
  sessions: number;
  longest: number;
  since: number | null;
  builds: { id: string; name: string; seconds: number; sessions: number }[];
  places: {
    key: string;
    kind: PlaceKind;
    name: string;
    build?: string;
    build_name?: string;
    folder?: string;
    seconds: number;
    sessions: number;
    first: number;
    last: number;
  }[];
  days: { day: number; seconds: number }[];
}

export type Phase = "idle" | "preparing" | "verifying" | "downloading" | "installing" | "starting" | "running";

/// Итог «Проверить файлы».
export interface Checked {
  /// Файлов игры, библиотек, ассетов и Java скачано заново.
  fixed: number;
  /// Модов из каталога, которые с ним не совпадают.
  mods_changed: number;
}

/// Что внутри файла .mrpack.
export interface PackInfo {
  name: string;
  game: string;
  loader: Loader;
  loader_version: string | null;
  files: number;
  size: number;
}

/// Мод в библиотеке модов.
export interface LibMod {
  key: string;
  file: string;
  title: string;
  version: string;
  size: number;
  in_library: boolean;
  source: { project_id: string; version_id: string; title: string; version_number: string } | null;
  icon_url: string | null;
  loaders: ("fabric" | "quilt" | "forge" | "neoforge")[];
  builds: { build: string; name: string; enabled: boolean }[];
}

export interface ModLibrary {
  mods: LibMod[];
  total: number;
  unused: number;
}

/// Задача панели «Загрузки».
export interface Task {
  id: number;
  title: string;
  place: string;
  done: number;
  total: number;
  files_done: number;
  files_total: number;
  stage: string | null;
  state: "running" | "done" | "failed" | "cancelled";
  error: string | null;
  started: number;
  finished: number | null;
}

export interface PlayStatus {
  build: string | null;
  phase: Phase;
  done: number;
  total: number;
  /// Что происходит сейчас словами: «Проверка модов», «Загрузка модов: 162».
  stage?: string | null;
  /// Когда запущена Java, мс с 1970 года.
  since?: number | null;
  /// Сколько сборка грузилась до окна игры в прошлые разы, секунд.
  expected?: number | null;
  /// «Перезапустить и вернуться» ждёт выхода из игры.
  restart?: boolean;
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
  /// Запуск отменили кнопкой «Отменить».
  cancelled?: boolean;
  /// Сколько раз подряд сборка падает.
  streak?: number;
  /// Снапшот, к которому стоит откатиться (падает после изменения сборки).
  rollback?: { id: string; reason: string; created: number } | null;
  /// Разбор анализатора ошибок.
  analysis?: Analysis | null;
  /// Лаунчер сам спросил ИИ — ответ придёт событием «ai-answer».
  ai_auto?: boolean;
}

/// Находка анализатора ошибок.
export interface Finding {
  rule: string;
  severity: "critical" | "warning";
  title: string;
  text: string;
  culprit: { id: string; name: string; file: string | null } | null;
  actions: ModAction[];
  count: number;
  lines: string[];
}

export interface Analysis {
  critical: Finding[];
  warnings: Finding[];
  crash_report: string | null;
  rules_version: number;
}

/// Сборка другого лаунчера, найденная для импорта.
export interface ImportFound {
  source: "prism" | "official" | "tlauncher" | "curseforge" | "modrinth" | "folder";
  source_title: string;
  name: string;
  path: string;
  version: string | null;
  loader: Loader | null;
  loader_version: string | null;
  guessed: boolean;
  mods: number;
  worlds: number;
  worlds_size: number;
  problem: string | null;
}

/// Экспорт в .mrpack: что получится.
export interface ExportPreview {
  name: string;
  game: string;
  loader: Loader;
  loader_version: string | null;
  modrinth: number;
  own: string[];
  disabled: number;
  packs: number;
  config_size: number;
  has_options: boolean;
  offline: boolean;
}

export interface ExportOptions {
  name: string;
  version: string;
  summary: string;
  configs: boolean;
  packs: boolean;
  options: boolean;
  disabled: boolean;
}

/// Перевод сборки на другую версию игры: что станет с модами и мирами.
export interface UpgradePlan {
  from: string;
  version: string;
  loader: Loader;
  loader_version: string | null;
  /// Новая версия старше нынешней.
  older: boolean;
  mods: {
    file: string;
    name: string;
    enabled: boolean;
    from: string;
    to: { version_id: string; version_number: string; size: number; same: boolean } | null;
    note: string | null;
  }[];
  deps: { project_id: string; version_id: string; title: string; version_number: string; size: number }[];
  missing_deps: string[];
  download: number;
  worlds: World[];
  name: string;
}

export interface UpgradeChoice {
  version: string;
  loader_version: string | null;
  name: string;
  versions: string[];
  keep_enabled: string[];
  worlds: string[];
}

/// Снапшот сборки: чем отличается от неё сейчас.
export interface SnapshotInfo {
  id: string;
  created: number;
  reason: string;
  manual: boolean;
  version: string;
  loader: Loader;
  loader_version: string | null;
  mods: number;
  /// Файлы модов, которых в снапшоте нет, — уйдут при откате.
  added: string[];
  /// Файлы модов из снапшота, которых сейчас нет, — вернутся.
  removed: string[];
  /// Сколько места занимает сам снапшот, байт.
  size: number;
  /// Сборка сейчас такая же.
  current: boolean;
}

/// Кусок описания сервера: текст одного вида (цвет — `#rrggbb`).
export interface Span {
  text: string;
  color?: string;
  bold?: boolean;
  italic?: boolean;
  underlined?: boolean;
  strikethrough?: boolean;
  obfuscated?: boolean;
}

export type ServerTextures = "ask" | "accept" | "deny";

export interface ServerRow {
  /// Место в файле — для правки, удаления и перестановки.
  index: number;
  name: string;
  address: string;
  /// PNG 64×64 в base64.
  icon: string | null;
  textures: ServerTextures;
  hidden: boolean;
  /// По статистике: сколько играл, когда и какой сборкой.
  played: { seconds: number; last: number; build: string; build_name: string } | null;
}

export interface ServersView {
  servers: ServerRow[];
  /// Сборки со своим списком серверов.
  lists: { build: string; name: string }[];
}

export interface ServerStatus {
  version: string;
  protocol: number | null;
  online: number | null;
  max: number | null;
  sample: string[];
  motd: Span[];
  favicon: string | null;
  ping_ms: number;
  modded: { loader: string | null; mods: number | null } | null;
  legacy: boolean;
}

/// Ответ сервера на опрос (событие «server-status»).
export interface ServerProbe {
  token: number;
  address: string;
  status: ServerStatus | null;
  error: string | null;
  kind: "offline" | "bad" | null;
}

export type Fit = "last" | "exact" | "range" | "unknown" | "no";

/// Сборка для «Играть на сервере».
export interface ServerChoice {
  id: string;
  name: string;
  version: string;
  loader: Loader;
  loader_version: string | null;
  fit: Fit;
  note: string;
}

export const TRASH_DAYS = 30;

export const api = {
  config: () => call<Config>("get_config"),
  dataDirSuggestion: () => call<DirCheck>("data_dir_suggestion"),
  inspectDir: (path: string) => call<DirCheck>("inspect_dir", { path }),
  setDataDir: (path: string) => call<Config>("set_data_dir", { path }),
  tutorial: (action: TutorialAction) => call<Config>("tutorial", { action }),
  dataDirCheck: () => call<DataStatus>("data_dir_check"),
  openDataDir: () => call<void>("open_data_dir"),
  setGame: (game: GameSettings) => call<Config>("set_game", { game }),
  systemMemory: () => call<{ total_mb: number | null; auto_mb: number }>("system_memory"),

  accounts: () => call<Account[]>("accounts_list"),
  setDefaultAccount: (id: string) => call<Account[]>("account_set_default", { id }),
  addOffline: (nick: string) => call<Account[]>("account_add_offline", { nick }),
  addEly: (login: string, password: string, code: string | null) =>
    call<ElyLogin>("account_add_ely", { login, password, code }),
  removeAccount: (id: string) => call<Account[]>("account_remove", { id }),

  play: (id: string, version: string | null = null, force = false, server: string | null = null) =>
    call<ModIssue[] | null>("build_play", { id, version, force, server }),

  servers: (build: string | null) => call<ServersView>("servers_list", { build }),
  serverAdd: (build: string | null, name: string, address: string) =>
    call<ServersView>("server_add", { build, name, address }),
  serverEdit: (
    build: string | null,
    index: number,
    address: string,
    change: { name?: string; address?: string; textures?: ServerTextures },
  ) => call<ServersView>("server_edit", { build, index, address, change }),
  serverRemove: (build: string | null, index: number, address: string) =>
    call<ServersView>("server_remove", { build, index, address }),
  serverMove: (build: string | null, index: number, address: string, to: number) =>
    call<ServersView>("server_move", { build, index, address, to }),
  serversPing: (build: string | null, addresses: string[], token: number) =>
    call<void>("servers_ping", { build, addresses, token }),
  serverChoices: (address: string, version: string | null, modded: boolean) =>
    call<ServerChoice[]>("server_choices", { address, version, modded }),
  verify: (id: string) => call<Checked>("build_verify", { id }),
  modsReport: (id: string, online: boolean) => call<ModsReport>("mods_report", { id, online }),
  modsApply: (id: string, action: ModAction) => call<void>("mods_apply", { id, action }),
  modsRemove: (id: string, file: string) => call<void>("mods_remove", { id, file }),
  modsUpdate: (id: string, only: [string, string][], label: string | null = null) =>
    call<string[]>("mods_update", { id, only, label }),
  modsPin: (id: string, project: string, pinned: boolean) => call<Instance>("mods_pin", { id, project, pinned }),
  worldsCheck: (id: string, version: string | null) => call<RiskyWorld[]>("worlds_check", { id, version }),
  playCancel: () => call<void>("play_cancel"),
  problemsLast: (id: string) => call<ProblemSession | null>("problems_last", { id }),
  problemsCounts: () => call<Record<string, number>>("problems_counts"),
  analyzeLog: (id: string, file: string) => call<Analysis>("analyze_log", { id, file }),
  openDriverPage: () => call<void>("open_driver_page"),
  rulesInfo: () => call<[number, boolean]>("rules_info"),
  setAi: (ai: AiSettings) => call<Config>("set_ai", { ai }),
  skins: () => call<Skin[]>("skins_list"),
  skinPng: (id: string) => call<ArrayBuffer>("skin_png", { id }),
  skinPath: (id: string) => call<string>("skin_path", { id }),
  skinAddFile: (path: string) => call<Skin>("skin_add_file", { path }),
  skinLookup: (nick: string, from: NickSource) => call<Textures>("skin_lookup", { nick, from }),
  skinTexture: (url: string) => call<ArrayBuffer>("skin_texture", { url }),
  skinAddTexture: (url: string, name: string, source: string, page: string | null, slim: boolean | null) =>
    call<Skin>("skin_add_texture", { url, name, source, page, slim }),
  skinAddUrl: (url: string) => call<Skin>("skin_add_url", { url }),
  skinRename: (id: string, name: string) => call<Skin>("skin_rename", { id, name }),
  skinSetSlim: (id: string, slim: boolean) => call<Skin>("skin_set_slim", { id, slim }),
  skinRemove: (id: string) => call<void>("skin_remove", { id }),
  skinExport: (id: string, to: string) => call<void>("skin_export", { id, to }),
  skinCatalog: (query: SkinCatalogQuery) => call<SkinCatalogPage>("skin_catalog", { query }),
  setSkins: (skins: SkinSettings) => call<Config>("set_skins", { skins }),
  skinEdit: (id: string) => call<void>("skin_edit", { id }),
  skinStopWatch: () => call<void>("skin_stop_watch"),
  skinReveal: (id: string) => call<void>("skin_reveal", { id }),
  skinWorn: (account: string) => call<string | null>("skin_worn", { account }),
  skinWear: (account: string, skin: string | null) => call<void>("skin_wear", { account, skin }),
  skinCape: (account: string) => call<ArrayBuffer>("skin_cape", { account }),
  skinCapeSet: (account: string, file: string | null, url: string | null) => call<void>("skin_cape_set", { account, file, url }),
  skinLoaderBuilds: () => call<{ id: string; name: string; has: boolean; vanilla: boolean }[]>("skin_loader_builds"),
  aiKeys: () => call<AiProvider[]>("ai_keys"),
  aiSetKey: (provider: AiProvider, key: string) => call<void>("ai_set_key", { provider, key }),
  aiModels: (ai: AiSettings) => call<{ list: string[]; url: string }>("ai_models", { ai }),
  aiUsedToday: () => call<number>("ai_used_today"),
  aiPreview: (build: string, file: string | null) => call<AiPrepared>("ai_preview", { build, file }),
  aiAsk: (build: string, file: string | null, text: string | null) => call<AiAnswer>("ai_ask", { build, file, text }),
  updatesList: () => call<{ list: BuildUpdates[]; checked: number | null; running: boolean }>("updates_list"),
  updatesCheck: () => call<{ list: BuildUpdates[]; checked: number | null; running: boolean }>("updates_check"),
  setUpdates: (updates: UpdateSettings) => call<Config>("set_updates", { updates }),
  launcherUpdate: () => call<LauncherUpdate>("launcher_update_view"),
  launcherUpdateCheck: () => call<LauncherUpdate>("launcher_update_check"),
  launcherUpdateInstall: () => call<void>("launcher_update_install"),
  packInfo: (id: string) => call<PackInfo>("pack_info", { id }),
  packUpdate: (id: string, version: string) => call<Instance>("pack_update", { id, version }),
  hardwareInfo: () => call<Hardware>("hardware_info"),
  netOnline: () => call<boolean>("net_online"),
  setOptimize: (optimize: OptimizeSettings) => call<Config>("set_optimize", { optimize }),
  openPowerSettings: () => call<void>("open_power_settings"),
  optimizeBuild: (id: string) => call<OptimizeReport>("optimize_build", { id }),
  importScan: () => call<ImportFound[]>("import_scan"),
  importInspect: (path: string) => call<ImportFound[]>("import_inspect", { path }),
  importGuess: (found: ImportFound) => call<ImportFound>("import_guess", { found }),
  importRun: (request: {
    path: string;
    name: string;
    version: string;
    loader: Loader;
    loader_version: string | null;
    worlds: boolean;
  }) => call<Instance>("import_run", { request }),
  exportPreview: (id: string) => call<ExportPreview>("export_preview", { id }),
  exportBuild: (id: string, options: ExportOptions, path: string) =>
    call<{ path: string; size: number; linked: number; embedded: number }>("export_build", { id, options, path }),
  upgradePlan: (id: string, version: string) => call<UpgradePlan>("upgrade_plan", { id, version }),
  upgradeRun: (id: string, choice: UpgradeChoice) => call<Instance>("upgrade_run", { id, choice }),
  snapshots: (id: string) => call<SnapshotInfo[]>("snapshots_list", { id }),
  snapshotTake: (id: string) => call<void>("snapshot_take", { id }),
  snapshotRestore: (id: string, snapshot: string) => call<void>("snapshot_restore", { id, snapshot }),
  snapshotDelete: (id: string, snapshot: string) => call<void>("snapshot_delete", { id, snapshot }),
  copyWorld: (id: string, folder: string, to: string) => call<string>("build_copy_world", { id, folder, to }),
  backupWorld: (id: string, folder: string) => call<string>("build_backup_world", { id, folder }),

  options: () => call<OptionsView>("options_get"),
  setOption: (key: string, value: string) => call<OptionsView>("options_set", { key, value }),
  setShared: (group: Group, shared: boolean) => call<OptionsView>("options_set_shared", { group, shared }),
  seedOptions: (id: string) => call<OptionsView>("options_seed", { id }),

  library: () => call<LibraryItem[]>("library_list"),
  libraryMods: () => call<ModLibrary>("library_mods"),
  libraryModsAdd: (keys: string[], build: string) => call<number>("library_mods_add", { keys, build }),
  libraryModsRemoveUnused: (keys: string[] | null) => call<number>("library_mods_remove_unused", { keys }),
  libraryAdd: (folder: LibraryItem["folder"], paths: string[]) => call<string[]>("library_add", { folder, paths }),
  librarySet: (folder: LibraryItem["folder"], files: string[], allBuilds: boolean, builds: string[]) =>
    call<void>("library_set", { folder, files, allBuilds, builds }),
  libraryRemove: (folder: LibraryItem["folder"], file: string) => call<void>("library_remove", { folder, file }),
  playStatus: () =>
    call<{ status: PlayStatus; finished: Finished | null; blocked?: { id: string; issues: ModIssue[] } | null }>("play_status"),
  playRestart: (closeNow: boolean) => call<void>("play_restart", { closeNow }),
  playRestartCancel: () => call<void>("play_restart_cancel"),

  files: (id: string, folder: Folder) => call<FileEntry[]>("build_files", { id, folder }),
  addFiles: (id: string, folder: Folder, paths: string[]) => call<string[]>("build_add_files", { id, folder, paths }),
  setEnabled: (id: string, folder: Folder, file: string, enabled: boolean) =>
    call<string>("build_set_enabled", { id, folder, file, enabled }),
  worlds: (id: string) => call<World[]>("build_worlds", { id }),
  logs: (id: string) => call<LogFile[]>("build_logs", { id }),
  thumbnail: (id: string, file: string) => call<string>("build_thumbnail", { id, file }),
  screenshot: (id: string, file: string) => call<string>("build_screenshot", { id, file }),
  screenshotFull: (id: string, file: string) => call<string>("build_screenshot_full", { id, file }),
  trashScreenshot: (id: string, file: string) => call<void>("build_trash_screenshot", { id, file }),
  logText: (id: string, file: string) => call<{ text: string; truncated: boolean }>("build_log_text", { id, file }),
  reveal: (id: string, target: OpenTarget) => call<void>("build_reveal", { id, target }),
  open: (id: string, target: OpenTarget) => call<void>("build_open", { id, target }),
  stats: (filter: StatsFilter) => call<Summary>("stats_summary", { filter }),

  versions: (snapshots: boolean) => call<VersionInfo[]>("versions_list", { snapshots }),
  builds: () => call<Entry[]>("builds_list"),
  groups: () => call<string[]>("builds_groups"),
  create: (name: string, version: string, loader: LoaderChoice, group: string | null) =>
    call<Instance>("build_create", { name, version, loader, group }),
  loaderGames: (loader: Loader) => call<string[]>("loader_games", { loader }),
  setCatalog: (catalog: CatalogSettings) => call<Config>("set_catalog", { catalog }),
  catalogSearch: (req: SearchRequest) => call<SearchResult>("catalog_search", { req }),
  catalogProject: (id: string) => call<Project>("catalog_project", { id }),
  catalogVersions: (target: CatalogTarget, kind: ProjectType, project: string, all: boolean) =>
    call<CatalogVersion[]>("catalog_versions", { target, kind, project, all }),
  catalogTranslate: (texts: string[]) => call<string[]>("catalog_translate", { texts }),
  catalogCategories: (kind: ProjectType) =>
    call<{ name: string; project_type: string; header: string }[]>("catalog_categories", { kind }),
  catalogInstalled: (target: CatalogTarget) => call<Installed[]>("catalog_installed", { target }),
  catalogPlan: (target: CatalogTarget, kind: ProjectType, project: string, version: string | null) =>
    call<Plan>("catalog_plan", { target, kind, project, version }),
  catalogInstall: (target: CatalogTarget, kind: ProjectType, versions: string[], label: string | null = null) =>
    call<string[]>("catalog_install", { target, kind, versions, label }),
  modpackInstall: (project: string, version: string | null, label: string | null) =>
    call<Instance>("modpack_install", { project, version, label }),
  mrpackInspect: (path: string) => call<PackInfo>("mrpack_inspect", { path }),
  mrpackImport: (path: string, name: string | null) => call<Instance>("mrpack_import", { path, name }),
  tasksList: () => call<Task[]>("tasks_list"),
  tasksClear: () => call<void>("tasks_clear"),
  loaderVersions: (loader: Loader, game: string) => call<LoaderVersion[]>("loader_versions", { loader, game }),
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
  dataSize: () => call<number>("data_size"),
  dataMovePlan: (path: string, size: number) => call<MovePlan>("data_move_plan", { path, size }),
  dataMove: (to: string) => call<Moved>("data_move", { to }),
  dataMoveCancel: () => call<void>("data_move_cancel"),
  dataRemoveOld: () => call<void>("data_remove_old"),
  setBackgroundImage: (path: string, width: number, height: number) =>
    call<Config>("set_background_image", { path, width, height }),
  removeBackgroundImage: () => call<Config>("remove_background_image"),
  addGroup: (name: string) => call<void>("group_add", { name }),
  deleteGroup: (name: string) => call<void>("group_delete", { name }),
  trashList: () => call<TrashEntry[]>("trash_list"),
  restore: (trashId: string) => call<Instance>("trash_restore", { trashId }),
  purge: (trashId: string) => call<void>("trash_purge", { trashId }),
};
