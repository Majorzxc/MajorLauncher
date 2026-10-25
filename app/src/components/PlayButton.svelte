<script lang="ts">
  // Кнопка «Играть» со всеми состояниями: подготовка, загрузка с процентами,
  // запуск, «игра запущена».
  import { busyLabel, cancel, cancellable, launchPercent, percent, play, stageLabel, start } from "../lib/play.svelte";
  import Icon from "./Icon.svelte";

  let { id, large = false }: { id: string; large?: boolean } = $props();
  const label = $derived(busyLabel(id));
  // Что делается сейчас: «Загрузка модов: 162 · 0:14». У большой кнопки —
  // второй строкой, у маленькой — в подсказке.
  const stage = $derived(stageLabel(id));
  // Полоса внутри кнопки: проценты загрузки, шаги установки загрузчика или
  // время запуска относительно прошлого раза.
  const fill = $derived.by(() => {
    if (play.status.build !== id) return null;
    if (play.status.phase === "downloading" || play.status.phase === "installing") return percent(play.status);
    return launchPercent(play.status);
  });
  // Пока окна игры нет, запуск можно отменить: при наведении кнопка
  // показывает «Отменить».
  const canCancel = $derived(cancellable(id));
</script>

<button
  class="play"
  class:large
  class:busy={!!label}
  class:cancellable={canCancel}
  disabled={!!label && !canCancel}
  title={canCancel ? "Отменить запуск" : !large && stage ? stage : undefined}
  aria-live="polite"
  onclick={() => (canCancel ? cancel() : start(id))}
>
  {#if fill !== null}<span class="fill" style:width="{fill}%"></span>{/if}
  <span class="label">
    {#if !label}<Icon name="play" size={large ? 22 : 18} />{/if}
    <span class="now">{label ?? "Играть"}</span>
    {#if canCancel}<span class="stop"><Icon name="close" size={large ? 20 : 16} />Отменить</span>{/if}
  </span>
  {#if large && stage}<span class="stage">{stage}</span>{/if}
</button>

<style>
  .play {
    position: relative;
    overflow: hidden;
    height: var(--h-md);
    min-width: 140px;
    padding: 0 var(--s5);
    border: 1px solid var(--accent);
    border-radius: var(--r);
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
    cursor: pointer;
  }
  .play:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .play:active:not(:disabled) {
    transform: translateY(1px);
  }
  .large {
    height: 56px;
    min-width: 260px;
    font-size: var(--fs-20);
  }
  .busy {
    background: var(--surface-raised);
    border-color: var(--border-strong);
    color: var(--text);
    cursor: default;
  }
  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: color-mix(in srgb, var(--accent) 35%, transparent);
    transition: width 0.2s linear;
  }
  .stop {
    display: none;
    align-items: center;
    gap: var(--s2);
  }
  .cancellable {
    cursor: pointer;
  }
  .cancellable:hover {
    border-color: var(--danger);
  }
  .cancellable:hover .now {
    display: none;
  }
  .cancellable:hover .stop {
    display: inline-flex;
    color: var(--danger);
  }
  .stage {
    position: relative;
    display: block;
    max-width: 212px;
    line-height: 1.3;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-12);
    font-weight: 400;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .label {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--s2);
    font-variant-numeric: tabular-nums;
  }
</style>
