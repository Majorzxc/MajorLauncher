<script lang="ts">
  import { onMount, untrack } from "svelte";
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
  import Tour from "./components/Tour.svelte";
  import DropOverlay from "./components/DropOverlay.svelte";
  import { connectDrop } from "./lib/drop.svelte";
  import type { TourId } from "./lib/tours";
  import { sectionOpened, setTutorial } from "./lib/tutorial.svelte";
  import { goBack, goForward, track } from "./lib/history.svelte";
  import { connectTasks, running, share, tasks } from "./lib/tasks.svelte";
  import { connectUpdates } from "./lib/updates.svelte";
  import { loadProblems } from "./lib/problems.svelte";
  import { connectAi } from "./lib/ai.svelte";
  import PlanDialog from "./components/catalog/PlanDialog.svelte";
  import { catalog } from "./lib/catalog.svelte";
  import { cropper, finish } from "./lib/crop.svelte";
  import Mods from "./screens/Mods.svelte";
  import Settings from "./screens/Settings.svelte";
  import Stats from "./screens/Stats.svelte";
  import Servers from "./screens/Servers.svelte";
  import Skins from "./screens/Skins.svelte";
  import { api, type DataStatus } from "./lib/api";
  import { toast } from "./lib/toast.svelte";
  import { size } from "./lib/format";
  import { go, nav, optimizing } from "./lib/nav.svelte";
  import OptimizeDialog from "./components/OptimizeDialog.svelte";
  import { connect, play, start } from "./lib/play.svelte";
  import Modal from "./components/Modal.svelte";
  import { apply, loadImage } from "./lib/theme.svelte";

  let ready = $state(false);
  // Мастер первого запуска или выбор папки данных вместо окна с разделами.
  let first = $state<null | "wizard" | "data">(null);
  let dataDir = $state<string | null>(null);
  // Папка данных пропала: диск отключён или сменил букву, а найти её не вышло.
  let missingDir = $state<string | null>(null);

  // Окно уже показано и покрашено в фон темы (это делает Rust-часть при старте).
  onMount(async () => {
    const config = await api.config();
    apply(config.appearance);
    loadImage();
    setTutorial(config.tutorial);
    dataDir = config.data_dir;
    if (config.data_dir) {
      const status = await api.dataDirCheck().catch(() => null);
      missingDir = status?.missing ?? null;
      if (status) movedNote(status);
    }
    first = !(config.tutorial?.onboarded ?? true) && !missingDir ? "wizard" : !config.data_dir || missingDir ? "data" : null;
    ready = true;
    connectTasks();
    connectUpdates();
    connectAi();
    connectDrop();
    loadProblems();
    if (!first) connect();
  });

  // Подсказки раздела при первом его открытии (если обучение включено).
  const tourId = $derived<TourId>(nav.section === "builds" && nav.build ? "build" : nav.section);
  $effect(() => {
    const id = tourId;
    if (ready && !first) untrack(() => sectionOpened(id));
  });

  /// Диск с данными сменил букву, и лаунчер сам нашёл папку — сказать об этом.
  function movedNote(status: DataStatus) {
    if (status.moved) {
      toast(`Диск с данными сменил букву — папка найдена в ${status.moved[1]}, лаунчер запомнил новый путь.`);
    }
  }

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

{#if !ready}
  <!-- пусто: тема ещё применяется -->
{:else if first}
  <main class="alone">
    <FirstRun
      mode={first}
      missing={missingDir}
      {dataDir}
      onmoved={movedNote}
      ondone={() => {
        first = null;
        missingDir = null;
        connect();
      }}
    />
  </main>
{:else}
<div class="app">
  <Header />
  <Sidebar current={nav.section} onselect={go} />
  <main>
    {#if nav.section === "home"}
      <Home />
    {:else if nav.section === "builds" && nav.build}
      {#key nav.build}<BuildPage id={nav.build} />{/key}
    {:else if nav.section === "builds"}
      <Builds />
    {:else if nav.section === "stats"}
      <Stats />
    {:else if nav.section === "mods"}
      <Mods />
    {:else if nav.section === "servers"}
      <Servers />
    {:else if nav.section === "skins"}
      <Skins />
    {:else if nav.section === "settings"}
      <Settings />
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
{/if}
<svelte:window onmousedown={mouse} onmouseup={mouse} onkeydown={keys} />
<Toasts />
<Tooltip />
<Tour />
<DropOverlay />
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
{#if optimizing.build}
  {@const o = optimizing.build}
  <OptimizeDialog id={o.id} name={o.name} onclose={() => (optimizing.build = null)} />
{/if}
{#if play.risky}
  {@const r = play.risky}
  <Modal title="Миры из более новой версии" width={520} onclose={() => (play.risky = null)}>
    <p>
      Эти миры открывали в версии новее той, что запустится. В старой версии мир может испортиться: пропадут блоки,
      предметы или целые участки.
    </p>
    <ul class="risky">
      {#each r.worlds as w (w.folder)}<li><b>{w.name}</b> <span class="muted">— {w.version}</span></li>{/each}
    </ul>
    <p class="muted small">Перед запуском лаунчер сохранит их копию в папку <code>backups</code> сборки.</p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (play.risky = null)}>Отмена</button>
      <button
        class="btn btn-primary"
        onclick={() => {
          play.risky = null;
          start(r.id, r.version, false, true, r.server ?? null);
        }}>Сохранить копию и играть</button
      >
    {/snippet}
  </Modal>
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
  main.alone {
    height: 100vh;
    padding: 0;
  }
  main {
    grid-area: 2 / 2;
    overflow: auto;
    padding: var(--pad-top) var(--pad-x) var(--s6);
    scrollbar-gutter: stable;
  }
  .risky {
    margin: 0 0 var(--s3);
    padding-left: var(--s5);
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
