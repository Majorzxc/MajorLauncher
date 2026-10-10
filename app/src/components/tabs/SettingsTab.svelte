<script lang="ts">
  // Настройки сборки: загрузчик модов, память и аргументы JVM поверх общих настроек.
  import { api, type BuildSettings, type Config, type Entry, type Loader } from "../../lib/api";
  import { LOADERS, loaderLabel, loaderTitle } from "../../lib/loaders";
  import LoaderVersion from "../LoaderVersion.svelte";
  import { refresh } from "../../lib/builds.svelte";
  import { openSettings } from "../../lib/nav.svelte";
  import { attempt, toast } from "../../lib/toast.svelte";
  import MemoryPicker from "../MemoryPicker.svelte";

  let { entry }: { entry: Entry } = $props();

  let config: Config | null = $state(null);
  let memory: { total_mb: number | null; auto_mb: number } | null = $state(null);
  api.config().then((c) => (config = c));
  api.systemMemory().then((m) => (memory = m));

  // Черновик: сохраняется кнопкой, чтобы не писать файл на каждое движение ползунка.
  const current = $derived<BuildSettings>({
    memory_mb: entry.settings?.memory_mb ?? null,
    jvm_args: entry.settings?.jvm_args ?? null,
    own_options: entry.settings?.own_options ?? false,
    own_servers: entry.settings?.own_servers ?? false,
  });
  let draft: BuildSettings = $state({ memory_mb: null, jvm_args: null, own_options: false, own_servers: false });
  $effect(() => {
    draft = { ...current };
  });
  const changed = $derived(
    draft.memory_mb !== current.memory_mb ||
      draft.jvm_args !== current.jvm_args ||
      draft.own_options !== current.own_options ||
      draft.own_servers !== current.own_servers,
  );

  // Загрузчик меняется отдельной кнопкой: это не мелкая настройка.
  let loader: Loader = $state("vanilla");
  let loaderVersion: string | null = $state(null);
  $effect(() => {
    loader = entry.loader;
    loaderVersion = entry.loader_version ?? null;
  });
  const loaderChanged = $derived(
    loader !== entry.loader || (loader !== "vanilla" && loaderVersion !== (entry.loader_version ?? null)),
  );

  async function saveLoader() {
    const choice = { loader, version: loader === "vanilla" ? null : loaderVersion };
    const ok = await attempt("Загрузчик", () => api.update(entry.id, { loader: choice }).then(() => true));
    if (ok) {
      toast(`Загрузчик: ${loaderLabel({ loader: choice.loader, loader_version: choice.version })}. Поставится при запуске.`);
      refresh();
    }
  }

  const inheritMemory = $derived.by(() => {
    if (!config || !memory) return "Как в общих настройках";
    const mb = config.game.memory_mb ?? memory.auto_mb;
    return `Как в общих настройках — ${mb} МБ${config.game.memory_mb === null ? " (авто)" : ""}`;
  });

  async function saveSettings() {
    const ok = await attempt("Настройки сборки", () => api.update(entry.id, { settings: draft }).then(() => true));
    if (ok) {
      toast("Настройки сборки сохранены");
      refresh();
    }
  }
</script>

{#if !entry.quick}
  <div class="card block">
    <h3>Загрузчик модов</h3>
    <div class="chips" role="radiogroup" aria-label="Загрузчик модов">
      {#each LOADERS as l (l.id)}
        <button class="chip" role="radio" aria-checked={loader === l.id} class:on={loader === l.id} onclick={() => (loader = l.id)}>
          {l.id === "vanilla" ? "Ванилла" : l.title}
        </button>
      {/each}
    </div>
    {#if loader !== "vanilla"}
      <LoaderVersion {loader} game={entry.version} bind:value={loaderVersion} />
    {/if}
    {#if loaderChanged}
      <p class="muted small">
        {loader !== entry.loader && entry.loader !== "vanilla"
          ? `Моды для ${loaderTitle(entry.loader)} с ${loaderTitle(loader)} не работают — их придётся заменить. Миры и настройки останутся.`
          : "Новый загрузчик скачается и поставится при следующем запуске."}
      </p>
      <div>
        <button class="btn btn-primary" disabled={loader !== "vanilla" && !loaderVersion} onclick={saveLoader}>
          Сменить загрузчик
        </button>
      </div>
    {/if}
  </div>
{/if}

<div class="card block">
  <h3>Общее для всех сборок</h3>
  <label class="opt">
    <input type="checkbox" checked={!draft.own_options} onchange={(e) => (draft.own_options = !e.currentTarget.checked)} />
    Общие настройки игры
  </label>
  <p class="muted small">
    FOV, чувствительность, звук, клавиши и прочее — одни на все сборки.
    <button class="link" onclick={() => openSettings("profile")}>Открыть настройки игры</button>
  </p>
  <label class="opt">
    <input type="checkbox" checked={!draft.own_servers} onchange={(e) => (draft.own_servers = !e.currentTarget.checked)} />
    Общий список серверов
  </label>
  <p class="muted small">Добавили сервер в одной сборке — он появится во всех.</p>
</div>

<div class="card block">
  <h3>Память для игры</h3>
  <MemoryPicker
    value={draft.memory_mb}
    inherit={inheritMemory}
    total={memory?.total_mb ?? null}
    onchange={(v) => (draft.memory_mb = v)}
  />
</div>

<div class="card block">
  <h3>Аргументы JVM</h3>
  <label class="opt">
    <input type="radio" checked={draft.jvm_args === null} onchange={() => (draft.jvm_args = null)} />
    Как в общих настройках{config?.game.jvm_args ? `: ${config.game.jvm_args}` : " (нет)"}
  </label>
  <label class="opt">
    <input type="radio" checked={draft.jvm_args !== null} onchange={() => (draft.jvm_args = draft.jvm_args ?? "")} />
    Свои
  </label>
  {#if draft.jvm_args !== null}
    <input class="input mono" bind:value={draft.jvm_args} placeholder="-XX:+UseG1GC" spellcheck="false" />
    <p class="muted small">Через пробел. Неверный аргумент не даст игре запуститься — тогда верните «Как в общих».</p>
  {/if}
</div>

<div class="buttons">
  <button class="btn btn-primary" disabled={!changed} onclick={saveSettings}>Сохранить</button>
</div>

<style>
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
    padding: var(--s5);
    margin-bottom: var(--s4);
    max-width: 720px;
  }
  h3 {
    margin: 0;
    font-size: var(--fs-16);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    cursor: pointer;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s2);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-decoration: underline;
    cursor: pointer;
  }
  .mono {
    font-family: ui-monospace, Consolas, monospace;
  }
  p {
    margin: 0;
  }
</style>
