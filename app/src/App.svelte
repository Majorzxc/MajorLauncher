<script lang="ts">
  import { onMount } from "svelte";
  import { call } from "./lib/tauri";
  import Header from "./components/Header.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Placeholder from "./screens/Placeholder.svelte";
  import Settings from "./screens/Settings.svelte";
  import { ALL, type SectionId } from "./lib/sections";
  import { apply, type Appearance } from "./lib/theme.svelte";

  // Раздел из адреса (#settings) — для снимков в браузере; в окне лаунчера адрес не меняется.
  const fromHash = ALL.find((s) => s.id === location.hash.slice(1))?.id;
  let current: SectionId = $state(fromHash ?? "home");
  const section = $derived(ALL.find((s) => s.id === current)!);

  // Окно уже показано и покрашено в фон темы (это делает Rust-часть при старте).
  onMount(async () => {
    const config = await call<{ appearance: Appearance }>("get_config");
    apply(config.appearance);
  });
</script>

<div class="app">
  <Header />
  <Sidebar {current} onselect={(id) => (current = id)} />
  <main>
    {#if current === "settings"}
      <Settings />
    {:else}
      <Placeholder {section} />
    {/if}
  </main>
</div>

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
