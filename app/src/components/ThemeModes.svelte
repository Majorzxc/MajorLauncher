<script lang="ts">
  // Выбор темы карточками с мини-окном: в настройках и в мастере первого запуска.
  import { theme, type ThemeMode } from "../lib/theme.svelte";

  let { onpick }: { onpick: (mode: ThemeMode) => void } = $props();

  const MODES: { id: ThemeMode; title: string; hint: string }[] = [
    { id: "dark", title: "Чёрная", hint: "Тёмный фон, светлый текст" },
    { id: "light", title: "Белая", hint: "Светлый фон, тёмный текст" },
    { id: "system", title: "Как в Windows", hint: "Следует за темой системы" },
  ];
</script>

<div class="modes" role="radiogroup" aria-label="Тема">
  {#each MODES as m (m.id)}
    <button class="mode" role="radio" aria-checked={theme.appearance.mode === m.id} onclick={() => onpick(m.id)}>
      <span class="mini {m.id}" aria-hidden="true">
        <i class="mini-nav"></i><i class="mini-card"></i><i class="mini-btn"></i>
      </span>
      <b>{m.title}</b>
      <span class="muted">{m.hint}</span>
    </button>
  {/each}
</div>

<style>
  .modes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--s3);
  }
  .mode {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--bg);
    text-align: left;
    cursor: pointer;
  }
  .mode:hover {
    border-color: var(--border-strong);
  }
  .mode[aria-checked="true"] {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .mode b {
    margin-top: var(--s2);
    font-weight: 600;
  }
  .mode .muted {
    font-size: var(--fs-14);
  }

  /* Мини-окно для превью темы: фиксированные цвета, не зависят от текущей темы. */
  .mini {
    position: relative;
    width: 100%;
    height: 72px;
    border-radius: var(--r-sm);
    overflow: hidden;
    border: 1px solid #00000022;
  }
  .mini i {
    position: absolute;
    display: block;
    border-radius: 3px;
  }
  .mini-nav {
    left: 0;
    top: 0;
    bottom: 0;
    width: 14px;
    border-radius: 0 !important;
  }
  .mini-card {
    left: 24px;
    right: 10px;
    top: 10px;
    height: 30px;
  }
  .mini-btn {
    left: 24px;
    width: 46px;
    bottom: 10px;
    height: 12px;
  }
  .mini.dark {
    background: #0e0e10;
  }
  .mini.dark .mini-nav,
  .mini.dark .mini-card {
    background: #1d1d20;
  }
  .mini.dark .mini-btn {
    background: #ededef;
  }
  .mini.light {
    background: #f4f4f5;
  }
  .mini.light .mini-nav,
  .mini.light .mini-card {
    background: #ffffff;
  }
  .mini.light .mini-btn {
    background: #121214;
  }
  .mini.system {
    background: linear-gradient(90deg, #0e0e10 50%, #f4f4f5 50%);
  }
  .mini.system .mini-nav {
    background: #1d1d20;
  }
  .mini.system .mini-card {
    background: linear-gradient(90deg, #1d1d20 50%, #ffffff 50%);
  }
  .mini.system .mini-btn {
    background: #ededef;
  }

</style>
