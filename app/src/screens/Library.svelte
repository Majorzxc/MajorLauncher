<script lang="ts">
  // Библиотека (разделы 5.4 и 5.18 ТЗ): моды — все моды игрока и где они
  // стоят; ресурспаки и шейдеры хранятся один раз и сразу раскладываются по сборкам.
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
  /// Для кого выбираем «Где использовать»: один файл или несколько.
  let picking: LibraryItem[] | null = $state(null);
  /// Отмеченные галочками файлы текущей вкладки.
  let selected: string[] = $state([]);
  let removing: LibraryItem[] | null = $state(null);
  $effect(() => {
    void tab;
    selected = [];
  });
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

  const shown = $derived(items.filter((i) => i.folder === tab));
  const chosen = $derived(shown.filter((i) => selected.includes(i.file)));
  const allChosen = $derived(shown.length > 0 && chosen.length === shown.length);
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
    if (errors && errors.length < paths.length) toast(`Добавлено: ${paths.length - errors.length} — уже во всех сборках.`);
    load();
  }

  async function removeConfirmed() {
    const list = removing!;
    removing = null;
    let done = 0;
    for (const i of list) {
      if (await attempt("Удаление", () => api.libraryRemove(i.folder, i.file).then(() => true))) done++;
    }
    if (done) toast(done === 1 ? `«${list[0].file}» убран из библиотеки и из сборок` : `Убрано из библиотеки и из сборок: ${done}`);
    selected = [];
    load();
  }

  // Для нескольких файлов: «во всех», если так у всех; сборки — общие для всех.
  function choose(list: LibraryItem[]) {
    picking = list;
    pickAll = list.every((i) => i.all_builds);
    pickBuilds = activeBuilds.map((b) => b.id).filter((id) => list.every((i) => i.builds.includes(id)));
  }

  async function saveChoice() {
    const list = picking!;
    picking = null;
    const files = list.map((i) => i.file);
    if (await attempt("Библиотека", () => api.librarySet(list[0].folder, files, pickAll, pickBuilds).then(() => true))) {
      toast(pickAll ? "Готово: уже во всех сборках" : pickBuilds.length ? `Готово: уже в выбранных сборках (${pickBuilds.length})` : "Убрано из всех сборок");
      selected = [];
      load();
    }
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
  <div class="pickbar" class:on={chosen.length > 0}>
    <label class="opt all">
      <input type="checkbox" checked={allChosen} onchange={(e) => (selected = e.currentTarget.checked ? shown.map((i) => i.file) : [])} />Выбрать все
    </label>
    {#if chosen.length}
      <span>Выбрано: <b>{chosen.length}</b></span>
      <span class="grow"></span>
      <button class="btn btn-sm btn-primary" onclick={() => choose(chosen)}>Где использовать</button>
      <button class="btn btn-sm btn-quiet" onclick={() => (removing = chosen)}><Icon name="trash" size={16} />Убрать</button>
    {/if}
  </div>
  <ul class="rows">
    {#each shown as i (i.file)}
      {@const where = i.builds.filter((id) => activeBuilds.some((b) => b.id === id))}
      <li class:sel={selected.includes(i.file)}>
        <input
          type="checkbox"
          aria-label="Выбрать {i.file}"
          checked={selected.includes(i.file)}
          onchange={(e) => (selected = e.currentTarget.checked ? [...selected, i.file] : selected.filter((f) => f !== i.file))}
        />
        <Icon name={tab === "resourcepacks" ? "resourcepack" : "shader"} size={20} />
        <span class="grow">
          {i.file}
          <span class="muted small">
            · {i.all_builds ? "во всех сборках" : where.length ? where.map(buildName).join(", ") : "ни в одной сборке"}
          </span>
        </span>
        <span class="muted small">{size(i.size)}</span>
        <span class="muted small when">{date(i.added)}</span>
        <button class="btn btn-sm" onclick={() => choose([i])}>Где использовать</button>
        <button class="btn btn-sm btn-quiet" aria-label="Убрать {i.file}" title="Убрать из библиотеки и из сборок" onclick={() => (removing = [i])}
          ><Icon name="trash" size={16} /></button
        >
      </li>
    {/each}
  </ul>
{/if}

{#if picking}
  <Modal
    title={picking.length === 1 ? `Где использовать «${picking[0].file}»` : `Где использовать: ${picking.length} файла(ов)`}
    width={480}
    onclose={() => (picking = null)}
  >
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
    <p class="muted small">Файлы появятся в сборках сразу; откуда выбор снят — оттуда уберутся. Включаются они в самой игре.</p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (picking = null)}>Отмена</button>
      <button class="btn btn-primary" onclick={saveChoice}>Сохранить</button>
    {/snippet}
  </Modal>
{/if}

{#if removing}
  <Modal title="Убрать из библиотеки?" width={480} onclose={() => (removing = null)}>
    <p>
      {removing.length === 1 ? `«${removing[0].file}»` : `Файлов: ${removing.length}`} — уберётся из библиотеки и из всех сборок, куда его положила
      библиотека. Свои файлы с тем же именем в сборках не тронутся.
    </p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (removing = null)}>Отмена</button>
      <button class="btn btn-primary" onclick={removeConfirmed}>Убрать</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .pickbar {
    display: flex;
    align-items: center;
    gap: var(--s3);
    min-height: 44px;
    padding: var(--s1) var(--s3);
    margin: var(--s3) 0 var(--s2);
    border: 1px solid transparent;
    border-radius: var(--r);
  }
  .pickbar.on {
    border-color: var(--accent);
    background: var(--surface);
  }
  .pickbar .all {
    margin: 0;
  }
  li.sel {
    border-color: var(--accent);
  }
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
