<script lang="ts">
  // Запуск остановлен: в сборке проблемы, которые не дадут игре запуститься
  // (раздел 5.6 ТЗ, уровень «блокирует запуск»). Можно исправить или всё
  // равно играть — проверки не всесильны.
  import type { ModIssue } from "../lib/api";
  import { openBuild } from "../lib/nav.svelte";
  import { start } from "../lib/play.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let {
    info,
    onclose,
  }: { info: { id: string; version: string | null; issues: ModIssue[] }; onclose: () => void } = $props();
</script>

<Modal title="Игра не запустится" width={560} {onclose}>
  <ul class="list">
    {#each info.issues as i}
      <li><Icon name="error" size={18} /><span>{i.text}</span></li>
    {/each}
  </ul>
  <p class="muted small">Исправить можно на вкладке «Моды» сборки — у каждой проблемы там есть кнопка.</p>
  {#snippet footer()}
    <button
      class="btn btn-quiet"
      onclick={() => {
        // Сначала запомнить: после onclose() свойство info уже null.
        const { id, version } = info;
        onclose();
        start(id, version, true);
      }}>Всё равно играть</button
    >
    <button
      class="btn btn-primary"
      onclick={() => {
        const { id } = info;
        onclose();
        openBuild(id, "mods");
      }}>Открыть моды</button
    >
  {/snippet}
</Modal>

<style>
  .list {
    list-style: none;
    margin: 0 0 var(--s3);
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--s2);
  }
  .list li {
    display: flex;
    gap: var(--s2);
    align-items: flex-start;
    color: var(--danger);
  }
  .list span {
    color: var(--text);
  }
  p {
    margin: 0;
  }
</style>
