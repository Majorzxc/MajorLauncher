<script lang="ts">
  // Редактор клавиш (раздел 5.18 ТЗ): назначения из общего профиля — для всех
  // сборок, в старых версиях лаунчер сам переводит их в их формат. Нажмите
  // «изменить» и клавишу или кнопку мыши; Esc — отмена.
  import { api, type OptionsView } from "../../lib/api";
  import { fromKeyboard, fromMouse, keyLabel } from "../../lib/keys";
  import { attempt, toast } from "../../lib/toast.svelte";

  let { opts, onchange }: { opts: OptionsView; onchange: (o: OptionsView) => void } = $props();

  let waiting = $state<string | null>(null);
  let query = $state("");

  const current = (id: string, fallback: string) => opts.keys[`key_${id}`] ?? fallback;
  // Действия модов — всё, чего нет среди ванильных.
  const modded = $derived(
    Object.keys(opts.keys)
      .map((k) => k.replace(/^key_/, ""))
      .filter((id) => !opts.actions.some((a) => a.id === id))
      .sort(),
  );
  const q = $derived(query.trim().toLowerCase());
  const vanilla = $derived(opts.actions.filter((a) => !q || a.label.toLowerCase().includes(q) || a.id.includes(q)));
  const mods = $derived(modded.filter((id) => !q || id.toLowerCase().includes(q)));

  // Одна клавиша на два действия — частая причина «не работает»: подсветим.
  const taken = $derived.by(() => {
    const count = new Map<string, number>();
    for (const a of opts.actions) {
      const k = current(a.id, a.default);
      if (!k.endsWith(".unknown")) count.set(k, (count.get(k) ?? 0) + 1);
    }
    return count;
  });

  async function assign(id: string, key: string | null) {
    waiting = null;
    const next = await attempt("Клавиша", () => api.setKey(id, key));
    if (next) onchange(next);
  }

  function start(id: string) {
    // Клик, которым начали, — не назначение: ждём следующего нажатия.
    setTimeout(() => (waiting = id), 0);
  }

  function key(e: KeyboardEvent) {
    if (!waiting) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.code === "Escape") return (waiting = null);
    const name = fromKeyboard(e.code);
    if (!name) return toast("Эту клавишу игра не поддерживает", { error: true });
    assign(waiting, name);
  }

  function mouse(e: MouseEvent) {
    if (!waiting) return;
    e.preventDefault();
    e.stopPropagation();
    assign(waiting, fromMouse(e.button));
  }
</script>

<svelte:window onkeydowncapture={key} onmousedowncapture={mouse} />

<input type="search" class="input" placeholder="Найти действие" bind:value={query} aria-label="Найти действие" />

{#snippet row(id: string, label: string, fallback: string | null)}
  {@const k = fallback === null ? opts.keys[`key_${id}`] : current(id, fallback)}
  <li class:waiting={waiting === id}>
    <span class="what">{label}{#if fallback === null}<span class="muted small mono">{id}</span>{/if}</span>
    <button
      class="key"
      class:dup={(taken.get(k) ?? 0) > 1}
      title={(taken.get(k) ?? 0) > 1 ? "Эта клавиша назначена и другому действию" : "Изменить"}
      onclick={() => start(id)}
    >
      {waiting === id ? "Нажмите клавишу…" : keyLabel(k)}
    </button>
    {#if fallback !== null && opts.keys[`key_${id}`] && opts.keys[`key_${id}`] !== fallback}
      <button class="btn btn-sm btn-quiet" title="Как по умолчанию: {keyLabel(fallback)}" onclick={() => assign(id, null)}>Сбросить</button>
    {:else}
      <span class="reset-space"></span>
    {/if}
  </li>
{/snippet}

<ul class="keys">
  {#each vanilla as a (a.id)}{@render row(a.id, a.label, a.default)}{/each}
</ul>
{#if mods.length}
  <h3>Действия модов</h3>
  <ul class="keys">
    {#each mods as id (id)}{@render row(id, id.replace(/^key\./, ""), null)}{/each}
  </ul>
{/if}
<p class="muted small">
  Esc — отменить ожидание. Клавиши можно менять и в игре — после неё лаунчер заберёт их сюда. Для сборок до 1.13
  назначения переводятся в их формат сами.
</p>

<style>
  .keys {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .keys li {
    display: flex;
    align-items: center;
    gap: var(--s3);
    min-height: 40px;
    border-top: 1px solid var(--border);
  }
  .keys li:first-child {
    border-top: 0;
  }
  .what {
    flex: 1;
    display: flex;
    flex-direction: column;
    font-size: var(--fs-14);
  }
  .mono {
    font-family: Consolas, monospace;
  }
  .key {
    min-width: 128px;
    height: var(--h-sm);
    padding: 0 var(--s3);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-sm);
    background: var(--bg);
    font-size: var(--fs-14);
    font-weight: 600;
    cursor: pointer;
  }
  .key:hover {
    border-color: var(--accent);
  }
  .key.dup {
    border-color: var(--warning);
    color: var(--warning);
  }
  .waiting .key {
    border-color: var(--accent);
    color: var(--accent);
  }
  .reset-space {
    width: 82px;
  }
  h3 {
    margin: var(--s2) 0 0;
    font-size: var(--fs-14);
    font-weight: 600;
  }
</style>
