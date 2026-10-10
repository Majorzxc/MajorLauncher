<script lang="ts">
  // Обрезка обложки (design/covers.md): картинка двигается под квадратной
  // рамкой мышью или стрелками, масштаб — колесом или ползунком.
  // Сохраняется копия 256×256, а с «Пикселизировать» — сетка 48×48
  // из 24 цветов, увеличенная до 240×240 без сглаживания.
  import { COLORS, GRID, quantize } from "../lib/pixelate";
  import Modal from "./Modal.svelte";

  let { src, ondone }: { src: string; ondone: (png: string | null) => void } = $props();

  /// Сторона рамки на экране.
  const VIEW = 320;
  const SIZE = 256;
  const PIXEL_SIZE = GRID * 5;

  let image: HTMLImageElement | null = $state(null);
  let error = $state("");
  let w = $state(1);
  let h = $state(1);
  let scale = $state(1);
  let x = $state(0);
  let y = $state(0);
  let pixel = $state(false);
  let preview: HTMLCanvasElement | null = $state(null);

  // Меньше рамки картинку не уменьшить; увеличить — пока в рамке не меньше 16 точек.
  const min = $derived(VIEW / Math.min(w, h));
  const max = $derived(Math.max(min, Math.min(min * 10, VIEW / 16)));
  const zoom = $derived(max > min ? Math.log(scale / min) / Math.log(max / min) : 0);

  $effect(() => {
    const img = new Image();
    img.src = src;
    img
      .decode()
      .then(() => {
        w = img.naturalWidth;
        h = img.naturalHeight;
        scale = VIEW / Math.min(w, h);
        x = (VIEW - w * scale) / 2;
        y = (VIEW - h * scale) / 2;
        image = img;
      })
      .catch(() => (error = "Не удалось открыть картинку."));
  });

  function clamp() {
    x = Math.min(0, Math.max(VIEW - w * scale, x));
    y = Math.min(0, Math.max(VIEW - h * scale, y));
  }

  /// Масштаб вокруг точки (cx, cy) рамки: она остаётся на месте.
  function zoomAt(next: number, cx = VIEW / 2, cy = VIEW / 2) {
    next = Math.min(max, Math.max(min, next));
    x = cx - ((cx - x) / scale) * next;
    y = cy - ((cy - y) / scale) * next;
    scale = next;
    clamp();
  }

  let drag: { px: number; py: number; x: number; y: number } | null = null;
  function down(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { px: e.clientX, py: e.clientY, x, y };
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    x = drag.x + e.clientX - drag.px;
    y = drag.y + e.clientY - drag.py;
    clamp();
  }
  function wheel(e: WheelEvent) {
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    zoomAt(scale * Math.exp(-e.deltaY * 0.0015), e.clientX - r.left, e.clientY - r.top);
  }
  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 40 : 10;
    const moves: Record<string, [number, number]> = {
      ArrowLeft: [step, 0],
      ArrowRight: [-step, 0],
      ArrowUp: [0, step],
      ArrowDown: [0, -step],
    };
    if (moves[e.key]) {
      x += moves[e.key][0];
      y += moves[e.key][1];
      clamp();
    } else if (e.key === "+" || e.key === "=") zoomAt(scale * 1.15);
    else if (e.key === "-") zoomAt(scale / 1.15);
    else return;
    e.preventDefault();
  }

  /// Нарисовать результат на холсте со стороной `side`.
  function render(canvas: HTMLCanvasElement, side: number) {
    const img = image!;
    const sx = -x / scale;
    const sy = -y / scale;
    const s = VIEW / scale;
    canvas.width = side;
    canvas.height = side;
    const ctx = canvas.getContext("2d")!;
    if (!pixel) {
      // Маленькие картинки (пиксель-арт) увеличиваем без размытия.
      ctx.imageSmoothingEnabled = s >= side;
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(img, sx, sy, s, s, 0, 0, side, side);
      return;
    }
    const grid = document.createElement("canvas");
    grid.width = GRID;
    grid.height = GRID;
    const g = grid.getContext("2d", { willReadFrequently: true })!;
    g.imageSmoothingQuality = "high";
    g.drawImage(img, sx, sy, s, s, 0, 0, GRID, GRID);
    const data = g.getImageData(0, 0, GRID, GRID);
    quantize(data, COLORS);
    g.putImageData(data, 0, 0);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(grid, 0, 0, side, side);
  }

  // Предпросмотр — не чаще кадра экрана.
  let frame = 0;
  $effect(() => {
    void [x, y, scale, pixel];
    if (!image || !preview) return;
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => preview && render(preview, 96));
  });

  function save() {
    const canvas = document.createElement("canvas");
    render(canvas, pixel ? PIXEL_SIZE : SIZE);
    ondone(canvas.toDataURL("image/png").split(",")[1]);
  }
</script>

<Modal title="Обложка" width={600} onclose={() => ondone(null)}>
  <div class="crop">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="frame"
      style:width="{VIEW}px"
      style:height="{VIEW}px"
      role="application"
      aria-label="Рамка обложки: двигайте картинку мышью или стрелками, масштаб — колесом или клавишами плюс и минус"
      tabindex="0"
      onpointerdown={down}
      onpointermove={move}
      onpointerup={() => (drag = null)}
      onpointercancel={() => (drag = null)}
      onwheel={wheel}
      onkeydown={key}
    >
      {#if image}
        <img
          {src}
          alt=""
          draggable="false"
          style:width="{w * scale}px"
          style:height="{h * scale}px"
          style:transform="translate({x}px, {y}px)"
          class:pixel={scale >= 2}
        />
      {:else if error}
        <p class="warn">{error}</p>
      {/if}
    </div>
    <div class="side">
      <span class="small muted">Так будет на плитке</span>
      <canvas bind:this={preview} width="96" height="96" class="preview" aria-label="Предпросмотр обложки"></canvas>
      <label class="zoom">
        <span class="small">Масштаб</span>
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          value={zoom}
          disabled={max <= min}
          oninput={(e) => zoomAt(min * (max / min) ** Number(e.currentTarget.value))}
          aria-label="Масштаб"
        />
      </label>
      <label class="opt">
        <input type="checkbox" bind:checked={pixel} />
        <span>
          Пикселизировать под стиль
          <span class="muted small hint">Сетка {GRID}×{GRID} и {COLORS} цвета — как у пиксельных обложек.</span>
        </span>
      </label>
    </div>
  </div>
  <p class="muted small">Двигайте картинку мышью, масштаб — колесом. Сохранится квадрат в рамке, оригинал не меняется.</p>
  {#snippet footer()}
    <button class="btn btn-quiet" onclick={() => ondone(null)}>Отмена</button>
    <button class="btn btn-primary" disabled={!image} onclick={save}>Сохранить</button>
  {/snippet}
</Modal>

<style>
  .crop {
    display: flex;
    gap: var(--s5);
    align-items: flex-start;
  }
  .frame {
    position: relative;
    flex: none;
    overflow: hidden;
    border-radius: var(--r);
    background: var(--surface-raised);
    outline: 2px solid var(--border-strong);
    cursor: grab;
    touch-action: none;
  }
  .frame:active {
    cursor: grabbing;
  }
  .frame:focus-visible {
    outline-color: var(--accent);
  }
  .frame img {
    position: absolute;
    top: 0;
    left: 0;
    max-width: none;
    transform-origin: 0 0;
    pointer-events: none;
    user-select: none;
  }
  .frame img.pixel {
    image-rendering: pixelated;
  }
  .frame .warn {
    padding: var(--s4);
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
    min-width: 0;
  }
  .preview {
    width: 96px;
    height: 96px;
    border-radius: var(--r);
    background: var(--surface-raised);
    image-rendering: pixelated;
  }
  .zoom {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
  }
  .zoom input {
    accent-color: var(--accent);
  }
  .opt {
    display: flex;
    gap: var(--s2);
    align-items: flex-start;
    cursor: pointer;
  }
  .opt input {
    margin-top: 5px;
  }
  .hint {
    display: block;
  }
  p {
    margin: var(--s4) 0 0;
  }
</style>
