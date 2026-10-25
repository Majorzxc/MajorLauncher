<script lang="ts">
  // Страница сборки (раздел 6.3 ТЗ): шапка с кнопкой «Играть» и вкладки.
  import Cover from "../components/Cover.svelte";
  import EditBuild from "../components/EditBuild.svelte";
  import ExportDialog from "../components/ExportDialog.svelte";
  import Icon from "../components/Icon.svelte";
  import PlayButton from "../components/PlayButton.svelte";
  import FilesTab from "../components/tabs/FilesTab.svelte";
  import ModsTab from "../components/tabs/ModsTab.svelte";
  import LogsTab from "../components/tabs/LogsTab.svelte";
  import SettingsTab from "../components/tabs/SettingsTab.svelte";
  import ShotsTab from "../components/tabs/ShotsTab.svelte";
  import SnapshotsTab from "../components/tabs/SnapshotsTab.svelte";
  import WorldsTab from "../components/tabs/WorldsTab.svelte";
  import { api } from "../lib/api";
  import { builds, refresh } from "../lib/builds.svelte";
  import { ago, playtime } from "../lib/format";
  import { go, lastTab, nav, optimizing } from "../lib/nav.svelte";
  import { attempt } from "../lib/toast.svelte";
  import { loaderLabel } from "../lib/loaders";

  let { id }: { id: string } = $props();

  type Tab = "mods" | "resourcepacks" | "shaderpacks" | "worlds" | "screenshots" | "snapshots" | "logs" | "settings";
  const TABS: { id: Tab; title: string; hint?: string }[] = [
    { id: "mods", title: "Моды" },
    { id: "resourcepacks", title: "Ресурспаки" },
    { id: "shaderpacks", title: "Шейдеры" },
    { id: "worlds", title: "Миры" },
    { id: "screenshots", title: "Скриншоты" },
    { id: "snapshots", title: "Снапшоты" },
    { id: "logs", title: "Логи" },
    { id: "settings", title: "Настройки" },
  ];
  // Вкладку можно задать при переходе («Моды» из предупреждения на главной).
  // Страница создаётся заново для каждой сборки ({#key} в App) — id не меняется.
  // svelte-ignore state_referenced_locally
  let tab: Tab = $state((nav.buildTab ?? lastTab[id] ?? "worlds") as Tab);
  nav.buildTab = null;
  $effect(() => {
    lastTab[id] = tab;
  });
  // Переход на другую вкладку этой же сборки (страница уже открыта).
  $effect(() => {
    if (nav.buildTab) {
      tab = nav.buildTab as Tab;
      nav.buildTab = null;
    }
  });
  let editing = $state(false);
  let exporting = $state(false);

  if (!builds.loaded) refresh();
  const entry = $derived(builds.list.find((b) => b.id === id && b.state === "active"));
</script>

<button class="btn btn-quiet back" onclick={() => go("builds")}><Icon name="arrow-left" size={18} />Сборки</button>

{#if !builds.loaded}
  <!-- пусто: список сборок загружается -->
{:else if !entry}
  <p class="note">Сборки больше нет — её удалили или отправили в архив.</p>
{:else}
  <header class="head">
    <Cover id={entry.id} size={112} />
    <div class="info">
      <h1>{entry.name}</h1>
      <p class="muted">{entry.quick ? `Быстрая игра · любая ванильная версия, последняя — ${entry.version}` : `${entry.version} · ${loaderLabel(entry)}`}</p>
      <p class="muted small meta">
        <span><Icon name="time" size={16} />{playtime(entry.playtime)} в игре</span>
        <span>{entry.last_played ? `запуск ${ago(entry.last_played)}` : "ещё не запускалась"}</span>
      </p>
    </div>
    <div class="actions">
      {#if !entry.quick}
        <button class="btn" onclick={() => (editing = true)}><Icon name="edit" size={18} />Изменить</button>
      {/if}
      <button class="btn icon-btn" title="Открыть папку сборки" aria-label="Папка" onclick={() => attempt("Папка", () => api.openFolder(entry.id))}>
        <Icon name="folder" size={18} />
      </button>
      {#if !entry.quick}
        <button class="btn icon-btn" title="Экспорт в файл .mrpack" aria-label="Экспорт" onclick={() => (exporting = true)}>
          <Icon name="export" size={18} />
        </button>
        <button
          class="btn"
          title="Оптимизаторы, память, видеокарта — по «Настройки → Оптимизация»"
          onclick={() => (optimizing.build = { id: entry.id, name: entry.name })}
        >
          <Icon name="cpu" size={18} />Оптимизировать
        </button>
      {/if}
      <PlayButton id={entry.id} large />
    </div>
  </header>

  <nav class="tabs" aria-label="Вкладки сборки">
    {#each TABS as t (t.id)}
      <button
        class="tab"
        class:on={tab === t.id}
        disabled={!!t.hint}
        title={t.hint ? `Появится на ${t.hint}` : undefined}
        onclick={() => (tab = t.id)}
      >
        {t.title}{#if t.hint}<small> · {t.hint}</small>{/if}
      </button>
    {/each}
  </nav>

  <div class="body">
    {#if tab === "mods" || tab === "resourcepacks" || tab === "shaderpacks"}
      {#key tab}
        {#if tab === "mods"}<ModsTab {entry} />{:else}<FilesTab id={entry.id} folder={tab} loader={entry.loader} />{/if}
      {/key}
    {:else if tab === "worlds"}
      <WorldsTab id={entry.id} />
    {:else if tab === "screenshots"}
      <ShotsTab id={entry.id} />
    {:else if tab === "snapshots"}
      <SnapshotsTab id={entry.id} />
    {:else if tab === "logs"}
      <LogsTab id={entry.id} />
    {:else if tab === "settings"}
      <SettingsTab {entry} />
    {/if}
  </div>

  {#if exporting}
    <ExportDialog {entry} onclose={() => (exporting = false)} />
  {/if}
  {#if editing}
    <EditBuild entry={entry} onclose={() => (editing = false)} />
  {/if}
{/if}

<style>
  .icon-btn {
    padding: 0 var(--s3);
  }
  .back {
    margin: calc(var(--s2) * -1) 0 var(--s3) calc(var(--s3) * -1);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s5);
    margin-bottom: var(--s5);
  }
  .info {
    flex: 1;
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-size: var(--fs-32);
    line-height: 1.2;
  }
  .info p {
    margin: var(--s1) 0 0;
  }
  .meta {
    display: flex;
    gap: var(--s4);
  }
  .meta span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
  .tab:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .tab small {
    font-size: var(--fs-12);
  }
  .body {
    padding-top: var(--s4);
  }
</style>
