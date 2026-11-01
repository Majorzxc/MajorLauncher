<script lang="ts">
  // Раздел «Скины» (раздел 5.9 ТЗ): слева — выбранный скин в 3D, справа —
  // свои скины или каталог. Пока раздел открыт, лаунчер следит за файлом,
  // открытым в редакторе, и принимает перетащенные в окно PNG.
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { onDestroy, onMount } from "svelte";
  import { api } from "../lib/api";
  import { forget } from "../lib/skinrender";
  import { loadSkins, skins } from "../lib/skins.svelte";
  import { inTauri } from "../lib/tauri";
  import { toast } from "../lib/toast.svelte";
  import SkinCatalog from "../components/skins/SkinCatalog.svelte";
  import SkinLibrary from "../components/skins/SkinLibrary.svelte";
  import SkinStage from "../components/skins/SkinStage.svelte";

  let library = $state<SkinLibrary>();
  let dropping = $state(false);
  const off: UnlistenFn[] = [];

  onMount(async () => {
    loadSkins();
    if (!inTauri) return;
    off.push(
      await listen<{ id: string; error: string | null }>("skin-changed", async (e) => {
        if (e.payload.error) {
          toast(`Скин после правки не подходит: ${e.payload.error}`, { error: true });
          return;
        }
        forget(e.payload.id);
        await loadSkins();
        const s = skins.list.find((x) => x.id === e.payload.id);
        if (s && skins.selected?.kind === "lib" && skins.selected.skin.id === s.id) skins.selected = { kind: "lib", skin: s };
        skins.version++;
      }),
    );
    off.push(
      await getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") dropping = true;
        else if (p.type === "leave") dropping = false;
        else if (p.type === "drop") {
          dropping = false;
          const pngs = p.paths.filter((x) => x.toLowerCase().endsWith(".png"));
          if (!pngs.length) return toast("Перетащите файл скина PNG", { error: true });
          skins.view = "mine";
          // Библиотека могла ещё не отрисоваться после смены вида.
          queueMicrotask(() => library?.addPaths(pngs));
        }
      }),
    );
  });

  onDestroy(() => {
    off.forEach((f) => f());
    if (inTauri) api.skinStopWatch().catch(() => {});
  });
</script>

<div class="skins" class:dropping>
  <aside class="left">
    <SkinStage />
  </aside>
  <section class="right">
    <div class="views" role="tablist" aria-label="Мои скины или каталог">
      <button class="chip" role="tab" aria-selected={skins.view === "mine"} class:on={skins.view === "mine"} onclick={() => (skins.view = "mine")}>
        Мои скины{skins.list.length ? ` · ${skins.list.length}` : ""}
      </button>
      <button class="chip" role="tab" aria-selected={skins.view === "catalog"} class:on={skins.view === "catalog"} onclick={() => (skins.view = "catalog")}>
        Каталог скинов
      </button>
    </div>
    {#if skins.view === "mine"}
      <SkinLibrary bind:this={library} />
    {:else}
      <SkinCatalog />
    {/if}
  </section>
  {#if dropping}<div class="drop">Отпустите — скин добавится в библиотеку</div>{/if}
</div>

<style>
  .skins {
    position: relative;
    display: grid;
    grid-template-columns: 320px 1fr;
    gap: var(--s5);
    align-items: start;
  }
  .left {
    position: sticky;
    top: 0;
  }
  .right {
    min-width: 0;
  }
  .views {
    display: flex;
    gap: var(--s2);
    margin-bottom: var(--s4);
  }
  .drop {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    border: 2px dashed var(--accent);
    border-radius: var(--r-card);
    background: color-mix(in srgb, var(--bg) 80%, transparent);
    font-size: var(--fs-16);
    font-weight: 600;
    pointer-events: none;
  }
</style>
