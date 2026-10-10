<script lang="ts">
  // Логи игры и краш-отчёты. Разбор ошибок с подсказками — этап 7.
  import { api, type LogFile } from "../../lib/api";
  import { ago, size } from "../../lib/format";
  import { attempt } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";

  let { id }: { id: string } = $props();
  let list: LogFile[] | null = $state(null);
  let error = $state("");

  $effect(() => {
    api
      .logs(id)
      .then((l) => (list = l))
      .catch((e) => (error = String(e)));
  });
</script>

<div class="bar">
  <span class="muted small grow">Разбор ошибок с подсказками, что делать, — на этапе 7.</span>
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
        <button class="link grow" onclick={() => attempt("Лог", () => api.open(id, { kind: "log", file: l.file }))}>
          {l.file}
        </button>
        {#if l.kind === "crash"}<span class="badge">краш</span>{/if}
        <span class="muted small">{size(l.size)}</span>
        <span class="muted small when">{ago(l.modified)}</span>
      </li>
    {/each}
  </ul>
{/if}

<style>
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
