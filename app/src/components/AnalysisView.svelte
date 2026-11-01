<script lang="ts">
  // Разбор анализатора ошибок (раздел 5.8 ТЗ): одна фраза — что случилось,
  // виновник, кнопки решений, «Подробнее» — строки лога, по которым узнали.
  import type { Analysis, Finding } from "../lib/api";
  import { actionLabel, runAction } from "../lib/modactions";
  import Icon from "./Icon.svelte";

  let {
    build,
    analysis,
    warnings = true,
    onaction,
  }: {
    build: string;
    analysis: Analysis;
    /// Показывать предупреждения (в окне падения — свёрнутыми).
    warnings?: boolean;
    /// После действия (выключили мод и т. п.) — окно может закрыться.
    onaction?: () => void;
  } = $props();

  let open = $state<Record<number, boolean>>({});
  let showWarnings = $state(false);
  let busy = $state(false);

  async function act(f: Finding, i: number) {
    busy = true;
    try {
      await runAction(build, f.actions[i], f.culprit?.name ?? null);
      onaction?.();
    } finally {
      busy = false;
    }
  }

  const all = $derived([
    ...analysis.critical.map((f) => ({ f, crit: true })),
    ...(warnings || showWarnings ? analysis.warnings.map((f) => ({ f, crit: false })) : []),
  ]);
</script>

<ul class="findings">
  {#each all as { f, crit }, n (n)}
    <li class:crit>
      <div class="head">
        <Icon name={crit ? "error" : "warning"} size={18} />
        <b class="grow">{f.title}</b>
        {#if f.count > 1}<span class="muted small">×{f.count}</span>{/if}
      </div>
      <p class="small">{f.text}</p>
      {#if f.culprit}
        <p class="small culprit">
          Виновник: <b>{f.culprit.name}</b>{#if f.culprit.file}<span class="muted mono">{" · "}{f.culprit.file}</span>{:else}<span
              class="muted">{" · "}в сборке его нет</span
            >{/if}
        </p>
      {/if}
      <div class="actions">
        {#each f.actions as a, i (i)}
          <button class="btn btn-sm" class:btn-primary={crit && i === 0} disabled={busy} onclick={() => act(f, i)}>
            {actionLabel(a)}
          </button>
        {/each}
        {#if f.lines.length}
          <button class="btn btn-sm btn-quiet" onclick={() => (open[n] = !open[n])}>
            {open[n] ? "Скрыть" : "Подробнее"}
          </button>
        {/if}
      </div>
      {#if open[n]}
        <pre>{#each f.lines as l (l)}<mark>{l}</mark>{"\n"}{/each}</pre>
      {/if}
    </li>
  {/each}
</ul>
{#if !warnings && analysis.warnings.length}
  <button class="btn btn-sm btn-quiet more" onclick={() => (showWarnings = !showWarnings)}>
    {showWarnings ? "Скрыть предупреждения" : `Ещё предупреждения за игру: ${analysis.warnings.length}`}
  </button>
{/if}

<style>
  .findings {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    padding: var(--s3) var(--s4);
    border: 1px solid var(--border);
    border-left: 3px solid var(--warning);
    border-radius: var(--r);
    background: var(--surface);
  }
  li.crit {
    border-left-color: var(--danger);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
  .head :global(svg) {
    flex: none;
    color: var(--warning);
  }
  .crit .head :global(svg) {
    color: var(--danger);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  p {
    margin: var(--s2) 0 0;
  }
  .culprit .mono {
    font-family: ui-monospace, Consolas, monospace;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s2);
    margin-top: var(--s3);
  }
  .actions:empty {
    display: none;
  }
  pre {
    max-height: 220px;
    margin: var(--s3) 0 0;
    padding: var(--s3);
    overflow: auto;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--r);
    font: var(--fs-12) / 1.5 ui-monospace, Consolas, monospace;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }
  mark {
    background: color-mix(in srgb, var(--warning) 25%, transparent);
    color: inherit;
  }
  .more {
    margin-top: var(--s3);
  }
</style>
