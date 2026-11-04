<script lang="ts">
  // «Играть на сервере»: запустить сборку и сразу зайти на сервер. Какая
  // сборка подходит, решает ядро (с какой играли, версия сервера); подходящая
  // запускается сразу, иначе — список на выбор. Стрелка — выбрать самому.
  import { fly } from "svelte/transition";
  import { api, QUICK_ID, type ServerChoice, type ServerRow, type ServerStatus } from "../../lib/api";
  import { loaderLabel } from "../../lib/loaders";
  import { ms } from "../../lib/motion";
  import { busyLabel, cancel, cancellable, play, start } from "../../lib/play.svelte";
  import { servers } from "../../lib/servers.svelte";
  import { toast } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";

  let { server, status }: { server: ServerRow; status: ServerStatus | null } = $props();

  let open = $state(false);
  let choices = $state<ServerChoice[] | null>(null);
  let root: HTMLDivElement;

  const GOOD = ["last", "exact", "range"];
  // Эту сборку запустили на этот сервер — кнопка показывает, что идёт.
  const mine = $derived(
    servers.launch?.address === server.address && play.status.build === servers.launch.build && play.status.phase !== "idle",
  );
  const label = $derived(mine ? busyLabel(servers.launch!.build) : null);
  const busy = $derived(play.status.phase !== "idle");

  async function load() {
    try {
      choices = await api.serverChoices(server.address, status?.version || null, !!status?.modded);
    } catch (e) {
      toast(`Сборки для сервера: ${e}`, { error: true });
      choices = [];
    }
    return choices;
  }

  async function quick() {
    const list = await load();
    if (list.length && GOOD.includes(list[0].fit)) go(list[0]);
    else show();
  }

  // У нижних строк списка меню открывается вверх — иначе уйдёт за край окна.
  let up = $state(false);
  function show() {
    up = root.getBoundingClientRect().bottom + 380 > window.innerHeight;
    open = true;
  }

  async function toggle() {
    if (open) return (open = false);
    show();
    await load();
  }

  function go(c: ServerChoice) {
    open = false;
    servers.launch = { address: server.address, build: c.id };
    start(c.id, c.id === QUICK_ID ? c.version : null, false, false, server.address);
  }

  const groups = $derived.by(() => {
    const list = choices ?? [];
    return [
      { title: "Подходят", items: list.filter((c) => GOOD.includes(c.fit)) },
      { title: "Версия сервера неизвестна", items: list.filter((c) => c.fit === "unknown") },
      { title: "Могут не подойти", items: list.filter((c) => c.fit === "no") },
    ].filter((g) => g.items.length);
  });
</script>

<svelte:window
  onmousedown={(e) => open && !root.contains(e.target as Node) && (open = false)}
  onkeydown={(e) => open && e.key === "Escape" && (open = false)}
/>

<div class="play" bind:this={root}>
  {#if mine && cancellable(servers.launch!.build)}
    <button class="btn btn-sm" title="Отменить запуск" onclick={cancel}><Icon name="stop" size={16} />{label}</button>
  {:else if mine}
    <button class="btn btn-sm" disabled>{label}</button>
  {:else}
    <div class="split">
      <button
        class="btn btn-sm btn-primary main"
        disabled={busy}
        title={busy ? "Игра уже запущена или готовится к запуску" : "Запустить подходящую сборку и сразу зайти на сервер"}
        onclick={quick}><Icon name="play" size={16} />Играть</button
      >
      <button
        class="btn btn-sm btn-primary arrow"
        disabled={busy}
        aria-label="Выбрать сборку для сервера {server.name}"
        title="Выбрать сборку"
        aria-expanded={open}
        onclick={toggle}><Icon name="chevron-down" size={16} /></button
      >
    </div>
  {/if}
  {#if open}
    <div class="menu" class:up role="menu" in:fly={{ y: -4, duration: ms(120) }} out:fly={{ y: -4, duration: ms(80) }}>
      <p class="head small">Чем заходить на «{server.name}»</p>
      {#if choices === null}
        <p class="muted small pad">Ищем сборки…</p>
      {:else if !choices.length}
        <p class="muted small pad">Сборок пока нет — создайте её в разделе «Сборки».</p>
      {/if}
      {#each groups as g (g.title)}
        <p class="group">{g.title}</p>
        {#each g.items as c (c.id + c.version)}
          <button role="menuitem" onclick={() => go(c)}>
            <span class="name">{c.name}</span>
            <span class="sub">
              {c.version} · {loaderLabel(c)}{#if c.note}<span class:bad={c.fit === "no"}>{` · ${c.note}`}</span>{/if}
            </span>
          </button>
        {/each}
      {/each}
    </div>
  {/if}
</div>

<style>
  .play {
    position: relative;
  }
  .split {
    display: flex;
  }
  .main {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
  }
  .arrow {
    padding: 0 6px;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
    border-left: 1px solid color-mix(in srgb, var(--on-accent) 25%, transparent);
  }
  .btn:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 4px);
    z-index: 60;
    width: 340px;
    max-height: 360px;
    overflow-y: auto;
    padding: var(--s1);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--r);
  }
  .menu.up {
    top: auto;
    bottom: calc(100% + 4px);
  }
  .head {
    margin: var(--s1) var(--s3) var(--s2);
    font-weight: 600;
  }
  .pad {
    margin: 0 var(--s3) var(--s2);
  }
  .group {
    margin: var(--s2) var(--s3) var(--s1);
    font-size: var(--fs-12);
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  button[role="menuitem"] {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    padding: 6px var(--s3);
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    text-align: left;
    cursor: pointer;
  }
  button[role="menuitem"]:hover,
  button[role="menuitem"]:focus-visible {
    outline: none;
    background: var(--surface-raised);
  }
  .name {
    font-size: var(--fs-14);
    font-weight: 500;
  }
  .sub {
    font-size: var(--fs-12);
    color: var(--text-muted);
  }
  .bad {
    color: var(--warning);
  }
</style>
