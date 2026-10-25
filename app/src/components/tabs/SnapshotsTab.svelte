<script lang="ts">
  // Снапшоты сборки: лаунчер сохраняет их сам перед каждым изменением модов и
  // загрузчика; откат возвращает моды, их настройки и версии одним действием.
  import { api, type SnapshotInfo } from "../../lib/api";
  import { refresh } from "../../lib/builds.svelte";
  import { ago, count, plural, size } from "../../lib/format";
  import { loaderLabel } from "../../lib/loaders";
  import { play } from "../../lib/play.svelte";
  import { attempt, toast } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";
  import Modal from "../Modal.svelte";

  let { id }: { id: string } = $props();
  let list: SnapshotInfo[] | null = $state(null);
  let error = $state("");
  let busy = $state(false);
  let open = $state<string | null>(null);
  let asking = $state<SnapshotInfo | null>(null);

  async function load() {
    try {
      list = await api.snapshots(id);
      error = "";
    } catch (e) {
      error = String(e);
    }
  }
  load();

  // Пока сборка готовится к запуску или идёт, откатывать нельзя: игра держит моды.
  const playing = $derived(play.status.phase !== "idle" && play.status.build === id);

  const mods = (n: number) => `${count(n)} ${plural(n, ["мод", "мода", "модов"])}`;

  /// «сегодня, 14:05», «вчера, 09:12», «9 октября, 14:05»: снапшоты бывают
  /// по несколько в день — без времени их не различить.
  function when(unix: number): string {
    const d = new Date(unix * 1000);
    const time = d.toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" });
    const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
    const days = Math.round((day(new Date()) - day(d)) / 86_400_000);
    if (days === 0) return `сегодня, ${time}`;
    if (days === 1) return `вчера, ${time}`;
    const sameYear = d.getFullYear() === new Date().getFullYear();
    const text = d.toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: sameYear ? undefined : "numeric" });
    return `${text.replace(/\s*г\.$/, "")}, ${time}`;
  }

  async function take() {
    busy = true;
    const done = await attempt("Снапшот", () => api.snapshotTake(id));
    busy = false;
    if (done !== undefined) {
      toast("Снапшот сохранён");
      load();
    }
  }

  async function restore() {
    const s = asking!;
    asking = null;
    busy = true;
    const done = await attempt("Откат", () => api.snapshotRestore(id, s.id));
    busy = false;
    if (done !== undefined) {
      toast(`Сборка откатилась к снапшоту «${when(s.created)}»`);
      refresh();
      load();
    }
  }

  async function remove(s: SnapshotInfo) {
    const done = await attempt("Удаление снапшота", () => api.snapshotDelete(id, s.id));
    if (done !== undefined) load();
  }
</script>

<div class="bar">
  <span class="muted small hint">
    Лаунчер сам сохраняет снапшот перед установкой, обновлением и удалением модов и перед сменой загрузчика. Откат
    возвращает моды, их настройки и версию загрузчика, миры не трогает.
  </span>
  <button class="btn btn-sm" disabled={busy} onclick={take}><Icon name="plus" size={16} />Сделать снапшот</button>
</div>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if list && list.length === 0}
  <p class="note">Снапшотов пока нет — первый появится перед первым изменением модов.</p>
{:else if list}
  <ul class="rows">
    {#each list as s (s.id)}
      {@const changes = s.added.length + s.removed.length}
      <li class="snap">
        <div class="line">
          <span class="grow">
            <b title={ago(s.created)}>{when(s.created)}</b>
            <span class="muted">· {s.reason}</span>
            {#if s.manual}<span class="tag">вручную</span>{/if}
            {#if s.current}<span class="tag ok">как сейчас</span>{/if}
          </span>
          {#if s.size > 0}<span class="muted small" title="Сколько места снапшот занимает сам">{size(s.size)}</span>{/if}
          <button
            class="btn btn-sm"
            disabled={busy || s.current || playing}
            title={playing ? "Сборка запущена — откатите её после игры" : s.current ? "Сборка уже такая" : undefined}
            onclick={() => (asking = s)}><Icon name="restore" size={16} />Откатить</button
          >
          <button
            class="btn btn-sm btn-quiet"
            aria-label="Удалить снапшот"
            title="Удалить снапшот"
            disabled={busy}
            onclick={() => remove(s)}><Icon name="trash" size={16} /></button
          >
        </div>
        <div class="muted small sub">
          {s.version} · {loaderLabel(s)} · {mods(s.mods)}
          {#if changes}
            · при откате
            {[s.removed.length ? `вернётся ${count(s.removed.length)}` : "", s.added.length ? `уйдёт ${count(s.added.length)}` : ""]
              .filter(Boolean)
              .join(", ")}
            <button class="link" onclick={() => (open = open === s.id ? null : s.id)}>
              {open === s.id ? "скрыть" : "какие"}
            </button>
          {/if}
        </div>
        {#if open === s.id}
          <div class="diff small">
            {#each s.removed as f (f)}<span class="plus">+ {f}</span>{/each}
            {#each s.added as f (f)}<span class="minus">− {f}</span>{/each}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

{#if asking}
  {@const s = asking}
  <Modal title="Откатить сборку?" width={520} onclose={() => (asking = null)}>
    <p>Сборка станет такой, какой была {when(s.created)} ({s.reason}).</p>
    <ul class="what">
      <li>{s.version} · {loaderLabel(s)}</li>
      {#if s.removed.length}<li>вернутся {mods(s.removed.length)}</li>{/if}
      {#if s.added.length}<li>уйдут {mods(s.added.length)} — свои файлы останутся в библиотеке модов</li>{/if}
      <li>настройки модов — как в снапшоте</li>
    </ul>
    <p class="muted small">
      Миры, скриншоты и настройки игры не меняются. Текущее состояние сохранится снапшотом «перед откатом» — откат можно
      отменить.
    </p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (asking = null)}>Отмена</button>
      <button class="btn btn-primary" onclick={restore}>Откатить</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .snap {
    display: block;
  }
  .hint {
    flex: 1;
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--s3);
  }
  .sub {
    margin-top: 2px;
  }
  .tag {
    margin-left: var(--s2);
    padding: 1px 6px;
    border-radius: var(--r-sm);
    background: var(--surface-raised);
    font-size: var(--fs-12);
  }
  .tag.ok {
    color: var(--accent);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }
  .diff {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: var(--s2);
    padding: var(--s2) var(--s3);
    border-radius: var(--r);
    background: var(--bg);
    font-family: Consolas, monospace;
    user-select: text;
  }
  .plus {
    color: var(--text);
  }
  .minus {
    color: var(--danger);
  }
  .what {
    margin: 0 0 var(--s3);
    padding-left: var(--s5);
  }
  p {
    margin: 0 0 var(--s3);
  }
</style>
