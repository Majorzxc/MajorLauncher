<script lang="ts">
  // Игра закрылась с ошибкой: код выхода и последние строки вывода.
  // Разбор ошибки с подсказками — этап 7 (анализатор).
  import type { Finished } from "../lib/api";
  import { api } from "../lib/api";
  import { duration } from "../lib/format";
  import { attempt } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let { info, onclose }: { info: Finished; onclose: () => void } = $props();
</script>

<Modal title="Игра закрылась с ошибкой" width={720} {onclose}>
  <p>
    «{info.name}»: {info.no_window ? "игра не дошла до окна" : `сессия ${duration(info.seconds)}`}{#if info.code !== null}, код
      выхода {info.code}{/if}.
  </p>
  {#if info.tail.length}
    <pre>{info.tail.join("\n")}</pre>
  {/if}
  <p class="muted small">Подробности — в логах сборки. Разбор ошибок с подсказками появится на этапе 7.</p>
  {#snippet footer()}
    <button class="btn" onclick={() => attempt("Логи", () => api.open(info.build, { kind: "folder", name: "logs" }))}>
      <Icon name="folder" size={18} />Папка логов
    </button>
    <button class="btn btn-primary" onclick={onclose}>Закрыть</button>
  {/snippet}
</Modal>

<style>
  p {
    margin: 0 0 var(--s3);
  }
  pre {
    max-height: 320px;
    margin: 0 0 var(--s3);
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
</style>
