<script lang="ts">
  import { call } from "../lib/tauri";
  import Icon from "./Icon.svelte";
  import Logo from "./Logo.svelte";

  interface AccountInfo {
    name: string;
    kind: string;
  }

  const account = call<AccountInfo | null>("current_account").catch(() => null);
</script>

<div class="logo"><Logo size={48} /></div>

<header>
  <span class="title">MajorLauncher</span>
  <span class="spacer"></span>

  {#await account then a}
    <!-- Меню аккаунтов (сменить, добавить, выйти) — этап 3.3. -->
    <div class="acct">
      {#if a}
        <span class="avatar">{a.name[0].toUpperCase()}</span>
        <span class="who"><b>{a.name}</b><span class="muted">·</span><span class="muted">{a.kind}</span></span>
      {:else}
        <span class="avatar"><Icon name="user" size={18} /></span>
        <span class="muted">Аккаунт не выбран</span>
      {/if}
    </div>
  {/await}
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
  .acct {
    display: inline-flex;
    align-items: center;
    gap: var(--s3);
    height: 48px;
    padding: 0 var(--s4) 0 var(--s2);
    border: 1px solid var(--border);
    border-radius: var(--r);
    white-space: nowrap;
  }
  .avatar {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--r-sm);
    background: var(--surface-raised);
    color: var(--text);
    font-weight: 600;
  }
  .who {
    display: inline-flex;
    gap: 6px;
  }
  b {
    font-weight: 600;
  }
</style>
