<script lang="ts">
  // Импорт сборки из другого лаунчера: Prism, официальный, TLauncher,
  // CurseForge, Modrinth App или любая папка игры. Файлы другого лаунчера
  // только копируются — в его папках ничего не меняется.
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type ImportFound, type Loader, type VersionInfo } from "../lib/api";
  import { refresh } from "../lib/builds.svelte";
  import { count, plural, size } from "../lib/format";
  import { LOADERS, loaderLabel } from "../lib/loaders";
  import { openBuild } from "../lib/nav.svelte";
  import { inTauri } from "../lib/tauri";
  import { toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import Select from "./Select.svelte";

  // `onmrpack` — «Из файла .mrpack»: этот импорт живёт на экране «Сборки».
  let { onclose, onmrpack }: { onclose: () => void; onmrpack: () => void } = $props();

  let list = $state<ImportFound[] | null>(null);
  let error = $state("");
  api
    .importScan()
    .then((l) => (list = l))
    .catch((e) => (error = String(e)));

  async function pickFolder() {
    if (!inTauri) return;
    const path = await open({ directory: true, title: "Папка сборки другого лаунчера" });
    if (typeof path !== "string") return;
    error = "";
    try {
      const found = await api.importInspect(path);
      const known = new Set((list ?? []).map((f) => f.path + "|" + f.name));
      list = [...found.filter((f) => !known.has(f.path + "|" + f.name)), ...(list ?? [])];
      if (found.length === 1) choose(found[0]);
    } catch (e) {
      error = String(e);
    }
  }

  // Выбранная сборка и что из неё сделать.
  let chosen = $state<ImportFound | null>(null);
  let guessing = $state(false);
  let name = $state("");
  let version = $state<string | null>(null);
  let loader = $state<Loader>("vanilla");
  let worlds = $state(true);
  let versions = $state<VersionInfo[]>([]);
  api.versions(true).then((v) => (versions = v)).catch(() => {});

  async function choose(f: ImportFound) {
    if (f.problem) return;
    chosen = f;
    name = f.name;
    version = f.version;
    loader = f.loader ?? "vanilla";
    worlds = true;
    if (!f.version || !f.loader) {
      guessing = true;
      try {
        const g = await api.importGuess(f);
        if (chosen === f) {
          chosen = g;
          version = g.version;
          loader = g.loader ?? "vanilla";
        }
      } catch (e) {
        error = `Не удалось определить версию по модам: ${e}`;
      } finally {
        guessing = false;
      }
    }
  }

  const options = $derived.by(() => {
    const out = versions.map((v) => ({ value: v.id as string | null, label: v.kind === "snapshot" ? `${v.id} · снапшот` : v.id }));
    if (version && !out.some((o) => o.value === version)) out.unshift({ value: version, label: version });
    return [{ value: null as string | null, label: "Выберите версию…" }, ...out];
  });
  // Версию загрузчика берём из того лаунчера, если загрузчик не поменяли.
  const loaderVersion = $derived(chosen && loader === chosen.loader && version === chosen.version ? chosen.loader_version : null);

  let running = $state(false);
  async function run() {
    if (!chosen || !version) return;
    running = true;
    error = "";
    try {
      const created = await api.importRun({
        path: chosen.path,
        name: name.trim() || chosen.name,
        version,
        loader,
        loader_version: loaderVersion,
        worlds,
      });
      toast(`Сборка «${created.name}» импортирована — в ${chosen.source_title} ничего не изменилось`);
      await refresh();
      onclose();
      openBuild(created.id);
    } catch (e) {
      error = String(e);
    } finally {
      running = false;
    }
  }

  const mods = (n: number) => `${count(n)} ${plural(n, ["мод", "мода", "модов"])}`;
  const worldsText = (f: ImportFound) => `${count(f.worlds)} ${plural(f.worlds, ["мир", "мира", "миров"])}`;
</script>

<Modal title="Импорт сборки" width={680} closable={!running} {onclose}>
  {#if !chosen}
    <p class="muted small">
      Лаунчер ищет сборки Prism и MultiMC, официального лаунчера и TLauncher, CurseForge и Modrinth App. Файлы того
      лаунчера только копируются — в нём ничего не изменится.
    </p>
    {#if !list && !error}
      <p class="muted small"><span class="spin"></span>Ищу сборки…</p>
    {:else if list && list.length === 0}
      <p class="note">Сборок других лаунчеров не нашлось. Укажите папку сборки вручную.</p>
    {:else if list}
      <ul class="found">
        {#each list as f (f.path + f.name)}
          <li>
            <button class="row" disabled={!!f.problem} onclick={() => choose(f)}>
              <span class="grow">
                <b>{f.name}</b>
                <span class="muted small">
                  · {f.source_title} ·
                  {f.version ? `${f.version}${f.loader && f.loader !== "vanilla" ? " · " + loaderLabel({ loader: f.loader, loader_version: f.loader_version }) : ""}` : "версия определится по модам"}
                </span>
              </span>
              <span class="muted small">{mods(f.mods)}{f.worlds ? ` · ${worldsText(f)}` : ""}</span>
              {#if f.problem}<span class="bad small">{f.problem}</span>{:else}<Icon name="chevron-right" size={18} />{/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
    <p>
      <b>{chosen.name}</b> <span class="muted small">· {chosen.source_title} · {mods(chosen.mods)}</span>
    </p>
    <p class="muted small path">{chosen.path}</p>
    <label class="field"><span>Название сборки</span><input class="input" bind:value={name} maxlength="64" /></label>
    <div class="field">
      <span>Версия игры</span>
      <Select label="Версия игры" value={version} {options} onchange={(v) => (version = v)} />
    </div>
    <div class="field">
      <span>Загрузчик</span>
      <div class="chips" role="radiogroup" aria-label="Загрузчик">
        {#each LOADERS as l (l.id)}
          <button class="chip" role="radio" aria-checked={loader === l.id} class:on={loader === l.id} onclick={() => (loader = l.id)}>
            {l.id === "vanilla" ? "Ванилла" : l.title}
          </button>
        {/each}
      </div>
    </div>
    {#if guessing}
      <p class="muted small"><span class="spin"></span>Определяю версию и загрузчик по модам…</p>
    {:else if chosen.guessed}
      <p class="warn-text small">
        <Icon name="info" size={16} />{chosen.source_title} не хранит версию рядом со сборкой — она определена по модам.
        Проверьте, совпадает ли с той, на которой вы играли.
      </p>
    {/if}
    {#if loader !== "vanilla"}
      <p class="muted small">
        {loaderVersion ? `Версия загрузчика — как в ${chosen.source_title}: ${loaderVersion}.` : "Версия загрузчика — рекомендованная для этой версии игры."}
      </p>
    {/if}
    {#if chosen.worlds}
      <label class="opt">
        <input type="checkbox" bind:checked={worlds} />Скопировать миры — {worldsText(chosen)}, {size(chosen.worlds_size)}
      </label>
    {/if}
    {#if running}<p class="muted small"><span class="spin"></span>Копирую файлы… Прогресс — в «Загрузках».</p>{/if}
  {/if}
  {#if error}<p class="bad"><Icon name="error" size={18} />{error}</p>{/if}

  {#snippet footer()}
    {#if chosen}
      <button class="btn btn-quiet back" disabled={running} onclick={() => (chosen = null)}>Назад</button>
      <button class="btn btn-quiet" disabled={running} onclick={onclose}>Отмена</button>
      <button class="btn btn-primary" disabled={running || guessing || !version || !name.trim()} onclick={run}>
        <Icon name="import" size={18} />Импортировать
      </button>
    {:else}
      <button class="btn" onclick={pickFolder}><Icon name="folder" size={18} />Выбрать папку…</button>
      <button class="btn back" title="Сборка Modrinth, Prism или Modrinth App в одном файле" onclick={onmrpack}
        ><Icon name="import" size={18} />Из файла .mrpack…</button
      >
      <button class="btn btn-quiet" onclick={onclose}>Закрыть</button>
    {/if}
  {/snippet}
</Modal>

<style>
  p {
    margin: 0 0 var(--s3);
  }
  .found {
    max-height: 360px;
    overflow: auto;
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: var(--r-card);
  }
  .found li + li {
    border-top: 1px solid var(--border);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--s3);
    width: 100%;
    padding: var(--s3) var(--s4);
    border: 0;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover:not(:disabled) {
    background: var(--surface-raised);
  }
  .row:disabled {
    cursor: default;
    opacity: 0.7;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .path {
    font-family: Consolas, monospace;
    word-break: break-all;
    user-select: text;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: var(--s3);
    font-size: var(--fs-14);
    font-weight: 600;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s2);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    margin-bottom: var(--s3);
    cursor: pointer;
  }
  .warn-text {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    color: var(--warning);
  }
  .bad {
    display: flex;
    gap: 6px;
    color: var(--danger);
  }
  .back {
    margin-right: auto;
  }
  .spin {
    display: inline-block;
    width: 12px;
    height: 12px;
    margin-right: 8px;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    vertical-align: -1px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
