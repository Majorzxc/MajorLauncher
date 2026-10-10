<script lang="ts">
  // Кнопка «Играть» со всеми состояниями: подготовка, загрузка с процентами,
  // запуск, «игра запущена».
  import { busyLabel, percent, play, start } from "../lib/play.svelte";
  import Icon from "./Icon.svelte";

  let { id, large = false }: { id: string; large?: boolean } = $props();
  const label = $derived(busyLabel(id));
  // Полоса внутри кнопки: проценты загрузки или шаги установки загрузчика.
  const loading = $derived(
    play.status.build === id && (play.status.phase === "downloading" || play.status.phase === "installing"),
  );
</script>

<button class="play" class:large class:busy={!!label} disabled={!!label} onclick={() => start(id)}>
  {#if loading}<span class="fill" style:width="{percent(play.status)}%"></span>{/if}
  <span class="label">
    {#if !label}<Icon name="play" size={large ? 22 : 18} />{/if}
    {label ?? "Играть"}
  </span>
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
  .label {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--s2);
    font-variant-numeric: tabular-nums;
  }
</style>
