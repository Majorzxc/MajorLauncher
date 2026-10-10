<script lang="ts">
  // Что поставится: мод, его обязательные зависимости, что уже стоит, чего
  // не хватает и с чем мод заявлен несовместимым (раздел 5.4 ТЗ).
  import type { Plan } from "../../lib/api";
  import { catalog, setAutoDeps } from "../../lib/catalog.svelte";
  import { size } from "../../lib/format";
  import Icon from "../Icon.svelte";
  import Modal from "../Modal.svelte";

  let { plan, ondone }: { plan: Plan; ondone: (ok: boolean) => void } = $props();

  const total = $derived(plan.items.reduce((s, i) => s + i.size, 0));
</script>

<Modal title="Установка" width={560} onclose={() => ondone(false)}>
  <ul class="rows items">
    {#each plan.items as it (it.version_id)}
      <li>
        {#if it.icon_url}<img src={it.icon_url} alt="" width="32" height="32" />{:else}<Icon name="mods" size={24} />{/if}
        <span class="grow">
          <b>{it.title}</b>
          <span class="muted small">
            {it.version_number}{it.dependency ? " · нужен для работы" : ""}{it.replaces ? ` · заменит ${it.replaces}` : ""}
          </span>
        </span>
        <span class="muted small">{size(it.size)}</span>
      </li>
    {/each}
  </ul>
  {#if plan.present.length}
    <p class="muted small"><Icon name="check" size={16} />Уже стоят: {plan.present.join(", ")}.</p>
  {/if}
  {#if plan.missing.length}
    <p class="warn">
      <Icon name="warning" size={18} />Нет версии под эту сборку: {plan.missing.join(", ")}. Без них мод может не запуститься.
    </p>
  {/if}
  {#if plan.conflicts.length}
    <p class="warn">
      <Icon name="warning" size={18} />Автор мода пишет, что он несовместим с {plan.conflicts.join(", ")} — они уже в сборке.
    </p>
  {/if}
  {#if plan.optional.length}
    <p class="muted small">Необязательные дополнения (не ставятся сами): {plan.optional.join(", ")}.</p>
  {/if}
  <label class="opt">
    <input type="checkbox" checked={catalog.autoDeps} onchange={(e) => setAutoDeps(e.currentTarget.checked)} />
    Ставить нужные зависимости без этого окна
  </label>
  {#snippet footer()}
    <button class="btn btn-quiet" onclick={() => ondone(false)}>Отмена</button>
    <button class="btn btn-primary" onclick={() => ondone(true)}>
      <Icon name="download" size={18} />Установить{plan.items.length > 1 ? ` (${plan.items.length})` : ""} · {size(total)}
    </button>
  {/snippet}
</Modal>

<style>
  .items {
    margin-bottom: var(--s3);
  }
  .items img {
    border-radius: var(--r-sm);
  }
  .grow {
    display: flex;
    flex-direction: column;
  }
  p {
    display: flex;
    align-items: flex-start;
    gap: var(--s2);
    margin: 0 0 var(--s2);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    margin-top: var(--s3);
    font-size: var(--fs-14);
    cursor: pointer;
  }
</style>
