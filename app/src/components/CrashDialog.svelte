<script lang="ts">
  // Игра закрылась с ошибкой: код выхода и последние строки вывода.
  // Разбор ошибки с подсказками — этап 7 (анализатор).
  import type { Finished } from "../lib/api";
  import { api } from "../lib/api";
  import { duration } from "../lib/format";
  import { refresh } from "../lib/builds.svelte";
  import { ago, plural } from "../lib/format";
  import { attempt, toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let { info, onclose }: { info: Finished; onclose: () => void } = $props();

  // Падает раз за разом после изменения сборки — откатить его одной кнопкой.
  async function rollback() {
    const [build, snap] = [info.build, info.rollback!];
    onclose();
    const done = await attempt("Откат", () => api.snapshotRestore(build, snap.id));
    if (done !== undefined) {
      toast("Сборка откатилась к состоянию до последнего изменения");
      refresh();
    }
  }
</script>

<Modal title="Игра закрылась с ошибкой" width={720} {onclose}>
  <p>
    «{info.name}»: {info.no_window ? "игра не дошла до окна" : `сессия ${duration(info.seconds)}`}{#if info.code !== null}, код
      выхода {info.code}{/if}.
  </p>
  {#if info.tail.length}
    <pre>{info.tail.join("\n")}</pre>
  {/if}
  {#if info.rollback}
    <p class="streak">
      <Icon name="warning" size={18} />
      <span>
        Игра падает {info.streak} {plural(info.streak ?? 0, ["раз", "раза", "раз"])} подряд. Последнее изменение сборки — {info.rollback.reason}, {ago(
          info.rollback.created,
        )}. Можно откатить сборку к состоянию до него (текущее сохранится снапшотом).
      </span>
    </p>
  {/if}
  <p class="muted small">Подробности — в логах сборки. Разбор ошибок с подсказками появится на этапе 7.</p>
  {#snippet footer()}
    <button class="btn" onclick={() => attempt("Логи", () => api.open(info.build, { kind: "folder", name: "logs" }))}>
      <Icon name="folder" size={18} />Папка логов
    </button>
    {#if info.rollback}
      <button class="btn" onclick={onclose}>Закрыть</button>
      <button class="btn btn-primary" onclick={rollback}><Icon name="restore" size={18} />Откатить изменение</button>
    {:else}
      <button class="btn btn-primary" onclick={onclose}>Закрыть</button>
    {/if}
  {/snippet}
</Modal>

<style>
  p {
    margin: 0 0 var(--s3);
  }
  .streak {
    display: flex;
    align-items: flex-start;
    gap: var(--s2);
    color: var(--warning);
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
