// Заглушки команд ядра для проверки вёрстки в браузере (только `npm run dev`).
// Параметры адреса для снимков:
//   ?mode=light&accent=%237c3aed&bg=%23202a44 — оформление
//   ?fresh — первый запуск (папка данных не выбрана)
//   ?missing — папка данных не найдена; ?moved — диск сменил букву
//   ?starting — игра запускается (этап и проценты)
//   ?risky — миры из более новой версии; ?crash&streak — падает подряд, откат
//   ?empty — ни одной сборки

import type { Entry, Instance, TrashEntry, VersionInfo } from "./api";

const url = new URLSearchParams(location.search);
const KEY = "mock-config";
const DB = "mock-builds";
const now = () => Math.floor(Date.now() / 1000);
const DAY = 86400;

function config() {
  const saved = localStorage.getItem(KEY);
  if (saved) return JSON.parse(saved);
  return {
    data_dir: url.has("fresh") ? null : "D:\\MajorLauncherData",
    appearance: { mode: url.get("mode") ?? "system", accent: url.get("accent"), background: url.get("bg") },
    game: { on_launch: "close", memory_mb: null, jvm_args: "" },
  };
}

function saveConfig(c: unknown) {
  localStorage.setItem(KEY, JSON.stringify(c));
}

interface Db {
  builds: (Instance & { archived?: number; cover?: string })[];
  groups: string[];
  trash: TrashEntry[];
}

function inst(id: string, name: string, version: string, extra: Partial<Instance> = {}): Instance {
  return { id, name, version, loader: "vanilla", group: null, pinned: false, created: now() - 40 * DAY, last_played: null, playtime: 0, ...extra };
}

function seed(): Db {
  if (url.has("empty")) return { builds: [], groups: [], trash: [] };
  return {
    groups: ["Серверы", "Тесты"],
    builds: [
      inst("vyzhivanie", "Выживание с друзьями", "1.21.1", { pinned: true, group: "Серверы", last_played: now() - DAY, playtime: 36 * 3600 }),
      inst("tehno", "Техно 1.20.1", "1.20.1", { last_played: now() - 3 * DAY, playtime: 120 * 3600 }),
      inst("vanilla-plus", "Ванилла+", "1.21.1", { last_played: now() - 9 * DAY, playtime: 5 * 3600 }),
      inst("moy-server", "Мой сервер", "1.21.1", { group: "Серверы", last_played: now() - 16 * DAY, playtime: 61 * 3600 }),
      inst("26-3", "26.3", "26.3", { group: "Тесты", created: now() - 2 * DAY }),
      inst("1-12-2", "1.12.2", "1.12.2", { group: "Тесты", last_played: now() - 120 * DAY, playtime: 3600 * 2 }),
      { ...inst("1-18-2", "1.18.2 с модами", "1.18.2", { playtime: 90 * 3600 }), archived: now() - 30 * DAY },
    ],
    trash: [{ trash_id: "old__1", instance: inst("old", "Старый тест", "1.20.4"), deleted_at: now() - 3 * DAY, size: 412 * 1024 * 1024 }],
  };
}

function db(): Db {
  const saved = localStorage.getItem(DB);
  return saved ? JSON.parse(saved) : seed();
}
function save(d: Db) {
  localStorage.setItem(DB, JSON.stringify(d));
}

const VERSIONS: VersionInfo[] = [
  { id: "26.4-snapshot-2", kind: "snapshot", date: "2026-09-29" },
  { id: "26.3", kind: "release", date: "2026-08-12" },
  { id: "26.2", kind: "release", date: "2026-05-20" },
  { id: "1.21.10", kind: "release", date: "2025-10-07" },
  { id: "1.21.8", kind: "release", date: "2025-07-17" },
  { id: "1.21.1", kind: "release", date: "2024-08-08" },
  { id: "1.20.1", kind: "release", date: "2023-06-12" },
  { id: "1.18.2", kind: "release", date: "2022-02-28" },
  { id: "1.12.2", kind: "release", date: "2017-09-18" },
];

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function mock<T>(cmd: string, a: Record<string, any> = {}): Promise<T> {
  const d = db();
  const find = (id: string) => {
    const b = d.builds.find((x) => x.id === id);
    if (!b) throw new Error(`сборка «${id}» не найдена`);
    return b;
  };
  const result = (v: unknown) => {
    save(d);
    return v as T;
  };
  switch (cmd) {
    case "core_version":
      return "0.1.0 (браузер)" as T;
    case "get_config":
      return config() as T;
    case "set_appearance":
      saveConfig({ ...config(), appearance: a.appearance });
      return config() as T;
    case "set_game":
      saveConfig({ ...config(), game: a.game });
      return config() as T;
    case "system_memory":
      return { total_mb: 8150, auto_mb: 2048 } as T;
    case "accounts_list":
    case "account_set_default":
    case "account_add_offline":
    case "account_remove":
      return (url.has("noacc") || !config().data_dir
        ? []
        : [
            { id: "ely-1", kind: "ely", name: "Majorzxc", is_default: true },
            { id: "offline-steve", kind: "offline", name: "Steve", is_default: false },
          ]) as T;
    case "account_add_ely":
      await sleep(500);
      return { status: "need_code" } as T;
    case "worlds_check":
      return (url.has("risky")
        ? [{ folder: "Survival", name: "Выживание", version: "1.21.4" }]
        : []) as T;
    case "play_cancel":
    case "snapshot_take":
    case "snapshot_restore":
    case "snapshot_delete":
      await sleep(300);
      return undefined as T;
    case "snapshots_list": {
      const t = now();
      return [
        { id: "3", created: t - 300, reason: "перед установкой: Iris Shaders", manual: false, version: "1.21.1", loader: "fabric", loader_version: "0.16.10", mods: 162, added: [], removed: [], size: 0, current: true },
        { id: "2", created: t - 2 * DAY, reason: "перед обновлением: Обновление модов (12)", manual: false, version: "1.21.1", loader: "fabric", loader_version: "0.16.10", mods: 161, added: ["iris-fabric-1.8.12+mc1.21.1.jar"], removed: [], size: 0, current: false },
        { id: "1", created: t - 9 * DAY, reason: "сделан вручную", manual: true, version: "1.21.1", loader: "fabric", loader_version: "0.16.5", mods: 158, added: ["iris-fabric-1.8.12+mc1.21.1.jar", "sodium-fabric-0.6.13+mc1.21.1.jar", "lithium-fabric-0.15.1+mc1.21.1.jar", "c2me-fabric-0.3.0.jar"], removed: ["sodium-fabric-0.6.5+mc1.21.1.jar"], size: 412 * 1024, current: false },
      ] as T;
    }
    case "play_status":
      return {
        status: url.has("downloading")
          ? { build: "vyzhivanie", phase: "downloading", done: 141 * 1024 * 1024, total: 402 * 1024 * 1024, stage: "Загрузка файлов игры: 312" }
          : url.has("starting")
            ? { build: "vyzhivanie", phase: "starting", done: 0, total: 0, stage: "Загрузка модов: 162", since: Date.now() - 14000, expected: url.has("first") ? null : 48 }
            : { build: null, phase: "idle", done: 0, total: 0 },
        finished: url.has("crash")
          ? { build: "vyzhivanie", name: "Выживание с друзьями", seconds: 0, code: 1, crashed: true, no_window: true, tail: ["Exception in thread \"main\" java.lang.OutOfMemoryError: Java heap space", "	at net.minecraft.client.main.Main.main(Main.java:42)"], warning: null, streak: url.has("streak") ? 3 : 1, rollback: url.has("streak") ? { id: "3", reason: "перед установкой: Iris Shaders", created: now() - 300 } : null }
          : null,
      } as T;
    case "tasks_list":
      return (url.has("tasks")
        ? [
            { id: 1, title: "Sodium, Iris Shaders", place: "в сборку «Выживание с друзьями»", done: 1.4e6, total: 3.1e6, files_done: 1, files_total: 2, stage: null, state: "running", error: null, started: now() - 5, finished: null },
            { id: 2, title: "Файлы игры 1.21.1 · NeoForge 21.1.257", place: "для сборки «Нео тест»", done: 0, total: 0, files_done: 0, files_total: 0, stage: "Ставим загрузчик 3/6", state: "running", error: null, started: now() - 20, finished: null },
            { id: 3, title: "Обновление: AppleSkin", place: "в сборку «Нео тест»", done: 75e3, total: 75e3, files_done: 1, files_total: 1, stage: null, state: "done", error: null, started: now() - 300, finished: now() - 298 },
            { id: 4, title: "Скачать заново", place: "в сборку «Нео тест»", done: 0, total: 1.1e6, files_done: 0, files_total: 1, stage: null, state: "failed", error: "нет связи с cdn.modrinth.com", started: now() - 600, finished: now() - 590 },
          ]
        : []) as T;
    case "build_log_text": {
      const lines: string[] = [];
      for (let i = 0; i < 3000; i++) {
        const t = `[12:${String(Math.floor(i / 60) % 60).padStart(2, "0")}:${String(i % 60).padStart(2, "0")}]`;
        if (i % 97 === 5) lines.push(`${t} [Render thread/ERROR]: Failed to load texture minecraft:block/${i}`, "java.io.FileNotFoundException: minecraft:block", "\tat net.minecraft.client.Texture.load(Texture.java:42)");
        else if (i % 31 === 3) lines.push(`${t} [Worker-Main-2/WARN]: Missing sound for event: ambient.cave_${i}`);
        else lines.push(`${t} [Render thread/INFO]: Loaded chunk ${i} in 3 ms`);
      }
      return { text: lines.join("\n"), truncated: false } as T;
    }
    case "build_screenshot_full":
      return "data:image/svg+xml;utf8," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080"><rect width="100%" height="100%" fill="#2b6b3a"/><text x="50%" y="50%" fill="#fff" font-size="96" text-anchor="middle">' + a.file + "</text></svg>") as T;
    case "build_trash_screenshot":
    case "build_reveal":
      return undefined as T;
    case "tasks_clear":
      return undefined as T;
    case "build_verify":
      await sleep(1500);
      return { fixed: 2, mods_changed: 1 } as T;
    case "build_play":
      await sleep(400);
      throw new Error("в браузере игру не запустить");
    case "build_files":
      if (a.folder === "screenshots") return Array.from({ length: 5 }, (_, i) => ({ file: `2026-10-0${i + 1}_12.00.00.png`, name: "", size: 2e6, modified: now() - i * DAY, enabled: true, is_dir: false })) as T;
      if (a.folder !== "mods") return [] as T;
      return [
        { file: "sodium-0.6.jar", name: "sodium-0.6.jar", size: 1.2e6, modified: now() - DAY, enabled: true, is_dir: false },
        { file: "journeymap.jar.disabled", name: "journeymap.jar", size: 4.5e6, modified: now() - 5 * DAY, enabled: false, is_dir: false },
      ] as T;
    case "build_set_enabled":
      return a.file as T;
    case "build_worlds":
      return [
        { folder: "New World", name: "Выживание", size: 84e6, last_played: now() - DAY, icon: null },
        { folder: "test", name: "test", size: 3e6, last_played: now() - 20 * DAY, icon: null },
      ] as T;
    case "build_logs":
      return [
        { file: "logs/latest.log", kind: "log", size: 41e3, modified: now() },
        { file: "crash-reports/crash-2026-10-03_09.20.11-client.txt", kind: "crash", size: 12e3, modified: now() - 6 * DAY },
      ] as T;
    case "build_thumbnail":
    case "build_screenshot": {
      // Скриншот для обрезки — побольше и с узором, чтобы было что двигать.
      const big = cmd === "build_screenshot";
      const c = document.createElement("canvas");
      c.width = big ? 1280 : 320;
      c.height = big ? 720 : 180;
      const g = c.getContext("2d")!;
      g.fillStyle = `hsl(${(a.file.length * 37 + a.file.charCodeAt(9) * 50) % 360} 40% 45%)`;
      g.fillRect(0, 0, c.width, c.height);
      if (big) {
        for (let i = 0; i < 14; i++) {
          g.fillStyle = `hsl(${i * 47} 60% ${35 + (i % 4) * 10}%)`;
          g.fillRect((i * 173) % 1180, (i * 97) % 640, 120 + (i % 3) * 60, 80 + (i % 5) * 30);
        }
      }
      return c.toDataURL() as T;
    }
    case "build_open":
      return undefined as T;
    case "stats_summary": {
      if (url.has("nostats")) return { total: 0, sessions: 0, longest: 0, since: null, builds: [], places: [], days: [] } as T;
      const today = Math.floor((Date.now() / 1000 - new Date().getTimezoneOffset() * 60) / DAY);
      const days = [0, 1, 2, 4, 5, 8, 12, 13, 20, 26].map((d, i) => ({ day: today - d, seconds: ((i * 7919) % 9000) + 900 }));
      const total = days.reduce((s, d) => s + d.seconds, 0);
      const seen = { sessions: 3, first: now() - 20 * DAY, last: now() - DAY };
      return {
        total, sessions: 14, longest: 9100, since: now() - 30 * DAY,
        builds: [
          { id: "vyzhivanie", name: "Выживание с друзьями", seconds: Math.round(total * 0.6), sessions: 8 },
          { id: "tehno", name: "Техно 1.20.1", seconds: Math.round(total * 0.3), sessions: 4 },
          { id: "1-12-2", name: "1.12.2", seconds: Math.round(total * 0.1), sessions: 2 },
        ],
        places: [
          { key: "server:play.example.org", kind: "server", name: "play.example.org", seconds: Math.round(total * 0.45), ...seen },
          { key: "server:mc.hypixel.net", kind: "server", name: "mc.hypixel.net", seconds: Math.round(total * 0.05), ...seen },
          { key: "world:vyzhivanie/New World", kind: "world", name: "Выживание", build_name: "Выживание с друзьями", seconds: Math.round(total * 0.35), ...seen },
          { key: "menu", kind: "menu", name: "", seconds: Math.round(total * 0.12), ...seen },
          { key: "world:tehno/test", kind: "world", name: "test", build_name: "Техно 1.20.1", seconds: Math.round(total * 0.08), ...seen },
        ].filter((p) => !a.filter.kind || p.kind === a.filter.kind),
        days,
      } as T;
    }
    case "data_dir_check":
      return {
        missing: url.has("missing") ? "G:\\MajorLauncherData" : null,
        moved: url.has("moved") ? ["G:\\MajorLauncherData", "H:\\MajorLauncherData"] : null,
      } as T;
    case "data_dir_suggestion":
    case "inspect_dir": {
      const path: string = a.path ?? "D:\\MajorLauncherData";
      const warnings = [];
      if (/[^\x00-\x7f]/.test(path)) warnings.push("В пути есть русские буквы или другие не латинские символы: старые версии Forge с такой папкой не запускаются. Лучше выбрать путь латиницей.");
      if (/onedrive/i.test(path)) warnings.push("Папка внутри OneDrive: синхронизация портит файлы игры во время записи. Лучше выбрать папку вне OneDrive.");
      return { path, free_bytes: 412 * 1024 ** 3, has_data: false, writable: !path.startsWith("Z:"), warnings } as T;
    }
    case "data_size":
      return (37.4 * 1024 ** 3) as T;
    case "data_move_plan": {
      const to: string = a.path.trim();
      const error = /^g:/i.test(to) ? "это та же папка" : null;
      return { to, same_disk: /^g:/i.test(to), free: 412 * 1024 ** 3, error } as T;
    }
    case "data_move": {
      const total = 37.4 * 1024 ** 3;
      await sleep(2000);
      saveConfig({ ...config(), data_dir: a.to });
      return { method: "copied", old: "G:\\MajorLauncherData", old_size: total } as T;
    }
    case "data_move_cancel":
    case "data_remove_old":
    case "remove_background_image":
      return (cmd === "remove_background_image" ? config() : undefined) as T;
    case "background_image":
      return null as T;
    case "set_data_dir":
      saveConfig({ ...config(), data_dir: a.path });
      return config() as T;
    case "open_data_dir":
    case "build_open_folder":
      return undefined as T;
    case "versions_list":
      await sleep(300);
      return VERSIONS.filter((v) => v.kind === "release" || a.snapshots) as T;
    case "builds_list":
      return d.builds.map((b) => ({
        ...b,
        state: b.archived ? "archived" : "active",
        has_cover: !!b.cover,
        archive_size: b.archived ? 310 * 1024 * 1024 : null,
      })) as Entry[] as T;
    case "builds_groups":
      return [...new Set([...d.groups, ...d.builds.flatMap((b) => (b.group ? [b.group] : []))])] as T;
    // Каталог: в браузере — настоящий Modrinth (он отвечает и сайтам).
    case "catalog_search": {
      const r = a.req;
      const groups: string[][] = [[`project_type:${r.kind}`], ...r.categories.map((c: string) => [`categories:${c}`])];
      if (r.environment) groups.push([`${r.environment}_side:required`, `${r.environment}_side:optional`]);
      if (r.open_source) groups.push(["open_source:true"]);
      const facets = JSON.stringify(groups);
      const q = new URLSearchParams({ query: r.query, index: r.sort, offset: String(r.offset), limit: "20", facets });
      const found = await (await fetch(`https://api.modrinth.com/v2/search?${q}`)).json();
      return { ...found, hits: found.hits.map((h: { project_id: string }) => ({ ...h, id: h.project_id })) } as T;
    }
    case "catalog_categories": {
      const all = await (await fetch("https://api.modrinth.com/v2/tag/category")).json();
      return all.filter((c: { project_type: string }) => c.project_type === a.kind) as T;
    }
    case "catalog_project":
      return (await (await fetch(`https://api.modrinth.com/v2/project/${a.id}`)).json()) as T;
    case "catalog_versions": {
      const list = await (await fetch(`https://api.modrinth.com/v2/project/${a.project}/version`)).json();
      return list.map((v: object, i: number) => ({ ...v, compatible: i % 3 !== 2 })) as T;
    }
    case "catalog_installed":
      return [] as T;
    case "catalog_plan":
      await sleep(400);
      return {
        items: [
          { project_id: a.project, version_id: "v1", title: "Iris Shaders", icon_url: null, version_number: "1.8.14", size: 2900000, dependency: false, replaces: null },
          { project_id: "AANobbMI", version_id: "v2", title: "Sodium", icon_url: null, version_number: "0.8.13", size: 1200000, dependency: true, replaces: null },
        ],
        present: ["Fabric API"],
        missing: [],
        conflicts: [],
        optional: ["Mod Menu"],
      } as T;
    case "catalog_install":
      await sleep(800);
      return ["Iris Shaders", "Sodium"] as T;
    case "set_catalog":
      return config() as T;
    case "loader_games":
      await sleep(200);
      return (a.loader === "neoforge" ? ["1.20.1", "1.20.4", "1.21.1", "26.3"] : VERSIONS.map((v) => v.id).filter((v) => v !== "1.5.2")) as T;
    case "loader_versions": {
      await sleep(300);
      const base = { fabric: ["0.19.5", "0.19.4", "0.16.10"], forge: ["52.1.2", "52.0.57", "51.0.33"], neoforge: ["21.1.257", "21.1.256-beta"] }[a.loader as "fabric"] ?? [];
      return base.map((version, i) => ({ version, stable: !version.includes("beta"), recommended: i === 0 })) as T;
    }
    case "build_create": {
      const id = a.name.toLowerCase().replace(/[^a-z0-9]+/g, "-") || "build";
      const b = inst(id + "-" + d.builds.length, a.name, a.version, {
        group: a.group,
        created: now(),
        loader: a.loader?.loader ?? "vanilla",
        loader_version: a.loader?.version ?? null,
      });
      d.builds.push(b);
      if (a.group && !d.groups.includes(a.group)) d.groups.push(a.group);
      return result(b);
    }
    case "build_update": {
      const b = find(a.id);
      const { loader, ...rest } = a.patch;
      Object.assign(b, rest);
      if (loader) Object.assign(b, { loader: loader.loader, loader_version: loader.version });
      if (b.group && !d.groups.includes(b.group)) d.groups.push(b.group);
      return result(b);
    }
    case "build_duplicate": {
      await sleep(800);
      const b = find(a.id);
      const copy = { ...b, id: b.id + "-copy", name: `${b.name} (копия)`, playtime: 0, last_played: null, pinned: false, created: now() };
      d.builds.push(copy);
      return result(copy);
    }
    case "build_trash": {
      const b = find(a.id);
      d.builds = d.builds.filter((x) => x !== b);
      d.trash.push({ trash_id: `${b.id}__${now()}`, instance: b, deleted_at: now(), size: 900 * 1024 * 1024 });
      return result(undefined);
    }
    case "build_archive":
      await sleep(1200);
      find(a.id).archived = now();
      return result(undefined);
    case "build_unarchive": {
      await sleep(800);
      const b = find(a.id);
      delete b.archived;
      return result(b);
    }
    case "build_size":
      await sleep(200);
      return ((a.id.length * 397) % 3000) * 1024 * 1024 as T;
    case "build_cover":
      return (find(a.id).cover ?? null) as T;
    case "build_set_cover":
      find(a.id).cover = `data:image/png;base64,${a.pngBase64}`;
      return result(undefined);
    case "build_remove_cover":
      delete find(a.id).cover;
      return result(undefined);
    case "group_add":
      if (!d.groups.includes(a.name)) d.groups.push(a.name);
      return result(undefined);
    case "group_delete":
      d.groups = d.groups.filter((g) => g !== a.name);
      d.builds.forEach((b) => b.group === a.name && (b.group = null));
      return result(undefined);
    case "trash_list":
      return d.trash as T;
    case "trash_restore": {
      const t = d.trash.find((x) => x.trash_id === a.trashId)!;
      d.trash = d.trash.filter((x) => x !== t);
      d.builds.push(t.instance);
      return result(t.instance);
    }
    case "trash_purge":
      d.trash = d.trash.filter((x) => x.trash_id !== a.trashId);
      return result(undefined);
    default:
      throw new Error(`нет заглушки для команды ${cmd}`);
  }
}
