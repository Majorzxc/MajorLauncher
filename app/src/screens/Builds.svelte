<script lang="ts">
  import ContextMenu, { type MenuItem } from "../components/ContextMenu.svelte";
  import BuildTile from "../components/BuildTile.svelte";
  import EditBuild from "../components/EditBuild.svelte";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";
  import Wizard from "../components/Wizard.svelte";
  import { api, TRASH_DAYS, type Entry, type TrashEntry } from "../lib/api";
  import { builds, forget, refresh, withBusy } from "../lib/builds.svelte";
  import { plural, size, trashLeft } from "../lib/format";
  import { attempt, toast } from "../lib/toast.svelte";

  type Filter = { kind: "all" } | { kind: "pinned" } | { kind: "archive" } | { kind: "group"; name: string };
  type Sort = "played" | "name" | "created" | "playtime";

  let query = $state("");
  let filter: Filter = $state({ kind: "all" });
  let sort: Sort = $state("played");
  let view: "grid" | "trash" = $state("grid");
  let trash: TrashEntry[] = $state([]);

  let menu: { x: number; y: number; items: MenuItem[] } | null = $state(null);
  let editing: Entry | null = $state(null);
  let creating = $state(false);
  let addingGroup = $state(false);
  let newGroup = $state("");
  let confirm: { title: string; text: string; action: string; run: () => void } | null = $state(null);

  refresh();
  loadTrash();

  async function loadTrash() {
    trash = (await attempt("Корзина", api.trashList)) ?? [];
  }

  const active = $derived(builds.list.filter((b) => b.state === "active"));
  const archived = $derived(builds.list.filter((b) => b.state === "archived"));

  const shown = $derived.by(() => {
    let list = filter.kind === "archive" ? archived : active;
    if (filter.kind === "pinned") list = list.filter((b) => b.pinned);
    if (filter.kind === "group") {
      const name = filter.name;
      list = list.filter((b) => b.group === name);
    }
    const q = query.trim().toLowerCase();
    if (q) list = list.filter((b) => b.name.toLowerCase().includes(q) || b.version.includes(q));
    const by: Record<Sort, (a: Entry, b: Entry) => number> = {
      played: (a, b) => (b.last_played ?? b.created) - (a.last_played ?? a.created),
      name: (a, b) => a.name.localeCompare(b.name, "ru"),
      created: (a, b) => b.created - a.created,
      playtime: (a, b) => b.playtime - a.playtime,
    };
    // Закреплённые всегда сверху (раздел 6.2 ТЗ).
    return [...list].sort((a, b) => Number(b.pinned) - Number(a.pinned) || by[sort](a, b));
  });

  const isFilter = (f: Filter) =>
    f.kind === filter.kind && (f.kind !== "group" || (filter.kind === "group" && filter.name === f.name));

  function openMenu(b: Entry, x: number, y: number) {
    if (b.state === "archived") {
      menu = {
        x,
        y,
        items: [
          { label: "Восстановить из архива", icon: "restore", onclick: () => unarchive(b) },
          { label: "Удалить", icon: "trash", danger: true, divider: true, onclick: () => trashArchived(b) },
        ],
      };
      return;
    }
    menu = {
      x,
      y,
      items: [
        { label: "Играть", icon: "play", hint: "этап 3.3" },
        { label: "Редактировать", icon: "edit", onclick: () => (editing = b) },
        { label: "Открыть папку", icon: "folder", onclick: () => attempt("Папка", () => api.openFolder(b.id)) },
        { label: "Копировать", icon: "copy", onclick: () => duplicate(b) },
        { label: "Экспорт", icon: "export", hint: "этап 6" },
        { label: "Снапшот", icon: "snapshot", hint: "этап 6" },
        {
          label: b.pinned ? "Открепить" : "Закрепить",
          icon: "pin",
          divider: true,
          onclick: () => attempt("Закрепление", () => api.update(b.id, { pinned: !b.pinned }).then(refresh)),
        },
        { label: "В архив", icon: "archive", onclick: () => archive(b) },
        { label: "Удалить", icon: "trash", danger: true, divider: true, onclick: () => trashBuild(b) },
      ],
    };
  }

  async function duplicate(b: Entry) {
    const copy = await attempt("Копирование", () => withBusy(b.id, "Копирую…", () => api.duplicate(b.id)));
    if (copy) {
      toast(`Создана «${copy.name}»`);
      refresh();
    }
  }

  async function trashBuild(b: Entry) {
    // void-операции возвращают true, чтобы отличить успех от ошибки (там undefined).
    const ok = await attempt("Удаление", () => withBusy(b.id, "Удаляю…", () => api.trash(b.id).then(() => true)));
    if (!ok) return;
    forget(b.id);
    await Promise.all([refresh(), loadTrash()]);
    const entry = trash.find((t) => t.instance.id === b.id);
    toast(`«${b.name}» в корзине на ${TRASH_DAYS} дней`, {
      action: entry ? { label: "Вернуть", run: () => restore(entry) } : undefined,
    });
  }

  async function trashArchived(b: Entry) {
    // Архивную сборку сначала распаковываем: в корзину кладётся обычная папка.
    const back = await attempt("Удаление", () => withBusy(b.id, "Распаковываю…", () => api.unarchive(b.id)));
    if (back) await trashBuild({ ...b, ...back, state: "active" });
  }

  async function archive(b: Entry) {
    const ok = await attempt("Архив", () =>
      withBusy(b.id, "Упаковываю в архив…", () => api.archive(b.id).then(() => true)),
    );
    if (!ok) return;
    forget(b.id);
    await refresh();
    const a = builds.list.find((x) => x.id === b.id && x.state === "archived");
    toast(`«${b.name}» в архиве${a?.archive_size ? `: ${size(a.archive_size)}` : ""}`);
  }

  async function unarchive(b: Entry) {
    const back = await attempt("Восстановление", () => withBusy(b.id, "Распаковываю…", () => api.unarchive(b.id)));
    if (!back) return;
    forget(b.id);
    toast(`«${back.name}» снова в сборках`);
    refresh();
  }

  async function restore(t: TrashEntry) {
    const back = await attempt("Восстановление", () => api.restore(t.trash_id));
    if (!back) return;
    toast(`«${back.name}» восстановлена`);
    await Promise.all([refresh(), loadTrash()]);
  }

  function purge(t: TrashEntry) {
    confirm = {
      title: "Удалить навсегда?",
      text: `«${t.instance.name}» удалится вместе с мирами без возможности вернуть.`,
      action: "Удалить навсегда",
      run: async () => {
        await attempt("Удаление", () => api.purge(t.trash_id));
        loadTrash();
      },
    };
  }

  function purgeAll() {
    confirm = {
      title: "Очистить корзину?",
      text: `Все сборки из корзины (${trash.length}) удалятся вместе с мирами без возможности вернуть.`,
      action: "Очистить корзину",
      run: async () => {
        for (const t of trash) await attempt("Удаление", () => api.purge(t.trash_id));
        loadTrash();
      },
    };
  }

  async function addGroup() {
    const name = newGroup.trim();
    addingGroup = false;
    newGroup = "";
    if (!name) return;
    await attempt("Группа", () => api.addGroup(name));
    await refresh();
    filter = { kind: "group", name };
  }

  function groupMenu(name: string, x: number, y: number) {
    menu = {
      x,
      y,
      items: [
        {
          label: "Удалить группу",
          icon: "trash",
          danger: true,
          onclick: async () => {
            await attempt("Группа", () => api.deleteGroup(name));
            if (filter.kind === "group" && filter.name === name) filter = { kind: "all" };
            toast(`Группа «${name}» удалена, сборки остались`);
            refresh();
          },
        },
      ],
    };
  }
</script>

{#if view === "trash"}
  <div class="head">
    <button class="btn btn-quiet" onclick={() => (view = "grid")}><Icon name="arrow-left" size={18} />Сборки</button>
    <h1>Корзина</h1>
    <span class="spacer"></span>
    {#if trash.length}
      <span class="muted">{size(trash.reduce((s, t) => s + t.size, 0))}</span>
      <button class="btn" onclick={purgeAll}><Icon name="trash" size={18} />Очистить корзину</button>
    {/if}
  </div>
  <p class="muted note">Удалённые сборки хранятся здесь {TRASH_DAYS} дней вместе с мирами, потом удаляются сами.</p>
  {#if trash.length === 0}
    <div class="empty"><Icon name="trash" size={32} /><p class="muted">Корзина пуста</p></div>
  {:else}
    <div class="rows">
      {#each trash as t (t.trash_id)}
        <div class="row card">
          <div class="grow">
            <b>{t.instance.name}</b>
            <span class="muted">{t.instance.version} · {trashLeft(t.deleted_at, TRASH_DAYS)} · {size(t.size)}</span>
          </div>
          <button class="btn btn-sm" onclick={() => restore(t)}><Icon name="restore" size={16} />Восстановить</button>
          <button class="btn btn-sm btn-quiet danger" onclick={() => purge(t)}>Удалить навсегда</button>
        </div>
      {/each}
    </div>
  {/if}
{:else}
  <div class="title">
    <h1>Сборки</h1>
    <span class="muted">{active.length} {plural(active.length, ["сборка", "сборки", "сборок"])}</span>
  </div>
  <div class="head">
    <label class="search">
      <Icon name="search" size={18} />
      <input placeholder="Поиск по названию или версии" bind:value={query} />
    </label>
    <label class="sort">
      <Icon name="sort" size={18} />
      <select bind:value={sort} aria-label="Сортировка">
        <option value="played">По последнему запуску</option>
        <option value="name">По названию</option>
        <option value="created">По дате создания</option>
        <option value="playtime">По времени в игре</option>
      </select>
    </label>
    <span class="spacer"></span>
    <button class="btn btn-primary" onclick={() => (creating = true)}><Icon name="plus" size={18} />Новая сборка</button>
  </div>

  <div class="chips" role="tablist" aria-label="Группы">
    <button class="chip" class:on={isFilter({ kind: "all" })} onclick={() => (filter = { kind: "all" })}>
      Все <span>{active.length}</span>
    </button>
    <button class="chip" class:on={isFilter({ kind: "pinned" })} onclick={() => (filter = { kind: "pinned" })}>
      <Icon name="pin" size={14} />Закреплённые
    </button>
    {#each builds.groups as g (g)}
      <button
        class="chip"
        class:on={isFilter({ kind: "group", name: g })}
        onclick={() => (filter = { kind: "group", name: g })}
        oncontextmenu={(e) => {
          e.preventDefault();
          groupMenu(g, e.clientX, e.clientY);
        }}
        title="Правый клик — удалить группу">{g} <span>{active.filter((b) => b.group === g).length}</span></button
      >
    {/each}
    <button class="chip" class:on={isFilter({ kind: "archive" })} onclick={() => (filter = { kind: "archive" })}>
      <Icon name="archive" size={14} />Архив <span>{archived.length}</span>
    </button>
    {#if addingGroup}
      <input
        class="chip-input"
        placeholder="Название группы"
        maxlength="32"
        bind:value={newGroup}
        onblur={addGroup}
        onkeydown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
          if (e.key === "Escape") {
            newGroup = "";
            e.currentTarget.blur();
          }
        }}
      />
    {:else}
      <button class="chip add" onclick={() => (addingGroup = true)}><Icon name="plus" size={14} />Группа</button>
    {/if}
    <span class="spacer"></span>
    <button class="btn btn-quiet btn-sm" onclick={() => ((view = "trash"), loadTrash())}>
      <Icon name="trash" size={16} />{trash.length ? `Корзина (${trash.length})` : "Корзина"}
    </button>
  </div>

  {#if builds.error}
    <p class="warn"><Icon name="error" size={18} />{builds.error}</p>
  {:else if builds.loaded && active.length === 0 && archived.length === 0}
    <div class="empty">
      <Icon name="builds" size={32} />
      <h2>Создайте первую сборку</h2>
      <p class="muted">Сборка — отдельная папка игры со своей версией, мирами и настройками.</p>
      <button class="btn btn-primary" onclick={() => (creating = true)}><Icon name="plus" size={18} />Новая сборка</button>
    </div>
  {:else if builds.loaded && shown.length === 0}
    <div class="empty">
      <Icon name="search" size={32} />
      <p class="muted">{query ? "Ничего не найдено" : "Здесь пока пусто"}</p>
    </div>
  {:else}
    <div class="grid">
      {#each shown as b (b.state + b.id)}
        <BuildTile
          entry={b}
          cover={builds.covers[b.id]}
          bytes={builds.sizes[b.id]}
          busy={builds.busy[b.id]}
          onopen={() => (editing = b)}
          onmenu={(x, y) => openMenu(b, x, y)}
          onrestore={() => unarchive(b)}
        />
      {/each}
    </div>
  {/if}
{/if}

{#if menu}<ContextMenu {...menu} onclose={() => (menu = null)} />{/if}
{#if editing}<EditBuild entry={editing} onclose={() => (editing = null)} />{/if}
{#if creating}
  <Wizard
    onclose={() => (creating = false)}
    oncreated={(name) => {
      creating = false;
      filter = { kind: "all" };
      toast(`Сборка «${name}» создана. Игра скачается при первом запуске.`);
      refresh();
    }}
  />
{/if}
{#if confirm}
  {@const c = confirm}
  <Modal title={c.title} width={460} onclose={() => (confirm = null)}>
    <p class="confirm">{c.text}</p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (confirm = null)}>Отмена</button>
      <button
        class="btn danger-fill"
        onclick={() => {
          confirm = null;
          c.run();
        }}>{c.action}</button
      >
    {/snippet}
  </Modal>
{/if}

<style>
  .head {
    display: flex;
    align-items: center;
    gap: var(--s3);
    margin-bottom: var(--s4);
  }
  .title {
    display: flex;
    align-items: baseline;
    gap: var(--s3);
    margin-bottom: var(--s4);
  }
  h1 {
    margin: 0 var(--s3) 0 0;
    font-size: var(--fs-24);
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  .search,
  .sort {
    display: flex;
    align-items: center;
    gap: var(--s2);
    height: var(--h-md);
    padding: 0 var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--surface);
    color: var(--text-muted);
  }
  .search:focus-within,
  .sort:focus-within {
    border-color: var(--accent);
  }
  .search {
    flex: 0 1 360px;
    min-width: 200px;
  }
  .search input {
    width: 100%;
    border: 0;
    background: none;
    outline: none;
    color: var(--text);
    user-select: text;
  }
  .sort select {
    border: 0;
    background: none;
    outline: none;
    color: var(--text);
    cursor: pointer;
  }
  .sort option {
    background: var(--surface);
  }
  .chips {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--s2);
    margin-bottom: var(--s5);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: var(--h-sm);
    padding: 0 var(--s3);
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    font-size: var(--fs-14);
    cursor: pointer;
  }
  .chip span {
    color: var(--text-muted);
  }
  .chip:hover {
    border-color: var(--border-strong);
  }
  .chip.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .chip.on span {
    color: inherit;
    opacity: 0.7;
  }
  .chip.add {
    border-style: dashed;
    color: var(--text-muted);
  }
  .chip-input {
    height: var(--h-sm);
    padding: 0 var(--s3);
    border: 1px solid var(--accent);
    border-radius: 999px;
    background: var(--surface);
    outline: none;
    font-size: var(--fs-14);
    user-select: text;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: var(--s4);
  }
  .empty {
    display: grid;
    justify-items: center;
    gap: var(--s2);
    padding: var(--s7) 0;
    text-align: center;
    color: var(--text-muted);
  }
  .empty h2 {
    margin: var(--s2) 0 0;
    color: var(--text);
    font-size: var(--fs-20);
    font-weight: 600;
  }
  .empty p {
    margin: 0 0 var(--s3);
  }
  .note {
    margin: 0 0 var(--s4);
    font-size: var(--fs-14);
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--s2);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: var(--s3) var(--s4);
  }
  .grow {
    flex: 1;
    display: flex;
    flex-direction: column;
  }
  .grow span {
    font-size: var(--fs-14);
  }
  .danger {
    color: var(--danger);
  }
  .danger-fill {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }
  .confirm {
    margin: 0;
  }
  .warn {
    display: flex;
    gap: var(--s2);
    color: var(--danger);
  }
</style>
