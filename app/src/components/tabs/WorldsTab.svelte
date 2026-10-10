<script lang="ts">
  // Миры сборки: название из level.dat, размер, когда играл.
  import { api, type World } from "../../lib/api";
  import { ago, size } from "../../lib/format";
  import { attempt } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";

  let { id }: { id: string } = $props();
  let list: World[] | null = $state(null);
  let error = $state("");

  async function load() {
    try {
      list = await api.worlds(id);
    } catch (e) {
      error = String(e);
    }
  }
  load();
</script>

<div class="bar">
  <span class="muted small grow">Бэкап мира и откат — со снапшотами на этапе 6.</span>
  <button class="btn btn-sm" onclick={() => attempt("Папка", () => api.open(id, { kind: "folder", name: "saves" }))}>
    <Icon name="folder" size={16} />Открыть папку
  </button>
</div>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if list && list.length === 0}
  <p class="note">Миров пока нет — они появятся, когда создадите мир в игре.</p>
{:else if list}
  <ul class="rows">
    {#each list as w (w.folder)}
      <li>
        {#if w.icon}<img class="icon" src={w.icon} alt="" />{:else}<span class="icon"><Icon name="globe" size={20} /></span>{/if}
        <span class="grow">
          {w.name}
          {#if w.name !== w.folder}<span class="muted small">· папка {w.folder}</span>{/if}
        </span>
        <span class="muted small">{size(w.size)}</span>
        <span class="muted small when">{ago(w.last_played)}</span>
        <button
          class="btn btn-sm btn-quiet"
          aria-label="Открыть папку мира {w.name}"
          title="Открыть папку мира"
          onclick={() => attempt("Папка мира", () => api.open(id, { kind: "world", folder: w.folder }))}
          ><Icon name="folder" size={16} /></button
        >
      </li>
    {/each}
  </ul>
{/if}

<style>
  .icon {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: var(--r-sm);
    background: var(--surface-raised);
    image-rendering: pixelated;
  }
  .when {
    width: 140px;
    text-align: right;
  }
</style>
