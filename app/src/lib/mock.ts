// Заглушки команд ядра для проверки вёрстки в браузере (только `npm run dev`).
// Параметры адреса для снимков:
//   ?mode=light&accent=%237c3aed&bg=%23202a44 — оформление
//   ?fresh — первый запуск (папка данных не выбрана)
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
    case "current_account":
      return (config().data_dir ? { name: "Majorzxc", kind: "Ely.by" } : null) as T;
    case "data_dir_suggestion":
    case "inspect_dir": {
      const path: string = a.path ?? "D:\\MajorLauncherData";
      const warnings = [];
      if (/[^\x00-\x7f]/.test(path)) warnings.push("В пути есть русские буквы или другие не латинские символы: старые версии Forge с такой папкой не запускаются. Лучше выбрать путь латиницей.");
      if (/onedrive/i.test(path)) warnings.push("Папка внутри OneDrive: синхронизация портит файлы игры во время записи. Лучше выбрать папку вне OneDrive.");
      return { path, free_bytes: 412 * 1024 ** 3, has_data: false, writable: !path.startsWith("Z:"), warnings } as T;
    }
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
    case "build_create": {
      const id = a.name.toLowerCase().replace(/[^a-z0-9]+/g, "-") || "build";
      const b = inst(id + "-" + d.builds.length, a.name, a.version, { group: a.group, created: now() });
      d.builds.push(b);
      if (a.group && !d.groups.includes(a.group)) d.groups.push(a.group);
      return result(b);
    }
    case "build_update": {
      const b = find(a.id);
      Object.assign(b, a.patch);
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
