<script lang="ts">
  // Логи игры и краш-отчёты; сверху — разбор последней игры анализатором,
  // у каждого лога — «Разобрать».
  import { api, type Analysis, type LogFile } from "../../lib/api";
  import AnalysisView from "../AnalysisView.svelte";
  import Modal from "../Modal.svelte";
  import { ago, size } from "../../lib/format";
  import { attempt } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";
  import LogViewer from "../LogViewer.svelte";

  let { id }: { id: string } = $props();
  let list: LogFile[] | null = $state(null);
  let error = $state("");
  /// Открытый в просмотре лог.
  let open = $state<LogFile | null>(null);

  $effect(() => {
    api
      .logs(id)
      .then((l) => (list = l))
      .catch((e) => (error = String(e)));
  });

  let last = $state<{ at: number; analysis: Analysis } | null>(null);
  $effect(() => {
    api.problemsLast(id).then((s) => (last = s)).catch(() => {});
  });
  const lastCount = $derived(last ? last.analysis.critical.length + last.analysis.warnings.length : 0);

  // «Разобрать» любой лог.
  let checked = $state<{ file: string; analysis: Analysis } | null>(null);
  let checking = $state<string | null>(null);
  async function analyze(l: LogFile) {
    checking = l.file;
    const a = await attempt("Разбор", () => api.analyzeLog(id, l.file));
    checking = null;
    if (a) checked = { file: l.file, analysis: a };
  }
</script>

{#if open}
  <LogViewer {id} log={open} onback={() => (open = null)} />
{:else}
{#if last}
  <div class="last">
    <h3>
      Последняя игра · {ago(last.at)}
      <span class="muted small">{lastCount ? `проблем: ${lastCount}` : "проблем не найдено"}</span>
    </h3>
    {#if lastCount}<AnalysisView build={id} analysis={last.analysis} />{/if}
  </div>
{/if}
<div class="bar">
  <span class="muted small grow">Лаунчер разбирает лог сам после каждой игры; любой лог можно разобрать кнопкой.</span>
  <button class="btn btn-sm" onclick={() => attempt("Папка", () => api.open(id, { kind: "folder", name: "logs" }))}>
    <Icon name="folder" size={16} />Открыть папку
  </button>
</div>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if list && list.length === 0}
  <p class="note">Логов пока нет — игра пишет их при запуске.</p>
{:else if list}
  <ul class="rows">
    {#each list as l (l.file)}
      <li>
        <Icon name={l.kind === "crash" ? "error" : "log"} size={20} />
        <button class="link grow" onclick={() => (open = l)}>
          {l.file}
        </button>
        {#if l.kind === "crash"}<span class="badge">краш</span>{/if}
        <button class="btn btn-sm btn-quiet" disabled={checking !== null} onclick={() => analyze(l)}>
          {checking === l.file ? "Разбираю…" : "Разобрать"}
        </button>
        <span class="muted small">{size(l.size)}</span>
        <span class="muted small when">{ago(l.modified)}</span>
      </li>
    {/each}
  </ul>
{/if}
{/if}

{#if checked}
  {@const c = checked}
  <Modal title="Разбор {c.file}" width={720} onclose={() => (checked = null)}>
    {#if c.analysis.critical.length + c.analysis.warnings.length}
      <AnalysisView build={id} analysis={c.analysis} onaction={() => (checked = null)} />
    {:else}
      <p class="muted">Анализатор ничего не нашёл: в этом логе только обычный шум модов.</p>
    {/if}
  </Modal>
{/if}

<style>
  .last {
    margin-bottom: var(--s5);
  }
  .last h3 {
    display: flex;
    align-items: baseline;
    gap: var(--s3);
    margin: 0 0 var(--s3);
    font-size: var(--fs-16);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    font-family: ui-monospace, Consolas, monospace;
    font-size: var(--fs-14);
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
  .badge {
    padding: 2px 8px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--danger) 20%, transparent);
    color: var(--danger);
    font-size: var(--fs-12);
  }
  .when {
    width: 140px;
    text-align: right;
  }
</style>
