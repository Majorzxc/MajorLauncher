<script lang="ts">
  import type { Entry } from "../lib/api";
  import { ago, playtime, size } from "../lib/format";
  import Icon from "./Icon.svelte";
  import Identicon from "./Identicon.svelte";

  let {
    entry,
    cover,
    bytes,
    busy,
    onopen,
    onmenu,
    onrestore,
  }: {
    entry: Entry;
    cover: string | null | undefined;
    bytes: number | undefined;
    busy: string | undefined;
    onopen: () => void;
    onmenu: (x: number, y: number) => void;
    onrestore: () => void;
  } = $props();

  const archived = $derived(entry.state === "archived");
</script>

<div
  class="tile"
  class:archived
  role="button"
  tabindex="0"
  aria-label={entry.name}
  onclick={() => !archived && !busy && onopen()}
  onkeydown={(e) => e.key === "Enter" && !archived && onopen()}
  oncontextmenu={(e) => {
    e.preventDefault();
    if (!busy) onmenu(e.clientX, e.clientY);
  }}
>
  <div class="cover">
    {#if cover}
      <img src={cover} alt="" draggable="false" />
    {:else}
      <Identicon seed={entry.id} />
    {/if}
    {#if entry.pinned && !archived}<span class="badge pin" title="Закреплена"><Icon name="pin" size={14} /></span>{/if}
    {#if !busy}
      <button
        class="more"
        aria-label="Действия со сборкой"
        onclick={(e) => {
          e.stopPropagation();
          const r = e.currentTarget.getBoundingClientRect();
          onmenu(r.left, r.bottom + 4);
        }}><Icon name="more" size={18} /></button
      >
    {/if}
    {#if busy}<span class="busy">{busy}</span>{/if}
  </div>

  <div class="info">
    <b class="name">{entry.name}</b>
    <span class="muted">{entry.version} · Без модов</span>
    {#if archived}
      <span class="muted row">
        <Icon name="archive" size={14} />в архиве{#if entry.archive_size !== null}, {size(entry.archive_size)}{/if}
      </span>
      <button
        class="btn btn-sm restore"
        disabled={!!busy}
        onclick={(e) => {
          e.stopPropagation();
          onrestore();
        }}><Icon name="restore" size={16} />Восстановить</button
      >
    {:else}
      <span class="muted row">
        <span title="Последний запуск"><Icon name="time" size={14} />{ago(entry.last_played)}</span>
      </span>
      <span class="muted row">
        <span title="Размер на диске"><Icon name="folder" size={14} />{bytes === undefined ? "…" : size(bytes)}</span>
        <span title="Время в игре"><Icon name="stats" size={14} />{playtime(entry.playtime)}</span>
      </span>
    {/if}
  </div>
</div>

<style>
  .tile {
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-card);
    overflow: hidden;
    cursor: pointer;
    transition: transform var(--t-fast) var(--ease);
  }
  .tile:hover {
    border-color: var(--border-strong);
  }
  .tile:active {
    transform: translateY(1px);
  }
  .tile:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .archived {
    cursor: default;
  }
  .archived .cover {
    filter: grayscale(1);
    opacity: 0.6;
  }
  .cover {
    position: relative;
    aspect-ratio: 1;
    display: grid;
    place-items: center;
    background: var(--surface-raised);
    border-bottom: 1px solid var(--border);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    image-rendering: pixelated;
  }
  .badge {
    position: absolute;
    top: var(--s2);
    left: var(--s2);
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: var(--r-sm);
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .more {
    position: absolute;
    top: var(--s2);
    right: var(--s2);
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--surface);
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--t-fast) var(--ease);
  }
  .tile:hover .more,
  .tile:focus-within .more {
    opacity: 1;
  }
  .busy {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.55);
    color: #fff;
    font-weight: 600;
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--s3) var(--s3) var(--s4);
    font-size: var(--fs-14);
  }
  .name {
    font-size: var(--fs-16);
    font-weight: 600;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .row {
    display: flex;
    gap: var(--s3);
  }
  .row span,
  .row {
    align-items: center;
  }
  .row span {
    display: inline-flex;
    gap: 4px;
  }
  .row :global(.i) {
    flex: none;
  }
  .restore {
    margin-top: var(--s2);
    align-self: flex-start;
  }
</style>
