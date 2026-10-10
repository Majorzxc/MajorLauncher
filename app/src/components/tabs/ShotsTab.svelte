<script lang="ts">
  // Скриншоты: миниатюры (их делает ядро — полный кадр занял бы в окне
  // 8 МБ памяти), открыть в просмотрщике, сделать обложкой.
  import { untrack } from "svelte";
  import { api, type FileEntry } from "../../lib/api";
  import { builds } from "../../lib/builds.svelte";
  import { crop } from "../../lib/crop.svelte";
  import { date } from "../../lib/format";
  import { attempt, toast } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";

  let { id }: { id: string } = $props();
  const PAGE = 24;

  let list: FileEntry[] | null = $state(null);
  let shown = $state(PAGE);
  let thumbs: Record<string, string> = $state({});
  let error = $state("");

  $effect(() => {
    api
      .files(id, "screenshots")
      .then((l) => (list = l))
      .catch((e) => (error = String(e)));
  });

  // Миниатюры грузятся по одной: первые появляются сразу, диск не перегружен.
  $effect(() => {
    const wanted = (list ?? []).slice(0, shown);
    let cancelled = false;
    (async () => {
      for (const f of wanted) {
        if (cancelled) return;
        if (untrack(() => f.file in thumbs)) continue;
        thumbs[f.file] = await api.thumbnail(id, f.file).catch(() => "");
      }
    })();
    return () => (cancelled = true);
  });

  // Скриншот открывается в окне обрезки: какой кусок кадра станет обложкой, выбирает игрок.
  async function makeCover(f: FileEntry) {
    const src = await attempt("Обложка", () => api.screenshot(id, f.file));
    const png = src && (await crop(src));
    if (!png) return;
    if (await attempt("Обложка", () => api.setCover(id, png).then(() => true))) {
      builds.covers[id] = `data:image/png;base64,${png}`;
      toast("Скриншот стал обложкой сборки");
    }
  }
</script>

<div class="bar">
  <span class="muted small grow">{#if list?.length}{list.length} скриншотов · F2 в игре{/if}</span>
  <button class="btn btn-sm" onclick={() => attempt("Папка", () => api.open(id, { kind: "folder", name: "screenshots" }))}>
    <Icon name="folder" size={16} />Открыть папку
  </button>
</div>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if list && list.length === 0}
  <p class="note">Скриншотов нет. В игре они делаются клавишей F2.</p>
{:else if list}
  <div class="grid">
    {#each list.slice(0, shown) as f (f.file)}
      <figure>
        <button
          class="shot"
          title="Открыть {f.file}"
          onclick={() => attempt("Скриншот", () => api.open(id, { kind: "file", folder: "screenshots", file: f.file }))}
        >
          {#if thumbs[f.file]}<img src={thumbs[f.file]} alt={f.file} />{/if}
        </button>
        <figcaption>
          <span class="muted small grow">{date(f.modified)}</span>
          <button class="btn btn-sm btn-quiet" onclick={() => makeCover(f)}>Сделать обложкой</button>
        </figcaption>
      </figure>
    {/each}
  </div>
  {#if list.length > shown}
    <div class="more"><button class="btn" onclick={() => (shown += PAGE)}>Показать ещё</button></div>
  {/if}
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: var(--s4);
  }
  figure {
    margin: 0;
  }
  .shot {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    padding: 0;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--surface-raised);
    cursor: zoom-in;
  }
  .shot:hover {
    border-color: var(--border-strong);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  figcaption {
    display: flex;
    align-items: center;
    gap: var(--s2);
    margin-top: var(--s1);
  }
  .more {
    display: flex;
    justify-content: center;
    margin-top: var(--s4);
  }
</style>
