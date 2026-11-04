<script lang="ts">
  // Миры сборки: название из level.dat, размер, когда играл.
  import { api, QUICK_ID, type World } from "../../lib/api";
  import { builds } from "../../lib/builds.svelte";
  import { ago, size } from "../../lib/format";
  import { attempt, toast } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";
  import Modal from "../Modal.svelte";
  import Select from "../Select.svelte";

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

  // «Сделать бэкап».
  let backingUp = $state<string | null>(null);
  async function backup(w: World) {
    backingUp = w.folder;
    const file = await attempt("Бэкап мира", () => api.backupWorld(id, w.folder));
    backingUp = null;
    if (file) toast(`Бэкап мира «${w.name}» готов: backups/${file}`);
  }

  // «Скопировать мир в другую сборку».
  let copying: World | null = $state(null);
  let target: string | null = $state(null);
  const targets = $derived([
    { value: null as string | null, label: "Выберите сборку…" },
    ...builds.list
      .filter((b) => b.state === "active" && b.id !== id)
      .map((b) => ({ value: b.id as string | null, label: b.id === QUICK_ID ? "Быстрая игра" : `${b.name} · ${b.version}` })),
  ]);
  async function copy() {
    const w = copying!;
    const to = target!;
    copying = null;
    const folder = await attempt("Копирование мира", () => api.copyWorld(id, w.folder, to));
    if (folder) toast(`Мир «${w.name}» скопирован в «${targets.find((t) => t.value === to)?.label}»`);
  }
</script>

<div class="bar">
  <span class="muted small grow" title="Чтобы вернуть мир из бэкапа, распакуйте архив в папку «saves» сборки">
    Бэкапы миров — zip-архивы в папке «backups» сборки.
  </span>
  <button class="btn btn-sm" onclick={() => attempt("Папка", () => api.open(id, { kind: "folder", name: "backups" }))}>
    <Icon name="archive" size={16} />Бэкапы
  </button>
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
        {#if w.version}<span class="muted small">{w.version}</span>{/if}
        <span class="muted small">{size(w.size)}</span>
        <span class="muted small when">{ago(w.last_played)}</span>
        <button
          class="btn btn-sm btn-quiet"
          aria-label="Сделать бэкап мира {w.name}"
          title="Сделать бэкап"
          disabled={backingUp !== null}
          onclick={() => backup(w)}><Icon name={backingUp === w.folder ? "refresh" : "archive"} size={16} /></button
        >
        <button
          class="btn btn-sm btn-quiet"
          aria-label="Скопировать мир {w.name} в другую сборку"
          title="Скопировать в другую сборку"
          onclick={() => ((copying = w), (target = null))}><Icon name="copy" size={16} /></button
        >
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

{#if copying}
  <Modal title="Скопировать «{copying.name}»" width={460} onclose={() => (copying = null)}>
    <p class="muted small">
      Копия мира появится в выбранной сборке, этот мир останется на месте. Открывать мир в версии старее той, где в нём
      играли, опасно: он может испортиться.
    </p>
    <Select label="Сборка" value={target} options={targets} onchange={(v) => (target = v)} />
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (copying = null)}>Отмена</button>
      <button class="btn btn-primary" disabled={!target} onclick={copy}>Скопировать</button>
    {/snippet}
  </Modal>
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
