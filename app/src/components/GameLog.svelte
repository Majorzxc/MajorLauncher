<script lang="ts">
  // Панель живого лога игры внизу окна. Прокрутка сама идёт вниз, пока её не
  // отвели вверх — тогда стоит на месте, чтобы можно было читать.
  import { gameLog } from "../lib/gamelog.svelte";
  import { play } from "../lib/play.svelte";
  import Icon from "./Icon.svelte";

  let box = $state<HTMLDivElement>();
  let follow = $state(true);

  $effect(() => {
    void gameLog.lines.length;
    if (follow && box) queueMicrotask(() => box && (box.scrollTop = box.scrollHeight));
  });

  function level(line: string): string {
    if (/\b(ERROR|FATAL|Exception|Caused by)\b/.test(line)) return "lvl-err";
    if (/\bWARN\b/.test(line)) return "lvl-warn";
    return "";
  }

  const running = $derived(play.status.phase !== "idle");
</script>

{#if gameLog.open && gameLog.lines.length}
  <section class="log" aria-label="Лог игры">
    <header>
      <b>Лог игры</b>
      <span class="muted small">{running ? "игра идёт" : "игра закрыта"} · строк: {gameLog.lines.length}</span>
      <span class="spacer"></span>
      {#if !follow}
        <button class="btn btn-sm btn-quiet" onclick={() => (follow = true)}>К концу</button>
      {/if}
      <button class="btn btn-sm btn-quiet" aria-label="Скрыть лог игры" title="Скрыть" onclick={() => (gameLog.open = false)}>
        <Icon name="close" size={16} />
      </button>
    </header>
    <div
      class="lines"
      bind:this={box}
      onscroll={() => box && (follow = box.scrollHeight - box.scrollTop - box.clientHeight < 24)}
    >
      {#each gameLog.lines as line (line.n)}<div class={level(line.text)}>{line.text}</div>{/each}
    </div>
  </section>
{/if}

<style>
  .log {
    position: fixed;
    left: var(--nav-w);
    right: 0;
    bottom: 0;
    z-index: 40;
    display: flex;
    flex-direction: column;
    height: 38vh;
    background: var(--surface);
    border-top: 1px solid var(--border-strong);
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: var(--s2) var(--s4);
    border-bottom: 1px solid var(--border);
  }
  .spacer {
    flex: 1;
  }
  .lines {
    flex: 1;
    overflow: auto;
    padding: var(--s2) var(--s4);
    font: var(--fs-12) / 1.5 ui-monospace, Consolas, monospace;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }
  .lvl-err {
    color: var(--danger);
  }
  .lvl-warn {
    color: var(--warning);
  }
</style>
