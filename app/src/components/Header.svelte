<script lang="ts">
  import AccountMenu from "./AccountMenu.svelte";
  import Downloads from "./Downloads.svelte";
  import Logo from "./Logo.svelte";
</script>

<div class="logo"><Logo size={48} /></div>

<header>
  <span class="title">MajorLauncher</span>
  <span class="spacer"></span>

  <Downloads />
  <AccountMenu />
</header>

<style>
  .logo {
    grid-area: 1 / 1;
    display: grid;
    place-items: center;
    background: var(--surface);
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }
  header {
    grid-area: 1 / 2;
    display: flex;
    align-items: center;
    gap: var(--s4);
    padding: 0 var(--pad-x) 0 var(--s5);
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }
  .title {
    font-size: var(--fs-24);
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .spacer {
    flex: 1;
  }
</style>
