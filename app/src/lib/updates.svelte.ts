// Фоновые обновления (раздел 5.13 ТЗ): что нашёл лаунчер, когда проверял и
// идёт ли проверка сейчас. Ядро присылает событие «updates», о новом —
// «updates-found», о поставленном сам — «updates-applied».

import { listen } from "@tauri-apps/api/event";
import { api, type BuildUpdates } from "./api";
import { refresh } from "./builds.svelte";
import { go } from "./nav.svelte";
import { inTauri } from "./tauri";
import { toast } from "./toast.svelte";

export const updates = $state({
  list: [] as BuildUpdates[],
  checked: null as number | null,
  running: false,
});

/// Сколько обновлений у сборки (модов и сама сборка из каталога).
export function updateCount(id: string): number {
  const b = updates.list.find((u) => u.build === id);
  return b ? b.mods + (b.pack ? 1 : 0) : 0;
}

export async function connectUpdates() {
  if (inTauri) {
    await listen<typeof updates>("updates", (e) => Object.assign(updates, e.payload));
    await listen<string>("updates-found", (e) =>
      toast(e.payload, { action: { label: "Сборки", run: () => go("builds") } }),
    );
    await listen<string>("updates-applied", (e) => {
      toast(e.payload);
      refresh();
    });
  }
  Object.assign(updates, await api.updatesList().catch(() => ({ list: [], checked: null, running: false })));
}

export async function checkNow() {
  updates.running = true;
  Object.assign(updates, await api.updatesCheck());
}
