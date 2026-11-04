<script lang="ts">
  // Раздел «Скины» (раздел 5.9 ТЗ): слева — выбранный скин в 3D, справа —
  // свои скины или каталог. Пока раздел открыт, лаунчер следит за файлом,
  // открытым в редакторе, и принимает перетащенные в окно PNG.
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onDestroy, onMount } from "svelte";
  import { api } from "../lib/api";
  import { acceptDrop, ext } from "../lib/drop.svelte";
  import { forget } from "../lib/skinrender";
  import { loadSkins, skins } from "../lib/skins.svelte";
  import { inTauri } from "../lib/tauri";
  import { toast } from "../lib/toast.svelte";
  import SkinCatalog from "../components/skins/SkinCatalog.svelte";
  import SkinLibrary from "../components/skins/SkinLibrary.svelte";
  import SkinStage from "../components/skins/SkinStage.svelte";

  let library = $state<SkinLibrary>();
  const off: UnlistenFn[] = [];

  acceptDrop(() => ({
    hint: "Скины (PNG 64×64) добавятся в «Мои скины»",
    ondrop: (paths) => {
      const pngs = paths.filter((x) => ext(x) === "png");
      if (!pngs.length) return toast("Перетащите файл скина PNG", { error: true });
      skins.view = "mine";
      // Библиотека могла ещё не отрисоваться после смены вида.
      queueMicrotask(() => library?.addPaths(pngs));
    },
  }));

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
  });

  onDestroy(() => {
    off.forEach((f) => f());
    if (inTauri) api.skinStopWatch().catch(() => {});
  });
</script>

<div class="skins">
  <aside class="left" data-tour="skins-stage">
    <SkinStage />
  </aside>
  <section class="right">
    <div class="views" role="tablist" aria-label="Мои скины или каталог" data-tour="skins-views">
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
</style>
