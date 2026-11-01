<script lang="ts">
  // Где офлайн-ник увидит свой скин: сборки с модом CustomSkinLoader. Мод
  // берёт скин из файла `CustomSkinLoader/LocalSkin/skins/<ник>.png` — его
  // лаунчер кладёт перед каждым запуском. В сборку без мода — «Поставить».
  import { api } from "../../lib/api";
  import { catalog, install } from "../../lib/catalog.svelte";
  import { SKIN_LOADER } from "../../lib/skins.svelte";

  type B = { id: string; name: string; has: boolean; vanilla: boolean };
  let list = $state<B[] | null>(null);
  let installing = $state<string | null>(null);

  function load() {
    api
      .skinLoaderBuilds()
      .then((l) => (list = l))
      .catch(() => (list = []));
  }
  load();

  /// Поставить мод в сборку через каталог Modrinth (с проверкой версии и
  /// зависимостей). Выбранную в каталоге сборку потом возвращаем как была.
  async function put(b: B) {
    installing = b.id;
    const before = catalog.pick;
    catalog.pick = { kind: "build", id: b.id };
    try {
      await install(SKIN_LOADER, "mod");
    } finally {
      catalog.pick = before;
      installing = null;
      load();
    }
  }

  const withMod = $derived(list?.filter((b) => b.has) ?? []);
  const without = $derived(list?.filter((b) => !b.has && !b.vanilla) ?? []);
</script>

{#if list}
  <div class="builds">
    {#if withMod.length}
      <p class="small">
        Скин виден в игре в сборках:
        <b>{withMod.map((b) => b.name).join(", ")}</b>.
      </p>
    {:else}
      <p class="small warn-text">Пока ни в одной сборке нет мода CustomSkinLoader — без него игра скин не покажет.</p>
    {/if}
    {#if without.length}
      <ul>
        {#each without as b (b.id)}
          <li>
            <span class="grow small">{b.name}</span>
            <button class="btn btn-sm" disabled={installing !== null} onclick={() => put(b)}>
              {installing === b.id ? "Ставлю…" : "Поставить CustomSkinLoader"}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .builds {
    display: flex;
    flex-direction: column;
    gap: var(--s2);
  }
  p {
    margin: 0;
  }
  .warn-text {
    color: var(--warning);
  }
  ul {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
</style>
