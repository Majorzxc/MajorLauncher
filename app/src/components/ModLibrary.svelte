<script lang="ts">
  // Библиотека модов (раздел 5.4 ТЗ): все моды игрока, где каждый стоит,
  // какие пустуют. Выделить несколько — добавить в сборку; пустующие — удалить.
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import Select from "./Select.svelte";
  import { api, type LibMod, type ModLibrary } from "../lib/api";
  import { builds, refresh } from "../lib/builds.svelte";
  import { catalog } from "../lib/catalog.svelte";
  import { size } from "../lib/format";
  import { loaderTitle } from "../lib/loaders";
  import { openBuild } from "../lib/nav.svelte";
  import { attempt, toast } from "../lib/toast.svelte";

  let lib = $state<ModLibrary | null>(null);
  let error = $state("");
  let query = $state("");
  let show = $state<"all" | "unused" | "own">("all");
  let picked = $state<string[]>([]);
  let target = $state<string | null>(null);
  let confirm = $state<null | { keys: string[] | null; bytes: number; count: number }>(null);

  if (!builds.loaded) refresh();

  async function load() {
    try {
      lib = await api.libraryMods();
      error = "";
      picked = picked.filter((k) => lib!.mods.some((m) => m.key === k));
    } catch (e) {
      error = String(e);
    }
  }
  load();

  const unused = (m: LibMod) => m.in_library && m.builds.length === 0;
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return (lib?.mods ?? []).filter((m) => {
      if (show === "unused" && !unused(m)) return false;
      if (show === "own" && m.in_library) return false;
      return !q || m.title.toLowerCase().includes(q) || m.file.toLowerCase().includes(q);
    });
  });
  const idle = $derived((lib?.mods ?? []).filter(unused));
  const ownCount = $derived((lib?.mods ?? []).filter((m) => !m.in_library).length);

  const targets = $derived(
    builds.list
      .filter((b) => b.state === "active" && !b.quick)
      .map((b) => ({ value: b.id as string | null, label: `${b.name} · ${b.version} · ${loaderTitle(b.loader)}` })),
  );
  $effect(() => {
    if (!target && targets.length) target = targets[0].value;
  });
  const targetBuild = $derived(builds.list.find((b) => b.id === target));

  function toggle(key: string, on: boolean) {
    picked = on ? [...picked, key] : picked.filter((k) => k !== key);
  }
  const allShown = $derived(shown.length > 0 && shown.every((m) => picked.includes(m.key)));
  function toggleAll(on: boolean) {
    const keys = shown.map((m) => m.key);
    picked = on ? [...new Set([...picked, ...keys])] : picked.filter((k) => !keys.includes(k));
  }

  // Мод под другой загрузчик в эту сборку не встанет — предупредим заранее.
  const mismatch = $derived.by(() => {
    if (!targetBuild || targetBuild.loader === "vanilla") return [];
    const want = targetBuild.loader;
    return (lib?.mods ?? []).filter(
      (m) =>
        picked.includes(m.key) &&
        m.loaders.length > 0 &&
        !m.loaders.includes(want) &&
        !(want === "neoforge" && m.loaders.includes("forge") && targetBuild.version === "1.20.1"),
    );
  });

  async function addPicked() {
    if (!target || !picked.length) return;
    const b = targetBuild;
    const n = await attempt("Библиотека", () => api.libraryModsAdd(picked, target!));
    if (n === null || n === undefined) return;
    const skipped = picked.length - n;
    toast(
      n
        ? `Добавлено в «${b?.name}»: ${n}${skipped ? `, уже стояли: ${skipped}` : ""}. Проверить совместимость — на вкладке «Моды» сборки.`
        : "Все выбранные моды уже стоят в этой сборке",
    );
    picked = [];
    load();
  }

  function askRemove(keys: string[] | null) {
    const list = keys ? idle.filter((m) => keys.includes(m.key)) : idle;
    confirm = { keys, count: list.length, bytes: list.reduce((s, m) => s + m.size, 0) };
  }

  async function removeConfirmed() {
    const c = confirm!;
    confirm = null;
    const freed = await attempt("Библиотека", () => api.libraryModsRemoveUnused(c.keys));
    if (freed !== null && freed !== undefined) {
      toast(`Удалено модов: ${c.count}, освобождено ${size(freed)}`);
      picked = [];
      load();
    }
  }

  const pickedIdle = $derived(idle.filter((m) => picked.includes(m.key)).map((m) => m.key));
</script>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if !lib}
  <p class="muted">Собираю моды из библиотеки и сборок…</p>
{:else if lib.mods.length === 0}
  <p class="note">
    Модов пока нет. Они появляются здесь, когда ставятся из каталога или лежат в сборках.
    <button class="link" onclick={() => ((catalog.kind = "mod"), (catalog.view = "catalog"))}>Открыть каталог</button>
  </p>
{:else}
  <div class="summary">
    <span>Модов: <b>{lib.mods.length}</b> · всего <b>{size(lib.total)}</b></span>
    {#if idle.length}
      <span class="muted">· пустуют {idle.length} ({size(lib.unused)})</span>
      <button class="btn btn-sm" onclick={() => askRemove(null)}><Icon name="trash" size={16} />Удалить неиспользуемые</button>
    {/if}
  </div>

  <div class="bar">
    <label class="search">
      <Icon name="search" size={16} />
      <input type="search" bind:value={query} placeholder="Найти мод" spellcheck="false" />
    </label>
    <div class="chips" role="radiogroup" aria-label="Какие моды показывать">
      <button class="chip" role="radio" aria-checked={show === "all"} class:on={show === "all"} onclick={() => (show = "all")}>Все</button>
      <button class="chip" role="radio" aria-checked={show === "unused"} class:on={show === "unused"} onclick={() => (show = "unused")}>
        Пустуют <span class="n">{idle.length}</span>
      </button>
      <button
        class="chip"
        role="radio"
        aria-checked={show === "own"}
        class:on={show === "own"}
        title="Файлы, добавленные в сборки вручную, а не из каталога"
        onclick={() => (show = "own")}>Свои файлы <span class="n">{ownCount}</span></button
      >
    </div>
  </div>

  <div class="pickbar" class:on={picked.length > 0}>
    <label class="all"><input type="checkbox" checked={allShown} onchange={(e) => toggleAll(e.currentTarget.checked)} />Выбрать все</label>
    {#if picked.length}
      <span>Выбрано: <b>{picked.length}</b></span>
      <span class="grow"></span>
      <span class="muted small">в сборку</span>
      <div class="target"><Select label="Сборка" value={target} options={targets} onchange={(v) => (target = v)} /></div>
      <button class="btn btn-primary btn-sm" disabled={!target} onclick={addPicked}><Icon name="plus" size={16} />Добавить</button>
      {#if pickedIdle.length}
        <button class="btn btn-sm btn-quiet" onclick={() => askRemove(pickedIdle)}><Icon name="trash" size={16} />Удалить пустующие ({pickedIdle.length})</button>
      {/if}
    {/if}
  </div>
  {#if mismatch.length}
    <p class="warn small">
      <Icon name="warning" size={16} />Не для {loaderTitle(targetBuild!.loader)}: {mismatch.map((m) => m.title).join(", ")} — в этой сборке не загрузятся.
    </p>
  {/if}

  {#if shown.length === 0}
    <p class="note">Ничего не нашлось.</p>
  {:else}
    <ul class="rows">
      {#each shown as m (m.key)}
        <li class:sel={picked.includes(m.key)}>
          <input type="checkbox" aria-label="Выбрать {m.title}" checked={picked.includes(m.key)} onchange={(e) => toggle(m.key, e.currentTarget.checked)} />
          {#if m.icon_url}
            <img src={m.icon_url} alt="" width="32" height="32" loading="lazy" />
          {:else}
            <span class="noicon"><Icon name="mods" size={18} /></span>
          {/if}
          <span class="grow info">
            <span class="name">
              <b>{m.title}</b>
              {#if m.version}<span class="muted small">{m.version}</span>{/if}
              {#if !m.in_library}<span class="tag" title="Добавлен в сборку вручную, не из каталога">свой файл</span>{/if}
            </span>
            <span class="muted small where">
              {#if m.builds.length === 0}
                <span class="idle">ни в одной сборке</span>
              {:else}
                {#each m.builds as u, i (u.build)}
                  {#if i > 0}<span>, </span>{/if}
                  <button class="link" class:off={!u.enabled} title={u.enabled ? "Открыть сборку" : "Выключен в этой сборке"} onclick={() => openBuild(u.build, "mods")}>{u.name}</button>
                {/each}
              {/if}
            </span>
          </span>
          <span class="muted small">{size(m.size)}</span>
        </li>
      {/each}
    </ul>
  {/if}
{/if}

{#if confirm}
  <Modal title="Удалить неиспользуемые моды?" width={480} onclose={() => (confirm = null)}>
    <p>
      Удалится модов: <b>{confirm.count}</b>, освободится <b>{size(confirm.bytes)}</b>. Они не стоят ни в одной сборке; из
      каталога их можно скачать снова.
    </p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (confirm = null)}>Отмена</button>
      <button class="btn btn-primary" onclick={removeConfirmed}>Удалить</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .summary {
    display: flex;
    align-items: center;
    gap: var(--s3);
    flex-wrap: wrap;
    margin-bottom: var(--s3);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--s3);
    flex-wrap: wrap;
    margin-bottom: var(--s3);
  }
  .search {
    display: flex;
    align-items: center;
    gap: var(--s2);
    height: var(--h-md, 36px);
    padding: 0 var(--s3);
    width: 300px;
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--bg);
    color: var(--text-muted);
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .search input {
    flex: 1;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
  }
  .chips {
    display: flex;
    gap: var(--s2);
  }
  .n {
    opacity: 0.7;
    margin-left: 2px;
  }
  .pickbar {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: var(--s3);
    min-height: 44px;
    padding: var(--s1) var(--s3);
    margin-bottom: var(--s2);
    border: 1px solid transparent;
    border-radius: var(--r);
    background: var(--bg);
  }
  .pickbar.on {
    border-color: var(--accent);
    background: var(--surface);
  }
  .all {
    display: flex;
    align-items: center;
    gap: var(--s2);
    cursor: pointer;
  }
  .target {
    width: 300px;
  }
  li.sel {
    border-color: var(--accent);
  }
  img,
  .noicon {
    width: 32px;
    height: 32px;
    border-radius: 6px;
    flex: none;
  }
  .noicon {
    display: grid;
    place-items: center;
    background: var(--surface-raised);
    color: var(--text-muted);
  }
  .info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name {
    display: flex;
    align-items: baseline;
    gap: var(--s2);
    flex-wrap: wrap;
  }
  .tag {
    padding: 0 6px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    font-size: var(--fs-12);
    color: var(--text-muted);
  }
  .where .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-muted);
    text-decoration: underline;
    cursor: pointer;
    font-size: inherit;
  }
  .where .link:hover {
    color: var(--text);
  }
  .where .link.off {
    text-decoration: line-through;
  }
  .idle {
    color: var(--warning, #e0a43a);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-decoration: underline;
    cursor: pointer;
  }
  p {
    margin: 0 0 var(--s3);
  }
</style>
