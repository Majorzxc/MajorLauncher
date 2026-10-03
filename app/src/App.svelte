<script lang="ts">
  import { onMount } from "svelte";
  import Header from "./components/Header.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Toasts from "./components/Toasts.svelte";
  import Builds from "./screens/Builds.svelte";
  import FirstRun from "./screens/FirstRun.svelte";
  import Placeholder from "./screens/Placeholder.svelte";
  import Settings from "./screens/Settings.svelte";
  import { api } from "./lib/api";
  import { ALL, type SectionId } from "./lib/sections";
  import { apply } from "./lib/theme.svelte";

  // Раздел из адреса (#settings) — для снимков в браузере; в окне лаунчера адрес не меняется.
  const fromHash = ALL.find((s) => s.id === location.hash.slice(1))?.id;
  let current: SectionId = $state(fromHash ?? "home");
  const section = $derived(ALL.find((s) => s.id === current)!);

  let ready = $state(false);
  let needsDataDir = $state(false);
  // Шапка перечитывает аккаунт, когда выбрана папка данных.
  let generation = $state(0);

  // Окно уже показано и покрашено в фон темы (это делает Rust-часть при старте).
  onMount(async () => {
    const config = await api.config();
    apply(config.appearance);
    needsDataDir = !config.data_dir;
    ready = true;
  });
</script>

<div class="app">
  {#key generation}<Header />{/key}
  <Sidebar {current} onselect={(id) => (current = id)} />
  <main>
    {#if !ready}
      <!-- пусто: тема ещё применяется -->
    {:else if needsDataDir}
      <FirstRun
        ondone={() => {
          needsDataDir = false;
          generation++;
        }}
      />
    {:else if current === "settings"}
      <Settings />
    {:else if current === "builds"}
      <Builds />
    {:else}
      <Placeholder {section} />
    {/if}
  </main>
</div>
<Toasts />

<style>
  .app {
    display: grid;
    grid-template-columns: var(--nav-w) minmax(0, 1fr);
    grid-template-rows: var(--header-h) minmax(0, 1fr);
    height: 100vh;
  }
  main {
    grid-area: 2 / 2;
    overflow: auto;
    padding: var(--pad-top) var(--pad-x) var(--s6);
    scrollbar-gutter: stable;
  }
</style>
