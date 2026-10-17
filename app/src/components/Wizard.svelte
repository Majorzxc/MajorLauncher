<script lang="ts">
  import { api, type Loader, type VersionInfo } from "../lib/api";
  import { LOADERS, loaderTitle } from "../lib/loaders";
  import LoaderVersion from "./LoaderVersion.svelte";
  import { builds } from "../lib/builds.svelte";
  import { pickCover } from "../lib/cover";
  import type { IconName } from "../lib/icons";
  import { attempt } from "../lib/toast.svelte";
  import GroupSelect from "./GroupSelect.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import Select from "./Select.svelte";

  let { onclose, oncreated }: { onclose: () => void; oncreated: (name: string) => void } = $props();

  const STEPS = ["Источник", "Версия", "Оформление", "Готово"];
  let step = $state(0);

  // Шаг 1. Пока работает только пустая сборка (раздел 6.2 ТЗ, остальное — этапы 5–6).
  const SOURCES: { title: string; hint: string; icon: IconName; stage?: string }[] = [
    { title: "Пустая сборка", hint: "Выберете версию игры, моды добавите потом", icon: "plus" },
    { title: "Из каталога Modrinth", hint: "Готовые сборки из каталога", icon: "globe", stage: "5" },
    { title: "Из файла", hint: "Файл .mrpack или .zip", icon: "import", stage: "6" },
    { title: "Импорт из другого лаунчера", hint: "Prism, официальный, TLauncher", icon: "export", stage: "6" },
  ];

  // Шаг 2.
  let snapshots = $state(false);
  let versions: VersionInfo[] = $state([]);
  let versionsError = $state("");
  let filter = $state("");
  let version = $state("");
  // Загрузчик выбирается первым: список версий игры сужается до тех, где он есть.
  let loader: Loader = $state("vanilla");
  let loaderVersion: string | null = $state(null);
  let loaderGames: Set<string> | null = $state(null);
  let loaderError = $state("");

  $effect(() => {
    const l = loader;
    loaderGames = null;
    loaderError = "";
    if (l === "vanilla") return;
    let stale = false;
    api
      .loaderGames(l)
      .then((games) => {
        if (stale) return;
        loaderGames = new Set(games);
      })
      .catch((e) => !stale && (loaderError = String(e)));
    return () => (stale = true);
  });

  $effect(() => {
    const snap = snapshots;
    versionsError = "";
    api
      .versions(snap)
      .then((list) => {
        versions = list;
        if (!version && list.length) version = list.find((v) => v.kind === "release")?.id ?? list[0].id;
      })
      .catch((e) => (versionsError = String(e)));
  });

  // Выбранной версии для загрузчика нет — берём последний релиз, где он есть
  // (списки версий и загрузчика приходят в любом порядке).
  $effect(() => {
    if (loaderGames && versions.length && !loaderGames.has(version)) {
      version = versions.find((v) => v.kind === "release" && loaderGames!.has(v.id))?.id ?? "";
    }
  });

  const shownVersions = $derived(
    versions.filter((v) => v.id.includes(filter.trim()) && (!loaderGames || loaderGames.has(v.id))).slice(0, 200),
  );

  // Шаг 3.
  let name = $state("");
  let nameTouched = $state(false);
  let group: string | null = $state(null);
  let cover: string | null = $state(null);
  $effect(() => {
    if (!nameTouched) name = loader === "vanilla" ? version : `${version} ${loaderTitle(loader)}`;
  });

  let error = $state("");
  let creating = $state(false);

  // «Перенести миры из сборки» — копией; настройки игры и так общие.
  let worldsFrom: string | null = $state(null);
  const sources = $derived([
    { value: null as string | null, label: "Не переносить" },
    ...builds.list
      .filter((b) => b.state === "active")
      .map((b) => ({ value: b.id as string | null, label: b.quick ? "Быстрая игра" : `${b.name} · ${b.version}` })),
  ]);

  const canNext = $derived(
    step === 0 ||
      (step === 1 && !!version && (loader === "vanilla" || !!loaderVersion)) ||
      (step === 2 && name.trim().length > 0) ||
      step === 3,
  );

  async function create() {
    creating = true;
    error = "";
    try {
      const b = await api.create(name, version, { loader, version: loaderVersion }, group);
      if (cover) {
        await api.setCover(b.id, cover);
        builds.covers[b.id] = `data:image/png;base64,${cover}`;
      }
      if (worldsFrom) {
        for (const w of await api.worlds(worldsFrom)) await api.copyWorld(worldsFrom, w.folder, b.id);
      }
      oncreated(b.name);
    } catch (e) {
      error = String(e);
    } finally {
      creating = false;
    }
  }
</script>

<Modal title="Новая сборка" width={720} {onclose}>
  <ol class="steps">
    {#each STEPS as s, i}
      <li class:done={i < step} class:now={i === step}><span>{i + 1}</span>{s}</li>
    {/each}
  </ol>

  <div class="page">
    {#if step === 0}
      <div class="sources">
        {#each SOURCES as s, i}
          <button class="source" class:on={i === 0} disabled={!!s.stage}>
            <Icon name={s.icon} size={24} />
            <b>{s.title}</b>
            <span class="muted">{s.stage ? `Появится на этапе ${s.stage}` : s.hint}</span>
          </button>
        {/each}
      </div>
    {:else if step === 1}
      <div class="versions">
        <div class="loaders" role="radiogroup" aria-label="Загрузчик модов">
          {#each LOADERS as l (l.id)}
            <button class="loader" role="radio" aria-checked={loader === l.id} class:on={loader === l.id} onclick={() => (loader = l.id)}>
              <b>{l.title}</b><span class="muted">{l.hint}</span>
            </button>
          {/each}
        </div>
        <div class="vbar">
          <input type="search" placeholder="Найти версию: 1.21, 26.3…" bind:value={filter} />
          <label class="check"><input type="checkbox" bind:checked={snapshots} />Показывать снапшоты</label>
        </div>
        {#if versionsError || loaderError}
          <p class="error">
            <Icon name="error" size={16} />Не удалось получить список версий: {versionsError || loaderError}
          </p>
        {:else}
          <div class="vlist" role="listbox" aria-label="Версия игры">
            {#each shownVersions as v (v.id)}
              <button role="option" aria-selected={version === v.id} class:on={version === v.id} onclick={() => (version = v.id)}>
                <b>{v.id}</b>
                <span class="muted">{v.kind === "snapshot" ? "снапшот · " : ""}{v.date}</span>
              </button>
            {:else}
              <p class="muted pad">
                {!versions.length || (loader !== "vanilla" && !loaderGames) ? "Загружаю список версий…" : "Нет такой версии"}
              </p>
            {/each}
          </div>
        {/if}
        {#if loader !== "vanilla" && version}
          <LoaderVersion {loader} game={version} bind:value={loaderVersion} />
        {/if}
      </div>
    {:else if step === 2}
      <div class="look">
        <div class="cover">
          {#if cover}<img src={`data:image/png;base64,${cover}`} alt="" />{:else}<span>{name.trim()[0]?.toUpperCase() ?? "?"}</span>{/if}
        </div>
        <div class="fields">
          <label class="field">
            <span>Название</span>
            <input type="search" bind:value={name} maxlength="64" oninput={() => (nameTouched = true)} />
          </label>
          <div class="field">
            <span>Группа</span>
            <GroupSelect bind:value={group} />
          </div>
          <div class="field">
            <span>Миры из другой сборки</span>
            <Select label="Миры из сборки" value={worldsFrom} options={sources} onchange={(v) => (worldsFrom = v)} />
          </div>
          <div class="buttons">
            <button class="btn btn-sm" onclick={async () => (cover = (await attempt("Обложка", pickCover)) ?? cover)}>
              <Icon name="screenshot" size={16} />{cover ? "Другая обложка" : "Выбрать обложку"}
            </button>
            {#if cover}<button class="btn btn-sm btn-quiet" onclick={() => (cover = null)}>Без обложки</button>{/if}
          </div>
        </div>
      </div>
    {:else}
      <dl class="summary">
        <dt>Название</dt><dd>{name}</dd>
        <dt>Версия игры</dt><dd>{version}</dd>
        <dt>Загрузчик</dt><dd>{loader === "vanilla" ? "Без загрузчика" : `${loaderTitle(loader)} ${loaderVersion}`}</dd>
        <dt>Группа</dt><dd>{group ?? "без группы"}</dd>
        <dt>Миры</dt><dd>{worldsFrom ? `копия из «${sources.find((x) => x.value === worldsFrom)?.label}»` : "новые"}</dd>
      </dl>
      <p class="muted">
        Сборка создастся сразу. Игра{loader === "vanilla" ? "" : `, ${loaderTitle(loader)}`} и нужная Java скачаются при
        первом запуске. Ваши настройки игры, список серверов и ресурспаки из библиотеки попадут в неё сами.
      </p>
      {#if error}<p class="error"><Icon name="error" size={16} />{error}</p>{/if}
    {/if}
  </div>

  {#snippet footer()}
    {#if step > 0}<button class="btn btn-quiet" onclick={() => step--}>Назад</button>{/if}
    {#if step < 3}
      <button class="btn btn-primary" disabled={!canNext} onclick={() => step++}>Далее</button>
    {:else}
      <button class="btn btn-primary" disabled={creating} onclick={create}>
        {creating ? "Создаю…" : "Создать сборку"}
      </button>
    {/if}
  {/snippet}
</Modal>

<style>
  .steps {
    display: flex;
    gap: var(--s5);
    margin: 0 0 var(--s5);
    padding: 0;
    list-style: none;
    font-size: var(--fs-14);
    color: var(--text-muted);
  }
  .steps li {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
  .steps span {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    font-size: var(--fs-12);
  }
  .steps .now {
    color: var(--text);
    font-weight: 600;
  }
  .steps .now span,
  .steps .done span {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .page {
    min-height: 330px;
  }
  .sources {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--s3);
  }
  .source,
  .loader {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: var(--s4);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--bg);
    text-align: left;
    cursor: pointer;
  }
  .source :global(.i) {
    margin-bottom: var(--s2);
  }
  .source span,
  .loader span {
    font-size: var(--fs-14);
  }
  .on {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .versions {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
  }
  .vbar {
    display: flex;
    gap: var(--s3);
    align-items: center;
  }
  .vbar input:not([type="checkbox"]),
  .field input {
    flex: 1;
    height: var(--h-md);
    padding: 0 var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--bg);
    outline: none;
    user-select: text;
  }
  .vbar input:focus,
  .field input:focus {
    border-color: var(--accent);
  }
  .check {
    display: flex;
    align-items: center;
    gap: var(--s2);
    font-size: var(--fs-14);
    white-space: nowrap;
    cursor: pointer;
  }
  .check input {
    accent-color: var(--accent);
  }
  .vlist {
    height: 170px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: var(--r);
    padding: var(--s1);
  }
  .vlist button {
    display: flex;
    justify-content: space-between;
    width: 100%;
    height: 36px;
    padding: 0 var(--s3);
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    cursor: pointer;
  }
  .vlist button:hover {
    background: var(--surface-raised);
  }
  .vlist button.on {
    background: var(--accent);
    color: var(--on-accent);
    box-shadow: none;
  }
  .vlist button.on .muted {
    color: inherit;
    opacity: 0.75;
  }
  .vlist span {
    font-size: var(--fs-14);
  }
  .pad {
    padding: var(--s3);
    margin: 0;
  }
  .loaders {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--s2);
  }
  .loader {
    padding: var(--s2) var(--s3);
  }
  .loader span {
    font-size: var(--fs-12);
  }
  .look {
    display: flex;
    gap: var(--s5);
  }
  .cover {
    width: 160px;
    height: 160px;
    flex: none;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: var(--r-card);
    background: var(--surface-raised);
    overflow: hidden;
    font-size: 64px;
    font-weight: 600;
    color: var(--text-muted);
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .fields {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: var(--s4);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field > span {
    font-size: var(--fs-14);
    font-weight: 600;
  }
  .buttons {
    display: flex;
    gap: var(--s2);
  }
  .summary {
    display: grid;
    grid-template-columns: 160px 1fr;
    gap: var(--s2) var(--s4);
    margin: 0 0 var(--s4);
  }
  .summary dt {
    color: var(--text-muted);
  }
  .summary dd {
    margin: 0;
    font-weight: 600;
  }
  .error {
    display: flex;
    gap: var(--s2);
    color: var(--danger);
    font-size: var(--fs-14);
  }
</style>
