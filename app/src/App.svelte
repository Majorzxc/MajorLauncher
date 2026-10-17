<script lang="ts">
  import { onMount } from "svelte";
  import CrashDialog from "./components/CrashDialog.svelte";
  import Header from "./components/Header.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Toasts from "./components/Toasts.svelte";
  import BuildPage from "./screens/BuildPage.svelte";
  import Builds from "./screens/Builds.svelte";
  import FirstRun from "./screens/FirstRun.svelte";
  import Home from "./screens/Home.svelte";
  import CropCover from "./components/CropCover.svelte";
  import BlockedDialog from "./components/BlockedDialog.svelte";
  import Tooltip from "./components/Tooltip.svelte";
  import { goBack, goForward, track } from "./lib/history.svelte";
  import { connectTasks, running, share, tasks } from "./lib/tasks.svelte";
  import PlanDialog from "./components/catalog/PlanDialog.svelte";
  import { catalog } from "./lib/catalog.svelte";
  import { cropper, finish } from "./lib/crop.svelte";
  import Mods from "./screens/Mods.svelte";
  import Placeholder from "./screens/Placeholder.svelte";
  import Settings from "./screens/Settings.svelte";
  import Stats from "./screens/Stats.svelte";
  import { api } from "./lib/api";
  import { size } from "./lib/format";
  import { go, nav } from "./lib/nav.svelte";
  import { connect, play } from "./lib/play.svelte";
  import { ALL } from "./lib/sections";
  import { apply, loadImage } from "./lib/theme.svelte";

  const section = $derived(ALL.find((s) => s.id === nav.section)!);

  let ready = $state(false);
  let needsDataDir = $state(false);
  // Шапка перечитывает аккаунты, когда выбрана папка данных.
  let generation = $state(0);

  // Окно уже показано и покрашено в фон темы (это делает Rust-часть при старте).
  onMount(async () => {
    const config = await api.config();
    apply(config.appearance);
    loadImage();
    needsDataDir = !config.data_dir;
    ready = true;
    connectTasks();
    if (!needsDataDir) connect();
  });

  // История переходов: боковые кнопки мыши и Alt+← / Alt+→.
  $effect(track);
  function mouse(e: MouseEvent) {
    if (e.button === 3 || e.button === 4) {
      e.preventDefault();
      if (e.type === "mouseup") (e.button === 3 ? goBack : goForward)();
    }
  }
  function keys(e: KeyboardEvent) {
    if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      (e.key === "ArrowLeft" ? goBack : goForward)();
    }
  }

  // Нижняя полоса: первая загрузка, у которой известен размер (игра, каталог, обновления).
  const active = $derived(running());
  const downloading = $derived(active.find((t) => t.total > 0) ?? null);
</script>

<div class="app">
  {#key generation}<Header />{/key}
  <Sidebar current={nav.section} onselect={go} />
  <main>
    {#if !ready}
      <!-- пусто: тема ещё применяется -->
    {:else if needsDataDir}
      <FirstRun
        ondone={() => {
          needsDataDir = false;
          generation++;
          connect();
        }}
      />
    {:else if nav.section === "home"}
      <Home />
    {:else if nav.section === "builds" && nav.build}
      {#key nav.build}<BuildPage id={nav.build} />{/key}
    {:else if nav.section === "builds"}
      <Builds />
    {:else if nav.section === "stats"}
      <Stats />
    {:else if nav.section === "mods"}
      <Mods />
    {:else if nav.section === "settings"}
      <Settings />
    {:else}
      <Placeholder {section} />
    {/if}
  </main>
  {#if downloading}
    <!-- Полоса загрузки внизу (раздел 6.1 ТЗ): видна только во время загрузки. -->
    <button class="download" aria-live="polite" title="Открыть «Загрузки»" onclick={() => (tasks.open = true)}>
      <span class="small what">
        {downloading.title} {downloading.place}: {size(downloading.done)} из {size(downloading.total)}
        {#if active.length > 1}<span class="muted"> · ещё {active.length - 1}</span>{/if}
      </span>
      <span class="track"><span style:width="{share(downloading) ?? 0}%"></span></span>
      <b class="small">{share(downloading) ?? 0}%</b>
    </button>
  {/if}
</div>
<svelte:window onmousedown={mouse} onmouseup={mouse} onkeydown={keys} />
<Toasts />
<Tooltip />
{#if catalog.ask}
  <PlanDialog
    plan={catalog.ask.plan}
    ondone={(ok) => {
      const r = catalog.ask!.resolve;
      catalog.ask = null;
      r(ok);
    }}
  />
{/if}
{#if play.blocked}
  <BlockedDialog info={play.blocked} onclose={() => (play.blocked = null)} />
{/if}
{#if cropper.request}
  {#key cropper.request}<CropCover src={cropper.request.src} ondone={finish} />{/key}
{/if}
{#if play.crash}
  <CrashDialog info={play.crash} onclose={() => (play.crash = null)} />
{/if}

<style>
  .app {
    display: grid;
    grid-template-columns: var(--nav-w) minmax(0, 1fr);
    grid-template-rows: var(--header-h) minmax(0, 1fr) auto;
    height: 100vh;
  }
  main {
    grid-area: 2 / 2;
    overflow: auto;
    padding: var(--pad-top) var(--pad-x) var(--s6);
    scrollbar-gutter: stable;
  }
  .download {
    grid-area: 3 / 2;
    width: 100%;
    border: 0;
    color: inherit;
    text-align: left;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: var(--s4);
    height: 44px;
    padding: 0 var(--pad-x);
    background: var(--surface);
    border-top: 1px solid var(--border);
  }
  .track {
    flex: 1;
    height: 6px;
    border-radius: 3px;
    background: var(--surface-raised);
  }
  .track span {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
    transition: width 0.2s linear;
  }
  b {
    width: 44px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
</style>
