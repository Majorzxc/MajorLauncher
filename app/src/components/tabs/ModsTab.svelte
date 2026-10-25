<script lang="ts">
  // Моды сборки (разделы 5.4 и 5.6 ТЗ): что за мод (Modrinth или сведения
  // из jar), предупреждения с кнопками, обновления и закрепление версии.
  import { play } from "../../lib/play.svelte";
  import RestartDialog from "../RestartDialog.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Entry, type ModAction, type ModInfo, type ModsReport } from "../../lib/api";
  import { refresh } from "../../lib/builds.svelte";
  import { catalog, openCatalog } from "../../lib/catalog.svelte";
  import { size } from "../../lib/format";
  import { actionLabel, runAction } from "../../lib/modactions";
  import { inTauri } from "../../lib/tauri";
  import { attempt, toast } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";
  import Modal from "../Modal.svelte";

  let { entry }: { entry: Entry } = $props();
  const id = $derived(entry.id);

  let report = $state<ModsReport | null>(null);
  let error = $state("");
  let checking = $state(false);
  /// Последний отчёт запрашивали у Modrinth (а не только из кэша).
  let asked = $state(false);
  let busy = $state(false);
  /// Какой мод сейчас обновляется (кнопка в его строке).
  let working = $state<string | null>(null);
  let removing = $state<ModInfo | null>(null);

  // Сначала то, что уже известно (мгновенно), потом — свежее с Modrinth.
  async function load(online: boolean) {
    try {
      if (online) checking = true;
      report = await api.modsReport(id, online);
      asked = online;
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      checking = false;
    }
  }
  $effect(() => {
    void id;
    report = null;
    load(false).then(() => load(true));
  });

  const issues = $derived(report?.issues ?? []);
  const perFile = $derived.by(() => {
    const m = new Map<string, "block" | "warn" | "tip">();
    for (const i of issues) {
      if (!i.file) continue;
      const was = m.get(i.file);
      if (!was || (i.level === "block" && was !== "block") || (i.level === "warn" && was === "tip")) m.set(i.file, i.level);
    }
    return m;
  });
  const updatable = $derived((report?.mods ?? []).filter((m) => m.enabled && m.update && !m.pinned));

  async function act(a: ModAction) {
    busy = true;
    if (a.kind === "update" || a.kind === "redownload") working = a.file;
    try {
      // Новый файл (обновление, установка) надо опознать на Modrinth, иначе он
      // до перезахода на вкладку будет «не опознан».
      const fresh = a.kind === "update" || a.kind === "install" || a.kind === "redownload";
      const file = "file" in a ? a.file : null;
      const name = report?.mods.find((m) => m.file === file)?.name ?? null;
      if (await runAction(id, a, name)) await load(fresh);
    } finally {
      busy = false;
      working = null;
    }
  }

  async function toggle(m: ModInfo) {
    await attempt("Мод", () => api.setEnabled(id, "mods", m.file, !m.enabled));
    load(false);
  }

  async function pin(m: ModInfo) {
    if (!m.modrinth) return;
    if (await attempt("Мод", () => api.modsPin(id, m.modrinth!.project_id, !m.pinned).then(() => true))) {
      toast(m.pinned ? `«${m.name}» снова обновляется` : `«${m.name}» закреплён: «Обновить всё» его пропустит`);
      await refresh();
      load(false);
    }
  }

  async function updateAll() {
    busy = true;
    checking = true;
    const titles = await attempt("Обновление", () => api.modsUpdate(id, [], `Обновление модов (${updatable.length})`));
    busy = false;
    if (titles) toast(titles.length ? `Обновлено: ${titles.join(", ")}` : "Обновлять нечего");
    load(true);
  }

  async function remove(m: ModInfo) {
    removing = null;
    if (await attempt("Мод", () => api.modsRemove(id, m.file).then(() => true))) {
      toast(`«${m.name}» убран из сборки`);
      load(false);
    }
  }

  async function addFiles() {
    if (!inTauri) return toast("В браузере файлы не выбрать", { error: true });
    const picked = await open({ multiple: true, title: "Моды (.jar)", filters: [{ name: "Моды", extensions: ["jar"] }] });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (!paths.length) return;
    const errors = await attempt("Файлы", () => api.addFiles(id, "mods", paths));
    errors?.forEach((e) => toast(e, { error: true }));
    load(true);
  }

  const LEVEL = { block: "error", warn: "warning", tip: "info" } as const;

  // Коротко, что делает код своего файла: «запускает программы, Telegram».
  const SHORT: [RegExp, string][] = [
    [/вебхук|Telegram/, "отправка в Discord/Telegram"],
    [/запускает программы/, "запуск программ"],
    [/нативную/, "нативная библиотека"],
    [/внешних файлов/, "подгрузка классов"],
    [/токен сессии/, "токен сессии"],
    [/расшифровывает/, "шифрование"],
  ];
  function codeSummary(list: ModInfo["suspect"]): string {
    const out = new Set<string>();
    for (const f of list) out.add(SHORT.find(([re]) => re.test(f.what))?.[1] ?? f.what);
    return [...out].join(", ");
  }
  let restarting = $state(false);
</script>

{#if entry.loader === "vanilla"}
  <p class="warn">
    <Icon name="warning" size={18} />В ванильной сборке моды не загрузятся: выберите загрузчик на вкладке «Настройки».
  </p>
{/if}

{#if play.status.build === entry.id && play.status.phase === "running"}
  <p class="running">
    <Icon name="info" size={18} />
    <span class="grow">Игра запущена: добавленные и выключенные сейчас моды применятся при следующем запуске.</span>
    {#if !play.status.restart}
      <button class="btn btn-sm" onclick={() => (restarting = true)}><Icon name="refresh" size={16} />Перезапустить и вернуться</button>
    {/if}
  </p>
{/if}
{#if restarting}<RestartDialog onclose={() => (restarting = false)} />{/if}

{#if issues.length}
  <ul class="issues" aria-label="Предупреждения">
    {#each issues as i, n (n)}
      <li class="lvl-{i.level}">
        <Icon name={LEVEL[i.level]} size={18} />
        <span class="grow">{i.text}</span>
        {#if i.action}
          <button class="btn btn-sm" disabled={busy} onclick={() => act(i.action!)}>{actionLabel(i.action)}</button>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<div class="bar">
  <span class="muted small grow">
    {#if report}{report.mods.length} · {report.mods.filter((m) => m.enabled).length} включено{/if}
    {#if checking}· проверяем на Modrinth…{:else if report && asked && !report.online}· Modrinth не ответил — показано известное{/if}
  </span>
  {#if updatable.length}
    <button class="btn btn-sm" disabled={busy} onclick={updateAll}><Icon name="refresh" size={16} />Обновить всё ({updatable.length})</button>
  {/if}
  <button class="btn btn-sm btn-primary" onclick={() => openCatalog(id, "mod")}><Icon name="search" size={16} />Каталог</button>
  <button class="btn btn-sm" onclick={addFiles}><Icon name="plus" size={16} />Добавить файл</button>
  <button class="btn btn-sm" onclick={() => attempt("Папка", () => api.open(id, { kind: "folder", name: "mods" }))}>
    <Icon name="folder" size={16} />Папка
  </button>
</div>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if report && report.mods.length === 0}
  <p class="note">Модов нет. Найдите их в каталоге Modrinth или добавьте свои файлы .jar.</p>
{:else if report}
  <ul class="rows">
    {#each report.mods as m (m.file)}
      {@const mark = perFile.get(m.file)}
      <li class:off={!m.enabled}>
        {#if m.modrinth?.icon_url}
          <img src={m.modrinth.icon_url} alt="" width="32" height="32" loading="lazy" />
        {:else}
          <span class="noicon"><Icon name="mods" size={20} /></span>
        {/if}
        <span class="grow info">
          <span class="name">
            {#if m.modrinth}
              <button class="link" onclick={() => { openCatalog(id, "mod"); catalog.project = m.modrinth!.project_id; }}>{m.name}</button>
            {:else}{m.name}{/if}
            {#if mark}<span class="dot {mark}" title="Есть предупреждение — см. список выше"></span>{/if}
          </span>
          <span class="muted small">
            {m.version || "версия неизвестна"} ·
            {m.malware ? "вредоносный код!" : m.modrinth ? "Modrinth" : m.trusted ? "ваш файл" : "не опознан"}
            {#if !m.malware && !m.modrinth && m.suspect.length && !m.suspect.some((f) => f.strength === "strong")}
              · <span class="code" title={[...new Set(m.suspect.map((f) => f.what))].join("; ")}>в коде: {codeSummary(m.suspect)}</span>
            {/if}
            {#if !m.malware && m.suspect.some((f) => f.strength === "strong")}· <span class="sus" title={m.suspect.map((f) => `${f.what} — ${f.at}`).join("; ")}>подозрительный код</span>{/if} · {size(m.size)}
          </span>
        </span>
        {#if m.update && m.enabled}
          <button
            class="btn btn-sm"
            disabled={busy}
            title={m.pinned ? "Закреплён — «Обновить всё» его пропускает" : "Обновить"}
            onclick={() => act({ kind: "update", version_id: m.update!.version_id, file: m.file })}
          >
            {#if working === m.file}
              Обновляю…
            {:else}
              <Icon name="download" size={14} />{m.update.version_number}
            {/if}
          </button>
        {/if}
        {#if m.modrinth}
          <button
            class="btn btn-sm btn-quiet pin"
            class:on={m.pinned}
            aria-label={m.pinned ? `Открепить ${m.name}` : `Закрепить версию ${m.name}`}
            title={m.pinned ? "Версия закреплена — не обновлять" : "Закрепить версию (не обновлять)"}
            onclick={() => pin(m)}><Icon name="pin" size={16} /></button
          >
        {/if}
        <button
          class="btn btn-sm btn-quiet"
          aria-label="Убрать {m.name} из сборки"
          title="Убрать из сборки"
          onclick={() => (removing = m)}><Icon name="trash" size={16} /></button
        >
        <label class="switch" title={m.enabled ? "Выключить" : "Включить"}>
          <input type="checkbox" checked={m.enabled} onchange={() => toggle(m)} aria-label="Мод {m.name} включён" />
          <span></span>
        </label>
      </li>
    {/each}
  </ul>
{:else}
  <p class="muted">Проверяю моды…</p>
{/if}

{#if removing}
  <Modal title="Убрать «{removing.name}»?" width={480} onclose={() => (removing = null)}>
    <p class="muted">
      {removing.malware
        ? "Файл с вредоносным кодом удалится совсем."
        : "Мод уберётся из сборки. Сам файл останется в библиотеке — его можно будет вернуть."}
    </p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (removing = null)}>Отмена</button>
      <button class="btn btn-primary" onclick={() => remove(removing!)}>Убрать</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .running {
    display: flex;
    align-items: center;
    gap: var(--s2);
    margin: 0 0 var(--s3);
    padding: var(--s2) var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    font-size: var(--fs-14);
  }
  .code {
    text-decoration: underline dotted;
    cursor: help;
  }
  .sus {
    color: var(--warning, #e0a43a);
    text-decoration: underline dotted;
  }
  .issues {
    list-style: none;
    margin: 0 0 var(--s4);
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--s2);
  }
  .issues li {
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: var(--s2) var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--surface);
    font-size: var(--fs-14);
  }
  .issues .grow {
    white-space: normal;
  }
  .lvl-block {
    border-color: var(--danger) !important;
  }
  .lvl-block :global(.i) {
    flex: none;
    color: var(--danger);
  }
  .lvl-warn :global(.i) {
    flex: none;
    color: var(--warning);
  }
  .lvl-tip :global(.i) {
    flex: none;
    color: var(--text-muted);
  }
  .rows img,
  .noicon {
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: var(--r-sm);
  }
  .noicon {
    display: grid;
    place-items: center;
    background: var(--surface-raised);
    color: var(--text-muted);
  }
  .info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-muted);
  }
  .dot.block {
    background: var(--danger);
  }
  .dot.warn {
    background: var(--warning);
  }
  .pin {
    color: var(--text-muted);
  }
  .pin.on {
    color: var(--accent);
  }
  .off .name {
    color: var(--text-muted);
    text-decoration: line-through;
  }
  p {
    margin: 0 0 var(--s3);
  }
</style>
