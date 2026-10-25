<script lang="ts">
  import AccountMenu from "./AccountMenu.svelte";
  import Downloads from "./Downloads.svelte";
  import Logo from "./Logo.svelte";
  import Icon from "./Icon.svelte";
  import { onMount } from "svelte";
  import { api } from "../lib/api";

  // Нет интернета — сказать сразу, а не ошибкой каталога или входа. Проверка
  // раз в полторы минуты: одно соединение с Modrinth.
  let offline = $state(false);
  onMount(() => {
    const check = () => api.netOnline().then((on) => (offline = !on)).catch(() => {});
    check();
    const timer = setInterval(check, 90_000);
    return () => clearInterval(timer);
  });
</script>

<div class="logo"><Logo size={48} /></div>

<header>
  <span class="title">MajorLauncher</span>
  <span class="spacer"></span>
  {#if offline}
    <span
      class="offline small"
      title="Скачанные сборки запускаются и без интернета. Каталог, обновления модов и вход через Ely.by — недоступны, пока сеть не появится."
      ><Icon name="warning" size={16} />Нет интернета — можно играть в скачанные сборки</span
    >
  {/if}

  <Downloads />
  <AccountMenu />
</header>

<style>
  .offline {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border: 1px solid var(--warning);
    border-radius: var(--r);
    color: var(--warning);
    white-space: nowrap;
  }
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
