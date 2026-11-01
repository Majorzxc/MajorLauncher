<script lang="ts" module>
  export type { Pose } from "../lib/skin3d";
</script>

<script lang="ts">
  // 3D-просмотр скина (раздел 5.9 ТЗ): поворот мышью, колесо — приблизить,
  // анимации стояния, ходьбы, бега и полёта. Рисует lib/skin3d — без WebGL:
  // окно лаунчера работает без видеокарты ради памяти.
  import { onDestroy, onMount } from "svelte";
  import { Skin3D, type Pose } from "../lib/skin3d";

  let {
    skin,
    slim,
    cape = null,
    pose = "walk",
    width = 300,
    height = 420,
  }: {
    /// Пиксели скина (из lib/skinrender).
    skin: ImageData | null;
    slim: boolean;
    cape?: ImageData | null;
    pose?: Pose;
    width?: number;
    height?: number;
  } = $props();

  let canvas: HTMLCanvasElement;
  let view = $state<Skin3D | null>(null);

  onMount(() => (view = new Skin3D(canvas)));
  onDestroy(() => view?.dispose());

  $effect(() => view?.setSkin(skin, slim));
  $effect(() => view?.setCape(cape));
  $effect(() => view?.setPose(pose));
</script>

<canvas
  bind:this={canvas}
  style:width="{width}px"
  style:height="{height}px"
  aria-label="Скин в 3D: тяните мышью, чтобы повернуть, колесо — приблизить"
></canvas>

<style>
  canvas {
    display: block;
    cursor: grab;
    touch-action: none;
  }
  canvas:active {
    cursor: grabbing;
  }
</style>
