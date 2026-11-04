// Раздел «Серверы» (раздел 5.10 ТЗ): открытый список (общий или свой у
// сборки) и ответы серверов на опрос. Ответы приходят событием по одному —
// живые серверы видно сразу, не дожидаясь тех, что молчат.

import { listen } from "@tauri-apps/api/event";
import { api, type ServerProbe, type ServersView } from "./api";
import { inTauri } from "./tauri";
import { toast } from "./toast.svelte";

export const servers = $state({
  /// Чей список открыт: `null` — общий, иначе сборка со своим списком.
  build: null as string | null,
  view: null as ServersView | null,
  error: "",
  /// Ответы по адресу (без регистра); "pending" — ждём ответа.
  probes: {} as Record<string, ServerProbe | "pending">,
  /// Сколько опросов идёт.
  pinging: 0,
  /// «Играть на сервере» запустил эту сборку — кнопка сервера показывает запуск.
  launch: null as null | { address: string; build: string },
});

const key = (address: string) => address.trim().toLowerCase();

export function probeOf(address: string): ServerProbe | "pending" | undefined {
  return servers.probes[key(address)];
}

function apply(p: ServerProbe) {
  servers.probes[key(p.address)] = p;
}

let listening: Promise<unknown> | null = null;
function listenOnce() {
  if (!inTauri) return Promise.resolve();
  listening ??= listen<ServerProbe>("server-status", (e) => apply(e.payload));
  return listening;
}

export async function loadServers() {
  try {
    servers.view = await api.servers(servers.build);
    servers.error = "";
  } catch (e) {
    servers.view = null;
    servers.error = String(e);
  }
}

let token = 0;

/// Опросить серверы списка (или только `addresses`).
export async function pingServers(addresses?: string[]) {
  const list = addresses ?? servers.view?.servers.map((s) => s.address) ?? [];
  if (!list.length) return;
  await listenOnce();
  for (const a of list) servers.probes[key(a)] = "pending";
  servers.pinging++;
  try {
    // В браузере (заглушки) ответы приходят сразу списком.
    const replies = (await api.serversPing(servers.build, list, ++token)) as unknown;
    if (Array.isArray(replies)) (replies as ServerProbe[]).forEach(apply);
  } catch (e) {
    toast(`Опрос серверов: ${e}`, { error: true });
    for (const a of list) if (servers.probes[key(a)] === "pending") delete servers.probes[key(a)];
  } finally {
    servers.pinging--;
  }
}

/// Открыть другой список: общий или свой у сборки.
export async function openList(build: string | null) {
  servers.build = build;
  servers.view = null;
  await loadServers();
  pingServers();
}
