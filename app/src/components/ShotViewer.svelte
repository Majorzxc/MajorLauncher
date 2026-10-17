<script lang="ts">
  // Просмотр скриншота прямо в лаунчере: стрелки (кнопки, ← →, колесо),
  // копировать, сделать обложкой, открыть в программе, показать в папке,
  // убрать в корзину. Esc или клик по фону — закрыть.
  import { api, type FileEntry } from "../lib/api";
  import { date, size } from "../lib/format";
  import { attempt, toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";

  let {
    id,
    list,
    index = $bindable(),
    onclose,
    oncover,
    ondeleted,
  }: {
    id: string;
    list: FileEntry[];
    index: number;
    onclose: () => void;
    oncover: (f: FileEntry) => void;
    ondeleted: (f: FileEntry) => void;
  } = $props();

  const file = $derived(list[index]);
  // Полные кадры: текущий и соседи, чтобы листать без задержки. Больше не держим —
  // каждый кадр 1080p в памяти окна около 8 МБ.
  let images: Record<string, string> = $state({});
  let failed = $state("");
  let asking = $state(false);

  async function load(f: FileEntry | undefined) {
    if (!f || f.file in images) return;
    try {
      images[f.file] = await api.screenshotFull(id, f.file);
    } catch (e) {
      if (f === file) failed = String(e);
    }
  }

  $effect(() => {
    failed = "";
    asking = false;
    const keep = [list[index - 1], file, list[index + 1]].filter(Boolean).map((f) => f.file);
    for (const k of Object.keys(images)) if (!keep.includes(k)) delete images[k];
    load(file).then(() => {
      load(list[index + 1]);
      load(list[index - 1]);
    });
  });

  function step(d: number) {
    const next = index + d;
    if (next >= 0 && next < list.length) index = next;
  }

  async function copy() {
    const src = images[file.file];
    if (!src) return;
    try {
      const blob = fromDataUrl(src);
      // В буфер обмена браузер кладёт только PNG.
      const png = blob.type === "image/png" ? blob : await toPng(blob);
      await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
      toast("Скриншот скопирован — можно вставить в чат");
    } catch (e) {
      toast(`Не удалось скопировать: ${e}`, { error: true });
    }
  }

  // data: URL → Blob без fetch (окну fetch на data: запрещён его CSP).
  function fromDataUrl(src: string): Blob {
    const [head, body] = src.split(",", 2);
    const type = head.slice(5).split(";")[0];
    const bin = atob(body);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return new Blob([bytes], { type });
  }

  async function toPng(blob: Blob): Promise<Blob> {
    const bitmap = await createImageBitmap(blob);
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    canvas.getContext("2d")!.drawImage(bitmap, 0, 0);
    return canvas.convertToBlob({ type: "image/png" });
  }

  async function remove() {
    const f = file;
    if (await attempt("Скриншот", () => api.trashScreenshot(id, f.file).then(() => true))) {
      toast("Скриншот в корзине — вернуть можно оттуда");
      delete images[f.file];
      ondeleted(f);
    }
  }

  function keys(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
    else if (e.key === "ArrowLeft") step(-1);
    else if (e.key === "ArrowRight") step(1);
    else if (e.key === "Delete") asking = true;
    else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c") copy();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  let wheelAt = 0;
  function wheel(e: WheelEvent) {
    const now = Date.now();
    if (now - wheelAt < 250 || Math.abs(e.deltaY) < 10) return;
    wheelAt = now;
    step(e.deltaY > 0 ? 1 : -1);
  }
</script>

<svelte:window onkeydown={keys} />

<div class="viewer" role="dialog" aria-modal="true" aria-label="Скриншот {file?.file}" onwheel={wheel}>
  <button class="backdrop" aria-label="Закрыть" onclick={onclose}></button>

  <div class="top">
    <span class="name">
      <b>{date(file.modified)}</b>
      <span class="muted small">{file.file} · {size(file.size)} · {index + 1} из {list.length}</span>
    </span>
    <span class="grow"></span>
    <button class="btn btn-sm" title="Копировать (Ctrl+C)" onclick={copy}><Icon name="copy" size={16} />Копировать</button>
    <button class="btn btn-sm" onclick={() => oncover(file)}><Icon name="screenshot" size={16} />Сделать обложкой</button>
    <button
      class="btn btn-sm btn-quiet"
      title="Открыть в программе просмотра Windows"
      aria-label="Открыть в программе"
      onclick={() => attempt("Скриншот", () => api.open(id, { kind: "file", folder: "screenshots", file: file.file }))}
      ><Icon name="export" size={16} /></button
    >
    <button
      class="btn btn-sm btn-quiet"
      title="Показать в папке"
      aria-label="Показать в папке"
      onclick={() => attempt("Скриншот", () => api.reveal(id, { kind: "file", folder: "screenshots", file: file.file }))}
      ><Icon name="folder" size={16} /></button
    >
    {#if asking}
      <span class="confirm">
        В корзину?
        <button class="btn btn-sm danger" onclick={remove}>Да</button>
        <button class="btn btn-sm btn-quiet" onclick={() => (asking = false)}>Нет</button>
      </span>
    {:else}
      <button class="btn btn-sm btn-quiet" title="Убрать в корзину (Delete)" aria-label="Убрать в корзину" onclick={() => (asking = true)}
        ><Icon name="trash" size={16} /></button
      >
    {/if}
    <button class="btn btn-sm btn-quiet" title="Закрыть (Esc)" aria-label="Закрыть" onclick={onclose}><Icon name="close" size={18} /></button>
  </div>

  <div class="stage">
    {#if failed}
      <p class="warn"><Icon name="error" size={18} />{failed}</p>
    {:else if images[file.file]}
      <img src={images[file.file]} alt={file.file} />
    {:else}
      <span class="spin" aria-label="Загрузка"></span>
    {/if}
  </div>

  {#if index > 0}
    <button class="nav prev" title="Предыдущий (←)" aria-label="Предыдущий" onclick={() => step(-1)}><Icon name="arrow-left" size={24} /></button>
  {/if}
  {#if index < list.length - 1}
    <button class="nav next" title="Следующий (→)" aria-label="Следующий" onclick={() => step(1)}><Icon name="chevron-right" size={28} /></button>
  {/if}
</div>

<style>
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    grid-template-rows: auto 1fr;
  }
  .backdrop {
    position: absolute;
    inset: 0;
    border: 0;
    background: rgb(0 0 0 / 0.88);
    cursor: zoom-out;
  }
  .top {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--s2);
    padding: var(--s3) var(--s4);
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    border-bottom: 1px solid var(--border);
  }
  .name {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name .small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .confirm {
    display: inline-flex;
    align-items: center;
    gap: var(--s2);
    font-size: var(--fs-14);
  }
  .danger {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }
  .stage {
    position: relative;
    display: grid;
    place-items: center;
    min-height: 0;
    overflow: hidden;
    padding: var(--s4) 72px;
    pointer-events: none;
  }
  img {
    /* Вписать в окно: шапка просмотра около 70 точек, поля по 16. */
    max-width: calc(100vw - 144px);
    max-height: calc(100vh - 110px);
    object-fit: contain;
    border-radius: var(--r);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.5);
    image-rendering: auto;
    pointer-events: auto;
  }
  .nav {
    position: absolute;
    top: 50%;
    display: grid;
    place-items: center;
    width: 48px;
    height: 64px;
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: color-mix(in srgb, var(--surface) 85%, transparent);
    color: var(--text);
    cursor: pointer;
  }
  .nav:hover {
    border-color: var(--border-strong);
  }
  .prev {
    left: var(--s3);
  }
  .next {
    right: var(--s3);
  }
  .spin {
    width: 32px;
    height: 32px;
    border: 3px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: turn 0.8s linear infinite;
  }
  @keyframes turn {
    to {
      transform: rotate(360deg);
    }
  }
</style>
