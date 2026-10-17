<script lang="ts">
  // Каталог Modrinth (раздел 5.4 ТЗ): одна выдача с вкладками, «Куда ставить»
  // вверху, по умолчанию — только то, что подходит к выбранной сборке.
  import NewBuildDialog from "../components/catalog/NewBuildDialog.svelte";
  import ProjectPage from "../components/catalog/ProjectPage.svelte";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";
  import { api, type Hit, type Loader, type ProjectType, type Sort, type VersionInfo } from "../lib/api";
  import { builds, refresh } from "../lib/builds.svelte";
  import { busy, catalog, defaultPick, install, installModpack, installed, refreshInstalled, searchTarget } from "../lib/catalog.svelte";
  import { categoryName, headerName } from "../lib/categories";
  import { ago, count } from "../lib/format";
  import { loaderLabel, loaderTitle } from "../lib/loaders";
  import { openBuild } from "../lib/nav.svelte";

  if (!builds.loaded) refresh();
  $effect(() => {
    if (!catalog.pick && builds.loaded) catalog.pick = defaultPick();
  });

  const TYPES: { id: ProjectType; title: string }[] = [
    { id: "mod", title: "Моды" },
    { id: "resourcepack", title: "Ресурспаки" },
    { id: "shader", title: "Шейдеры" },
    { id: "modpack", title: "Сборки" },
  ];
  const SORTS: { value: Sort; label: string }[] = [
    { value: "relevance", label: "По совпадению" },
    { value: "downloads", label: "По скачиваниям" },
    { value: "follows", label: "По подпискам" },
    { value: "updated", label: "По обновлению" },
    { value: "newest", label: "Сначала новые" },
  ];

  // «Куда ставить».
  const active = $derived(builds.list.filter((b) => b.state === "active"));
  const pick = $derived(catalog.pick);
  const pickKey = $derived(
    !pick ? "" : pick.kind === "build" ? `build:${pick.id}` : pick.kind === "new" ? "new" : "library",
  );
  const targets = $derived([
    ...active.map((b) => ({
      value: `build:${b.id}`,
      label: b.quick ? "Быстрая игра · ванилла" : `${b.name} · ${b.version} · ${loaderLabel(b)}`,
    })),
    {
      value: "new",
      label: pick?.kind === "new" ? `Новая: ${pick.name} · ${pick.game} · ${loaderTitle(pick.loader)}` : "Новая сборка…",
    },
    { value: "library", label: "Только в библиотеку" },
  ]);
  let newDialog = $state(false);
  function choose(v: string) {
    if (v === "new") newDialog = true;
    else if (v === "library") catalog.pick = { kind: "library", game: null, loader: null };
    else catalog.pick = { kind: "build", id: v.slice(6) };
  }
  const build = $derived(pick?.kind === "build" ? builds.list.find((b) => b.id === pick.id) : null);
  const vanillaMods = $derived(
    catalog.kind === "mod" &&
      ((build && build.loader === "vanilla") || (pick?.kind === "new" && pick.loader === "vanilla")),
  );

  // Фильтры для «Только в библиотеку».
  let gameVersions: VersionInfo[] = $state([]);
  api.versions(false).then((v) => (gameVersions = v)).catch(() => {});
  const gameOptions = $derived([
    { value: null as string | null, label: "Любая версия" },
    ...gameVersions.map((v) => ({ value: v.id as string | null, label: v.id })),
  ]);
  const loaderOptions: { value: Loader | null; label: string }[] = [
    { value: null, label: "Любой загрузчик" },
    { value: "fabric", label: "Fabric" },
    { value: "forge", label: "Forge" },
    { value: "neoforge", label: "NeoForge" },
  ];

  // Поиск: с паузой после ввода, «Показать ещё» дописывает выдачу.
  let hits: Hit[] = $state([]);
  let total = $state(0);
  let loading = $state(false);

  // Фильтры (как на Modrinth): категории своего вида проектов, группами.
  let cats = $state<{ name: string; header: string }[]>([]);
  $effect(() => {
    const kind = catalog.kind;
    cats = [];
    api
      .catalogCategories(kind)
      .then((c) => kind === catalog.kind && (cats = c))
      .catch(() => {});
  });
  const groups = $derived.by(() => {
    const out = new Map<string, string[]>();
    for (const c of cats) out.set(c.header, [...(out.get(c.header) ?? []), c.name]);
    // Внутри группы — по русскому названию; разрешения — по размеру.
    for (const [h, list] of out) {
      if (h === "resolutions") list.sort((a, b) => parseInt(a) - parseInt(b));
      else list.sort((a, b) => categoryName(a).localeCompare(categoryName(b), "ru"));
    }
    return [...out];
  });
  const filters = $derived(catalog.categories.length + (catalog.env ? 1 : 0) + (catalog.openSource ? 1 : 0));

  function setKind(kind: ProjectType) {
    if (kind === catalog.kind) return;
    catalog.kind = kind;
    // Категории у каждого вида свои.
    catalog.categories = [];
    catalog.env = null;
  }

  function toggleCategory(name: string, on: boolean) {
    catalog.categories = on ? [...catalog.categories, name] : catalog.categories.filter((c) => c !== name);
  }

  function resetFilters() {
    catalog.categories = [];
    catalog.env = null;
    catalog.openSource = false;
  }
  let error = $state("");
  let generation = 0;

  async function search(offset: number) {
    if (!catalog.pick) return;
    const mine = ++generation;
    loading = true;
    error = "";
    try {
      const r = await api.catalogSearch({
        target: searchTarget(catalog.pick),
        kind: catalog.kind,
        query: catalog.query,
        sort: catalog.sort,
        offset,
        all: catalog.all,
        categories: catalog.categories,
        environment: catalog.kind === "mod" ? catalog.env : null,
        open_source: catalog.openSource,
      });
      if (mine !== generation) return;
      hits = offset ? [...hits, ...r.hits] : r.hits;
      total = r.total_hits;
    } catch (e) {
      if (mine === generation) error = String(e);
    } finally {
      if (mine === generation) loading = false;
    }
  }

  let timer: ReturnType<typeof setTimeout>;
  $effect(() => {
    // Всё, от чего зависит выдача.
    void [catalog.kind, catalog.query, catalog.sort, catalog.all, pickKey, JSON.stringify(catalog.pick)];
    void [catalog.categories.join(","), catalog.env, catalog.openSource, catalog.packGame, catalog.packLoader];
    clearTimeout(timer);
    timer = setTimeout(() => search(0), 300);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    void pickKey;
    refreshInstalled();
  });

  const when = (iso: string) => ago(Math.floor(Date.parse(iso) / 1000));
</script>

{#if catalog.project}
  <ProjectPage id={catalog.project} />
{:else}
  <h1>Каталог Modrinth</h1>

  <nav class="tabs types" aria-label="Что искать">
    {#each TYPES as t (t.id)}
      <button class="tab" class:on={catalog.kind === t.id} onclick={() => setKind(t.id)}>{t.title}</button>
    {/each}
  </nav>

  {#if catalog.kind === "modpack"}
  <div class="bar">
    <span class="muted">Сборка ставится как новая — вместе с модами, их настройками и нужным загрузчиком.</span>
    <span class="grow"></span>
    <div class="filter">
      <Select label="Версия игры" value={catalog.packGame} options={gameOptions} onchange={(v) => (catalog.packGame = v)} />
    </div>
    <div class="filter">
      <Select label="Загрузчик" value={catalog.packLoader} options={loaderOptions} onchange={(v) => (catalog.packLoader = v)} />
    </div>
  </div>
  {:else}
  <div class="bar">
    <span class="label">Куда ставить</span>
    <div class="target"><Select label="Куда ставить" icon="builds" value={pickKey} options={targets} onchange={choose} /></div>
    {#if pick?.kind === "library"}
      <div class="filter">
        <Select
          label="Версия игры"
          value={pick.game}
          options={gameOptions}
          onchange={(v) => (catalog.pick = { ...pick, game: v })}
        />
      </div>
      {#if catalog.kind === "mod"}
        <div class="filter">
          <Select
            label="Загрузчик"
            value={pick.loader}
            options={loaderOptions}
            onchange={(v) => (catalog.pick = { ...pick, loader: v })}
          />
        </div>
      {/if}
    {:else if pick?.kind === "new"}
      <span class="muted small">создастся при первой установке</span>
    {/if}
  </div>
  {/if}

  <div class="bar">
    <label class="search">
      <Icon name="search" size={18} />
      <input
        type="search"
        bind:value={catalog.query}
        placeholder={catalog.kind === "mod" ? "Найти мод: sodium, jei, create…" : "Найти…"}
        spellcheck="false"
      />
    </label>
    <div class="sort"><Select label="Сортировка" icon="sort" value={catalog.sort} options={SORTS} onchange={(v) => (catalog.sort = v)} /></div>
    {#if catalog.kind !== "modpack"}
      <label class="opt" title="Показать и то, что не подходит к выбранной сборке">
        <input type="checkbox" bind:checked={catalog.all} />Показать всё
      </label>
    {/if}
  </div>

  {#if vanillaMods}
    <p class="warn">
      <Icon name="warning" size={18} />В ванильной сборке моды не загрузятся. Выберите загрузчик в настройках сборки
      {#if build}<button class="link" onclick={() => openBuild(build.id)}>Открыть сборку</button>{/if}
    </p>
  {/if}

  <div class="layout">
    <aside class="filters" aria-label="Фильтры">
      <div class="fhead">
        <b>Фильтры</b>
        {#if filters}<button class="link small" onclick={resetFilters}>Сбросить ({filters})</button>{/if}
      </div>
      {#if catalog.kind === "mod"}
        <fieldset>
          <legend>Где работает</legend>
          <label class="fopt"><input type="radio" checked={catalog.env === null} onchange={() => (catalog.env = null)} />Где угодно</label>
          <label class="fopt"><input type="radio" checked={catalog.env === "client"} onchange={() => (catalog.env = "client")} />В игре (клиент)</label>
          <label class="fopt"><input type="radio" checked={catalog.env === "server"} onchange={() => (catalog.env = "server")} />На сервере</label>
        </fieldset>
      {/if}
      {#each groups as [header, names] (header)}
        <fieldset>
          <legend>{headerName(header)}</legend>
          {#each names as name (name)}
            <label class="fopt">
              <input
                type="checkbox"
                checked={catalog.categories.includes(name)}
                onchange={(e) => toggleCategory(name, e.currentTarget.checked)}
              />{categoryName(name)}
            </label>
          {/each}
        </fieldset>
      {/each}
      <fieldset>
        <legend>Другое</legend>
        <label class="fopt"><input type="checkbox" bind:checked={catalog.openSource} />Только с открытым кодом</label>
      </fieldset>
    </aside>
    <div class="results">
    {#if error}
      <p class="warn"><Icon name="error" size={18} />Каталог недоступен: {error}</p>
    {:else if !loading && hits.length === 0}
      <p class="note">Ничего не нашлось{catalog.all ? "" : " для выбранной сборки — попробуйте «Показать всё»"}.</p>
    {:else}
      <p class="muted small found">
        {loading && !hits.length ? "Ищем…" : `Найдено: ${count(total)}${catalog.all || catalog.kind === "modpack" ? "" : " · подходит к выбранной сборке"}`}
      </p>
      <ul class="hits">
        {#each hits as h (h.id)}
          {@const have = installed.get(h.id)}
          <li>
            <button class="open" onclick={() => (catalog.project = h.id)}>
              {#if h.icon_url}
                <img src={h.icon_url} alt="" width="56" height="56" loading="lazy" />
              {:else}
                <span class="noicon"><Icon name={catalog.kind === "shader" ? "shader" : catalog.kind === "resourcepack" ? "resourcepack" : "mods"} size={28} /></span>
              {/if}
              <span class="text">
                <span><b>{h.title}</b> <span class="muted small">· {h.author}</span></span>
                <span class="desc">{h.description}</span>
                <span class="meta muted small">
                  <Icon name="download" size={14} />{count(h.downloads)}
                  <span>· обновлён {when(h.date_modified)}</span>
                  {#each h.display_categories.slice(0, 4) as c}<span class="cat">{categoryName(c)}</span>{/each}
                </span>
              </span>
            </button>
            <div class="act">
              {#if catalog.kind === "modpack"}
              <button class="btn btn-sm" disabled={busy.has(h.id)} onclick={() => installModpack(h.id, null, h.title)}>
                <Icon name="plus" size={16} />{busy.has(h.id) ? "Ставим…" : "Создать сборку"}
              </button>
            {:else if have}
                <span class="have"><Icon name="check" size={16} />{pick?.kind === "library" ? "В библиотеке" : have.enabled ? "Установлен" : "Выключен"}</span>
              {:else}
                <button class="btn btn-sm" disabled={busy.has(h.id) || vanillaMods} onclick={() => install(h.id, catalog.kind)}>
                  <Icon name="download" size={16} />{busy.has(h.id) ? "Ставим…" : "Установить"}
                </button>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
      {#if hits.length < total}
        <div class="more">
          <button class="btn" disabled={loading} onclick={() => search(hits.length)}>{loading ? "Загружаю…" : "Показать ещё"}</button>
        </div>
      {/if}
    {/if}
    </div>
  </div>
{/if}

{#if newDialog}
  <NewBuildDialog
    ondone={(p) => {
      newDialog = false;
      if (p) catalog.pick = p;
    }}
  />
{/if}

<style>
  h1 {
    margin: 0 0 var(--s3);
    font-size: var(--fs-32);
  }
  .types {
    margin-bottom: var(--s4);
  }
  .layout {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: var(--s5);
    align-items: start;
  }
  .filters {
    position: sticky;
    top: 0;
    max-height: calc(100vh - 140px);
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--s4);
    padding-right: var(--s2);
  }
  .fhead {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  fieldset {
    margin: 0;
    padding: 0;
    border: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  legend {
    padding: 0;
    margin-bottom: var(--s1);
    font-size: var(--fs-12);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }
  .fopt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    padding: 3px 0;
    font-size: var(--fs-14);
    cursor: pointer;
  }
  .fopt:hover {
    color: var(--text);
  }
  @media (max-width: 900px) {
    .layout {
      grid-template-columns: 1fr;
    }
    .filters {
      position: static;
      max-height: none;
    }
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--s3);
    margin-bottom: var(--s3);
    flex-wrap: wrap;
  }
  .label {
    font-size: var(--fs-14);
    font-weight: 600;
  }
  .target {
    width: 420px;
  }
  .filter {
    width: 190px;
  }
  .search {
    flex: 1;
    min-width: 240px;
    display: flex;
    align-items: center;
    gap: var(--s2);
    height: var(--h-md);
    padding: 0 var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--surface);
    color: var(--text-muted);
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .search input {
    flex: 1;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
    font: inherit;
    user-select: text;
  }
  .sort {
    width: 200px;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    font-size: var(--fs-14);
    white-space: nowrap;
    cursor: pointer;
  }
  .opt input {
    accent-color: var(--accent);
  }
  .found {
    margin: var(--s2) 0;
  }
  .hits {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--s2);
  }
  .hits li {
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r-card);
    background: var(--surface);
  }
  .open {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: var(--s3);
    align-items: flex-start;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .open:hover b {
    text-decoration: underline;
  }
  .open img,
  .noicon {
    flex: none;
    width: 56px;
    height: 56px;
    border-radius: var(--r);
    background: var(--surface-raised);
    object-fit: cover;
  }
  .noicon {
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }
  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .desc {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    font-size: var(--fs-14);
  }
  .meta {
    display: flex;
    align-items: center;
    gap: var(--s1);
    flex-wrap: wrap;
  }
  .cat {
    padding: 0 var(--s2);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: var(--fs-12);
  }
  .act {
    flex: none;
  }
  .have {
    display: flex;
    align-items: center;
    gap: var(--s1);
    font-size: var(--fs-14);
    color: var(--text-muted);
  }
  .more {
    display: flex;
    justify-content: center;
    margin-top: var(--s4);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
  .warn {
    margin-bottom: var(--s3);
  }
</style>
