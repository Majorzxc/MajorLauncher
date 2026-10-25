<script lang="ts">
  // Кнопка «Загрузки» в шапке: что лаунчер качает или проверяет, куда и
  // сколько осталось; завершённые и ошибки — до «Очистить».
  import { fly } from "svelte/transition";
  import { api, type Task } from "../lib/api";
  import { size } from "../lib/format";
  import { ms } from "../lib/motion";
  import { running, share, tasks } from "../lib/tasks.svelte";
  import Icon from "./Icon.svelte";

  let root: HTMLDivElement;
  const active = $derived(running());
  const failed = $derived(tasks.list.filter((t) => t.state === "failed").length);
  // Новые сверху.
  const list = $derived([...tasks.list].reverse());
  const overall = $derived.by(() => {
    const total = active.reduce((s, t) => s + t.total, 0);
    const done = active.reduce((s, t) => s + t.done, 0);
    return total > 0 ? Math.floor((done * 100) / total) : null;
  });

  function detail(t: Task): string {
    if (t.state === "failed") return t.error ?? "ошибка";
    if (t.state === "cancelled") return "отменено — скачанное останется до следующего раза";
    if (t.state === "done") {
      const took = (t.finished ?? t.started) - t.started;
      const what = t.files_total ? `${t.files_total} ${files(t.files_total)}, ${size(t.total)}` : "готово";
      return took >= 1 ? `${what} · ${took} с` : what;
    }
    if (t.stage && t.total === 0) return t.stage;
    if (t.total === 0) return "подготовка…";
    return `${size(t.done)} из ${size(t.total)} · файлов ${t.files_done} из ${t.files_total}`;
  }

  function files(n: number) {
    const d = n % 10;
    const dd = n % 100;
    if (d === 1 && dd !== 11) return "файл";
    if (d >= 2 && d <= 4 && (dd < 12 || dd > 14)) return "файла";
    return "файлов";
  }
</script>

<svelte:window
  onmousedown={(e) => tasks.open && !root.contains(e.target as Node) && (tasks.open = false)}
  onkeydown={(e) => e.key === "Escape" && (tasks.open = false)}
/>

<div class="wrap" bind:this={root}>
  <button
    class="btn-dl"
    class:busy={active.length > 0}
    aria-haspopup="dialog"
    aria-expanded={tasks.open}
    aria-label="Загрузки"
    title="Загрузки"
    onclick={() => (tasks.open = !tasks.open)}
  >
    <Icon name="download" size={20} />
    {#if active.length}
      <span class="count">{overall !== null ? `${overall}%` : active.length}</span>
    {:else if failed}
      <span class="count bad">{failed}</span>
    {/if}
  </button>

  {#if tasks.open}
    <div class="panel" role="dialog" aria-label="Загрузки" in:fly={{ y: -4, duration: ms(120) }} out:fly={{ y: -4, duration: ms(80) }}>
      <div class="head">
        <b>Загрузки</b>
        {#if tasks.list.length > active.length}
          <button class="link small" onclick={() => api.tasksClear()}>Очистить завершённые</button>
        {/if}
      </div>
      {#if list.length === 0}
        <p class="muted small empty">
          Сейчас ничего не качается. Здесь видно установку из каталога, обновление модов, загрузку файлов игры и проверку файлов.
        </p>
      {:else}
        <ul>
          {#each list as t (t.id)}
            {@const p = share(t)}
            <li class={t.state}>
              <span class="state">
                {#if t.state === "running"}<span class="spin"></span>
                {:else if t.state === "done"}<Icon name="check" size={16} />
                {:else if t.state === "cancelled"}<Icon name="close" size={16} />
                {:else}<Icon name="error" size={16} />{/if}
              </span>
              <span class="body">
                <span class="title">{t.title}</span>
                <span class="muted small">{t.place}</span>
                {#if t.state === "running"}
                  <span class="track" class:unknown={p === null}><span style:width="{p ?? 30}%"></span></span>
                {/if}
                <span class="small detail">{detail(t)}</span>
              </span>
              {#if t.state === "running" && p !== null}<b class="small">{p}%</b>{/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }
  .btn-dl {
    display: inline-flex;
    align-items: center;
    gap: var(--s2);
    height: 48px;
    min-width: 48px;
    justify-content: center;
    padding: 0 var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: none;
    color: var(--text-muted);
    cursor: pointer;
  }
  .btn-dl:hover,
  .btn-dl[aria-expanded="true"] {
    border-color: var(--border-strong);
    color: var(--text);
  }
  .btn-dl.busy {
    color: var(--accent);
    border-color: var(--accent);
  }
  .count {
    font-size: var(--fs-14);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .count.bad {
    color: var(--danger);
  }
  .panel {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    z-index: 60;
    width: 380px;
    max-height: 70vh;
    overflow: auto;
    padding: var(--s3);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--r);
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--s2);
  }
  .empty {
    margin: 0;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--s2);
  }
  li {
    display: flex;
    gap: var(--s3);
    align-items: flex-start;
    padding: var(--s2) var(--s2);
    border-radius: var(--r);
    background: var(--bg);
  }
  .state {
    display: grid;
    place-items: center;
    width: 18px;
    height: 20px;
    flex: none;
  }
  .done .state {
    color: var(--success, var(--text-muted));
  }
  .failed .state,
  .failed .detail {
    color: var(--danger);
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title {
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .detail {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .track {
    display: block;
    height: 4px;
    margin: 4px 0 2px;
    border-radius: 2px;
    background: var(--border);
    overflow: hidden;
  }
  .track > span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  .track.unknown > span {
    animation: slide 1.2s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  .spin {
    width: 14px;
    height: 14px;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: turn 0.8s linear infinite;
  }
  @keyframes turn {
    to {
      transform: rotate(360deg);
    }
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-muted);
    text-decoration: underline;
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
  }
  @media (prefers-reduced-motion: reduce) {
    .spin,
    .track.unknown > span {
      animation: none;
    }
  }
</style>
