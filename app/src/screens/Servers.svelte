<script lang="ts">
  // Раздел «Серверы» (раздел 5.10 ТЗ): общий список для всех сборок (или
  // свой у сборки), состояние каждого сервера и «Играть на сервере».
  // Переставить — перетащить строку за край слева или из меню «⋯».
  import { onMount } from "svelte";
  import { api, type ServerRow } from "../lib/api";
  import { ago, duration, plural } from "../lib/format";
  import { loadServers, openList, pingServers, probeOf, servers } from "../lib/servers.svelte";
  import { toast } from "../lib/toast.svelte";
  import ContextMenu, { type MenuItem } from "../components/ContextMenu.svelte";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";
  import Motd from "../components/servers/Motd.svelte";
  import ServerDialog from "../components/servers/ServerDialog.svelte";
  import ServerPlay from "../components/servers/ServerPlay.svelte";

  let editing = $state<ServerRow | null | "new">(null);
  let menu: { x: number; y: number; items: MenuItem[]; anchor?: Element } | null = $state(null);

  onMount(async () => {
    await loadServers();
    pingServers();
  });

  const list = $derived(servers.view?.servers ?? []);
  const lists = $derived([
    { value: null as string | null, label: "Общий список" },
    ...(servers.view?.lists ?? []).map((l) => ({ value: l.build as string | null, label: `«${l.name}» — свой список` })),
  ]);

  function statusOf(s: ServerRow) {
    const p = probeOf(s.address);
    return p && p !== "pending" ? p.status : null;
  }

  function icon(s: ServerRow): string | null {
    const b64 = statusOf(s)?.favicon ?? s.icon;
    return b64 ? `data:image/png;base64,${b64}` : null;
  }

  /// Деления связи, как в игре: до 150 мс — 5, до 300 — 4, до 600 — 3, до 1000 — 2.
  function bars(ms: number) {
    return ms < 150 ? 5 : ms < 300 ? 4 : ms < 600 ? 3 : ms < 1000 ? 2 : 1;
  }

  async function run(what: string, f: () => Promise<typeof servers.view>) {
    try {
      servers.view = await f();
    } catch (e) {
      toast(`${what}: ${e}`, { error: true });
      loadServers();
    }
  }

  function move(s: ServerRow, to: number) {
    // Места в окне — без скрытых серверов, в файле — с ними.
    const target = list[Math.max(0, Math.min(list.length - 1, to))];
    if (!target || target.index === s.index) return;
    run("Порядок серверов", () => api.serverMove(servers.build, s.index, s.address, target.index));
  }

  async function remove(s: ServerRow) {
    const at = list.indexOf(s);
    await run("Удаление", () => api.serverRemove(servers.build, s.index, s.address));
    toast(`Сервер «${s.name}» удалён`, {
      action: {
        label: "Вернуть",
        run: async () => {
          await run("Возврат", () => api.serverAdd(servers.build, s.name, s.address));
          const back = servers.view?.servers.at(-1);
          if (!back) return;
          if (s.textures !== "ask") {
            await run("Возврат", () => api.serverEdit(servers.build, back.index, back.address, { textures: s.textures }));
          }
          const now = servers.view?.servers.at(-1);
          if (now && at < list.length - 1) move(now, at);
        },
      },
    });
  }

  function openMenu(s: ServerRow, x: number, y: number, anchor?: Element) {
    const at = list.indexOf(s);
    menu = {
      x,
      y,
      anchor,
      items: [
        { label: "Изменить", icon: "edit", onclick: () => (editing = s) },
        {
          label: "Скопировать адрес",
          icon: "copy",
          onclick: () => navigator.clipboard.writeText(s.address).then(() => toast(`Адрес скопирован: ${s.address}`)),
        },
        { label: "Опросить снова", icon: "refresh", onclick: () => pingServers([s.address]) },
        { label: "Выше", icon: "chevron-up", divider: true, onclick: at > 0 ? () => move(s, at - 1) : undefined },
        { label: "Ниже", icon: "chevron-down", onclick: at < list.length - 1 ? () => move(s, at + 1) : undefined },
        { label: "Удалить", icon: "trash", danger: true, divider: true, onclick: () => remove(s) },
      ],
    };
  }

  // Перестановка мышью. Перетаскивание браузера (drag and drop) в окне
  // недоступно — его забирает окно под файлы, поэтому вручную по указателю.
  let drag = $state<{ from: number; over: number } | null>(null);
  function dragStart(e: PointerEvent, i: number) {
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { from: i, over: i };
  }
  function dragMove(e: PointerEvent) {
    if (!drag) return;
    const row = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("li[data-i]");
    if (row) drag.over = Number(row.dataset.i);
  }
  function dragEnd() {
    if (!drag) return;
    const { from, over } = drag;
    drag = null;
    if (from !== over) move(list[from], over);
  }

  const pending = $derived(list.filter((s) => probeOf(s.address) === "pending").length);
</script>

<div class="title">
  <h1>Серверы</h1>
  {#if servers.view}
    <span class="muted">{list.length} {plural(list.length, ["сервер", "сервера", "серверов"])}</span>
  {/if}
</div>
<div class="head">
  {#if lists.length > 1}
    <Select label="Список серверов" icon="servers" shrink value={servers.build} options={lists} onchange={(v) => openList(v)} />
  {/if}
  <span class="muted small grow">
    {servers.build
      ? "У этой сборки свой список: в других сборках его нет."
      : "Один список для всех сборок: сервер, добавленный здесь или в игре, есть везде."}
  </span>
  <button class="btn" disabled={!list.length || pending > 0} onclick={() => pingServers()}>
    <Icon name="refresh" size={18} />{pending ? `Опрос… ${list.length - pending}/${list.length}` : "Обновить"}
  </button>
  <button class="btn btn-primary" onclick={() => (editing = "new")}><Icon name="plus" size={18} />Добавить сервер</button>
</div>

{#if servers.error}
  <p class="warn"><Icon name="error" size={18} />{servers.error}</p>
{:else if servers.view && !list.length}
  <div class="note">
    <p>Серверов пока нет. Добавьте сервер — он появится во всех сборках и в меню «Сетевая игра».</p>
    <button class="btn btn-primary" onclick={() => (editing = "new")}><Icon name="plus" size={18} />Добавить сервер</button>
  </div>
{:else if servers.view}
  <ul class="rows" class:dragging={drag !== null}>
    {#each list as s, i (s.index + s.address)}
      {@const p = probeOf(s.address)}
      {@const st = p && p !== "pending" ? p.status : null}
      {@const src = icon(s)}
      <li
        data-i={i}
        class:lifted={drag?.from === i}
        class:above={drag && drag.over === i && drag.over < drag.from}
        class:below={drag && drag.over === i && drag.over > drag.from}
        oncontextmenu={(e) => {
          e.preventDefault();
          openMenu(s, e.clientX, e.clientY);
        }}
      >
        <span
          class="grip"
          role="button"
          tabindex="-1"
          aria-label="Переставить {s.name}"
          title="Перетащите, чтобы переставить"
          onpointerdown={(e) => dragStart(e, i)}
          onpointermove={dragMove}
          onpointerup={dragEnd}
          onpointercancel={() => (drag = null)}>⠿</span
        >
        {#if src}<img class="icon" {src} alt="" />{:else}<span class="icon"><Icon name="servers" size={22} /></span>{/if}
        <div class="main">
          <div class="line">
            <b class="name">{s.name}</b>
            <span class="muted small addr">{s.address}</span>
          </div>
          {#if p === "pending" || p === undefined}
            <span class="muted small">Опрашиваем…</span>
          {:else if st}
            <Motd spans={st.motd.length ? st.motd : [{ text: " " }]} />
          {:else}
            <span class="small off"><Icon name={p.kind === "bad" ? "warning" : "error"} size={16} />{p.error}</span>
          {/if}
          {#if s.played}
            <span class="muted small" title="Последний раз — в «{s.played.build_name}»">
              Наиграно {duration(s.played.seconds)} · {ago(s.played.last)}
            </span>
          {/if}
        </div>
        <div class="facts small">
          {#if st}
            <span
              class="players"
              title={st.sample.length ? st.sample.join("\n") : "Сервер не показывает, кто играет"}
              ><Icon name="user" size={16} /><span>{st.online ?? "?"}{#if st.max !== null}<span class="muted">/{st.max}</span>{/if}</span></span
            >
            <span class="muted version" title={st.modded ? `Сервер с модами${st.modded.loader ? ` (${st.modded.loader})` : ""}${st.modded.mods ? `: ${st.modded.mods}` : ""}` : st.version}
              >{st.version || "версия неизвестна"}{st.modded ? " · моды" : ""}</span
            >
            <span class="ping" title="Задержка {st.ping_ms} мс">
              {#each [1, 2, 3, 4, 5] as n}<i class:on={n <= bars(st.ping_ms)} style:height="{4 + n * 2}px"></i>{/each}
              <span class="muted">{st.ping_ms} мс</span>
            </span>
          {/if}
        </div>
        <ServerPlay server={s} status={st} />
        <button
          class="btn btn-sm btn-quiet"
          aria-label="Ещё — {s.name}"
          title="Ещё"
          onclick={(e) => {
            const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
            if (menu?.anchor === e.currentTarget) menu = null;
            else openMenu(s, r.left, r.bottom + 4, e.currentTarget as Element);
          }}><Icon name="more" size={16} /></button
        >
      </li>
    {/each}
  </ul>
{/if}

{#if menu}<ContextMenu {...menu} onclose={() => (menu = null)} />{/if}
{#if editing}
  <ServerDialog server={editing === "new" ? null : editing} onclose={() => (editing = null)} />
{/if}

<style>
  .title {
    display: flex;
    align-items: baseline;
    gap: var(--s2);
    margin-bottom: var(--s4);
  }
  h1 {
    margin: 0 var(--s3) 0 0;
    font-size: var(--fs-24);
    font-weight: 600;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s3);
    margin-bottom: var(--s4);
  }
  .note {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s3);
  }
  .note p {
    margin: 0;
  }
  .rows > li {
    align-items: flex-start;
    padding-top: var(--s3);
    padding-bottom: var(--s3);
  }
  .rows.dragging {
    cursor: grabbing;
  }
  li.lifted {
    opacity: 0.5;
  }
  li.above {
    box-shadow: inset 0 2px 0 var(--accent);
  }
  li.below {
    box-shadow: inset 0 -2px 0 var(--accent);
  }
  .grip {
    align-self: center;
    margin-left: calc(-1 * var(--s2));
    padding: var(--s1);
    color: var(--text-muted);
    cursor: grab;
    touch-action: none;
  }
  .grip:hover {
    color: var(--text);
  }
  .icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 48px;
    height: 48px;
    border-radius: var(--r-sm);
    background: var(--surface-raised);
    color: var(--text-muted);
    image-rendering: pixelated;
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-width: 0;
  }
  .line {
    display: flex;
    align-items: baseline;
    gap: var(--s2);
    min-width: 0;
  }
  .name,
  .addr {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    flex: none;
    max-width: 60%;
    font-weight: 600;
  }
  .off {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-muted);
  }
  .facts {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    width: 150px;
    flex: none;
    text-align: right;
  }
  .players {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-weight: 500;
  }
  .version {
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ping {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
  }
  .ping i {
    width: 3px;
    border-radius: 1px;
    background: var(--border);
  }
  .ping i.on {
    background: var(--accent);
  }
  .ping .muted {
    margin-left: 4px;
    line-height: 1;
  }
</style>
