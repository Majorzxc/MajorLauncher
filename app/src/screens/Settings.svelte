<script lang="ts">
  // Настройки лаунчера (раздел 5.16 ТЗ): слева — страницы, справа — открытая.
  // Каждая страница — свой компонент в components/settings; общие для них
  // карточки, заголовки и переключатели оформлены здесь.
  import AboutSettings from "../components/settings/AboutSettings.svelte";
  import AccountsSettings from "../components/settings/AccountsSettings.svelte";
  import AiSettings from "../components/settings/AiSettings.svelte";
  import DataSettings from "../components/settings/DataSettings.svelte";
  import DownloadSettings from "../components/settings/DownloadSettings.svelte";
  import GameOptions from "../components/settings/GameOptions.svelte";
  import GameProfile from "../components/settings/GameProfile.svelte";
  import LookSettings from "../components/settings/LookSettings.svelte";
  import ModsSettings from "../components/settings/ModsSettings.svelte";
  import OptimizeSettings from "../components/settings/OptimizeSettings.svelte";
  import UpdateSettings from "../components/settings/UpdateSettings.svelte";
  import { settingsPage, type SettingsPage } from "../lib/nav.svelte";

  const PAGES: { id: SettingsPage; title: string; heading?: string }[] = [
    { id: "profile", title: "Настройки игры" },
    { id: "look", title: "Внешний вид" },
    { id: "game", title: "Запуск игры" },
    { id: "accounts", title: "Аккаунты" },
    { id: "catalog", title: "Моды и каталог" },
    { id: "optimize", title: "Оптимизация" },
    { id: "updates", title: "Обновления" },
    { id: "downloads", title: "Загрузки" },
    { id: "ai", title: "ИИ-разбор", heading: "ИИ-разбор ошибок" },
    { id: "data", title: "Папка данных" },
    { id: "about", title: "О программе" },
  ];

  // Страница из перехода (openSettings) — например, «Настройки игры» по ссылке.
  let page = $state<SettingsPage>(settingsPage.value ?? "look");
  settingsPage.value = null;
  const current = $derived(PAGES.find((p) => p.id === page)!);
</script>

<div class="settings">
  <nav class="pages" aria-label="Разделы настроек" data-tour="settings-pages">
    {#each PAGES as p (p.id)}
      <button class="page" aria-current={page === p.id ? "page" : undefined} onclick={() => (page = p.id)}>{p.title}</button>
    {/each}
  </nav>

  <section class="body">
    <h1>{current.heading ?? current.title}</h1>
    {#key page}
      {#if page === "profile"}<GameProfile />
      {:else if page === "look"}<LookSettings />
      {:else if page === "game"}<GameOptions />
      {:else if page === "accounts"}<AccountsSettings />
      {:else if page === "catalog"}<ModsSettings />
      {:else if page === "optimize"}<OptimizeSettings />
      {:else if page === "updates"}<UpdateSettings />
      {:else if page === "downloads"}<DownloadSettings />
      {:else if page === "ai"}<AiSettings />
      {:else if page === "data"}<DataSettings />
      {:else}<AboutSettings />
      {/if}
    {/key}
  </section>
</div>

<style>
  .settings {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: var(--s6);
    align-items: start;
  }
  .pages {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    position: sticky;
    top: 0;
  }
  .page {
    height: var(--h-md);
    padding: 0 var(--s3);
    border: 0;
    border-radius: var(--r);
    background: none;
    text-align: left;
    color: var(--text-muted);
    cursor: pointer;
  }
  .page:hover {
    background: var(--surface-raised);
    color: var(--text);
  }
  .page[aria-current="page"] {
    background: var(--surface-raised);
    color: var(--text);
    font-weight: 600;
  }
  .body {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--s4);
    max-width: 760px;
  }
  h1 {
    margin: 0 0 var(--s2);
    font-size: var(--fs-24);
    font-weight: 600;
  }

  /* Общее оформление страниц настроек: карточка-блок, заголовок карточки,
     галочка с подписью в две строки, ряд кнопок. Страница может уточнить своё. */
  .body :global(.block) {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: var(--s4);
    padding: var(--s5);
  }
  .body :global(.block > p) {
    margin: 0;
  }
  .body :global(.block h2) {
    margin: 0;
    font-size: var(--fs-16);
    font-weight: 600;
  }
  .body :global(.opt) {
    display: flex;
    align-items: flex-start;
    gap: var(--s3);
    cursor: pointer;
  }
  .body :global(.opt > input) {
    flex: none;
    margin-top: 4px;
  }
  .body :global(.opt > span) {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .body :global(.off) {
    opacity: 0.55;
  }
  .body :global(.row) {
    display: flex;
    align-items: center;
    gap: var(--s3);
  }
  .body :global(.row-buttons) {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s2);
  }
</style>
