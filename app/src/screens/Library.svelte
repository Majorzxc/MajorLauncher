<script lang="ts">
  // Библиотека (разделы 5.4 и 5.18 ТЗ): моды — все моды игрока и где они
  // стоят; ресурспаки и шейдеры хранятся один раз и попадают в сборки при запуске.
  import { open } from "@tauri-apps/plugin-dialog";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";
  import ModLibrary from "../components/ModLibrary.svelte";
  import { api, QUICK_ID, type LibraryItem } from "../lib/api";
  import { builds, refresh } from "../lib/builds.svelte";
  import { date, size } from "../lib/format";
  import { inTauri } from "../lib/tauri";
  import { attempt, toast } from "../lib/toast.svelte";

  type Folder = LibraryItem["folder"];
  let tab: Folder | "mods" = $state("mods");
  let items: LibraryItem[] = $state([]);
  let error = $state("");
  let picking: LibraryItem | null = $state(null);
  let pickAll = $state(true);
  let pickBuilds: string[] = $state([]);

  if (!builds.loaded) refresh();

  async function load() {
    try {
      items = await api.library();
      error = "";
    } catch (e) {
      error = String(e);
    }
  }
  load();

  // Моды из каталога тоже хранятся здесь, но их раздел — в части 5.3.
  const shown = $derived(items.filter((i) => i.folder === tab));
  const activeBuilds = $derived(builds.list.filter((b) => b.state === "active"));
  const buildName = (id: string) => (id === QUICK_ID ? "Быстрая игра" : (builds.list.find((b) => b.id === id)?.name ?? id));

  async function add() {
    if (tab === "mods") return;
    const folder = tab;
    if (!inTauri) return toast("В браузере файлы не выбрать", { error: true });
    const picked = await open({
      multiple: true,
      title: tab === "resourcepacks" ? "Ресурспаки (.zip)" : "Шейдеры (.zip)",
      filters: [{ name: "Архив", extensions: ["zip"] }],
    });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (!paths.length) return;
    const errors = await attempt("Библиотека", () => api.libraryAdd(folder, paths));
    errors?.forEach((e) => toast(e, { error: true }));
    if (errors && errors.length < paths.length) toast(`Добавлено: ${paths.length - errors.length}. Попадут в сборки при запуске.`);
    load();
  }

  async function remove(i: LibraryItem) {
    if (await attempt("Удаление", () => api.libraryRemove(i.folder, i.file).then(() => true))) {
      toast(`«${i.file}» убран из библиотеки и из сборок`);
      load();
    }
  }

  function choose(i: LibraryItem) {
    picking = i;
    pickAll = i.all_builds;
    pickBuilds = [...i.builds];
  }

  async function saveChoice() {
    const i = picking!;
    picking = null;
    if (await attempt("Библиотека", () => api.librarySet(i.folder, i.file, pickAll, pickBuilds).then(() => true))) load();
  }
</script>

<h1>Библиотека</h1>
<p class="muted lead">
  {#if tab === "mods"}
    Все ваши моды: из каталога и свои файлы в сборках. Файл хранится один раз — в сборки он попадает без копии.
    Видно, где стоит каждый мод и какие пустуют.
  {:else}
    Ресурспаки и шейдеры хранятся здесь один раз и при запуске попадают в сборки — без копий и без ручного переноса.
    Включаются они в самой игре. Найти новые — во вкладке «Каталог Modrinth».
  {/if}
</p>

<div class="bar">
  <nav class="tabs" aria-label="Что в библиотеке">
    <button class="tab" class:on={tab === "mods"} onclick={() => (tab = "mods")}>Моды</button>
    <button class="tab" class:on={tab === "resourcepacks"} onclick={() => (tab = "resourcepacks")}>Ресурспаки</button>
    <button class="tab" class:on={tab === "shaderpacks"} onclick={() => (tab = "shaderpacks")}>Шейдеры</button>
  </nav>
  <span class="grow"></span>
  {#if tab !== "mods"}
    <button class="btn btn-primary" onclick={add}><Icon name="plus" size={18} />Добавить .zip</button>
  {/if}
</div>

{#if tab === "mods"}
  <ModLibrary />
{:else if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if shown.length === 0}
  <p class="note">
    {tab === "resourcepacks"
      ? "Ресурспаков пока нет. Добавьте zip-файлы — они появятся во всех сборках."
      : "Шейдеров пока нет. Для них в сборке нужен мод Iris или OptiFine (загрузчики — этап 4)."}
  </p>
{:else}
  <ul class="rows">
    {#each shown as i (i.file)}
      <li>
        <Icon name={tab === "resourcepacks" ? "resourcepack" : "shader"} size={20} />
        <span class="grow">
          {i.file}
          <span class="muted small">
            · {i.all_builds ? "во всех сборках" : i.builds.length ? i.builds.map(buildName).join(", ") : "ни в одной сборке"}
          </span>
        </span>
        <span class="muted small">{size(i.size)}</span>
        <span class="muted small when">{date(i.added)}</span>
        <button class="btn btn-sm" onclick={() => choose(i)}>Где использовать</button>
        <button class="btn btn-sm btn-quiet" aria-label="Убрать {i.file}" title="Убрать из библиотеки и из сборок" onclick={() => remove(i)}
          ><Icon name="trash" size={16} /></button
        >
      </li>
    {/each}
  </ul>
{/if}

{#if picking}
  <Modal title="Где использовать «{picking.file}»" width={480} onclose={() => (picking = null)}>
    <label class="opt"><input type="radio" checked={pickAll} onchange={() => (pickAll = true)} />Во всех сборках</label>
    <label class="opt"><input type="radio" checked={!pickAll} onchange={() => (pickAll = false)} />Только в выбранных</label>
    {#if !pickAll}
      <div class="list">
        {#each activeBuilds as b (b.id)}
          <label class="opt">
            <input
              type="checkbox"
              checked={pickBuilds.includes(b.id)}
              onchange={(e) =>
                (pickBuilds = e.currentTarget.checked ? [...pickBuilds, b.id] : pickBuilds.filter((x) => x !== b.id))}
            />{b.name}
          </label>
        {/each}
      </div>
    {/if}
    <p class="muted small">Файл появится в сборке при её следующем запуске.</p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (picking = null)}>Отмена</button>
      <button class="btn btn-primary" onclick={saveChoice}>Сохранить</button>
    {/snippet}
  </Modal>
{/if}

<style>
  h1 {
    margin: 0 0 var(--s2);
    font-size: var(--fs-32);
  }
  .lead {
    max-width: 760px;
    margin: 0 0 var(--s5);
  }
  .bar .tabs {
    border-bottom: 0;
  }
  .when {
    width: 140px;
    text-align: right;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    margin-bottom: var(--s2);
    cursor: pointer;
  }
  .list {
    max-height: 260px;
    overflow: auto;
    padding-left: var(--s5);
  }
  p {
    margin: var(--s3) 0 0;
  }
</style>
