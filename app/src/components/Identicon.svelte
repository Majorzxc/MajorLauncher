<script lang="ts">
  // Обложка по умолчанию: пиксельный узор 5×5, зеркальный, как аватары GitHub.
  // Свой у каждой сборки (зависит от её id), цвета — из темы.
  let { seed }: { seed: string } = $props();

  function hash(s: string): number {
    let h = 2166136261;
    for (const c of s) h = Math.imul(h ^ c.codePointAt(0)!, 16777619);
    return h >>> 0;
  }

  const cells = $derived.by(() => {
    let h = hash(seed);
    const out: { x: number; y: number; strong: boolean }[] = [];
    for (let y = 0; y < 5; y++) {
      for (let x = 0; x < 3; x++) {
        const on = h & 1;
        const strong = (h & 2) !== 0;
        h = (h >>> 2) | ((h & 3) << 30);
        if (!on) continue;
        out.push({ x, y, strong });
        if (x < 2) out.push({ x: 4 - x, y, strong });
      }
    }
    return out;
  });
</script>

<svg viewBox="-1 -1 7 7" aria-hidden="true">
  {#each cells as c}
    <rect x={c.x} y={c.y} width="1" height="1" class:strong={c.strong} />
  {/each}
</svg>

<style>
  svg {
    width: 56%;
    height: 56%;
    shape-rendering: crispEdges;
  }
  rect {
    fill: var(--border-strong);
  }
  rect.strong {
    fill: var(--text-muted);
  }
</style>
