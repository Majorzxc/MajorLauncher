// «Пикселизировать под стиль» (design/covers.md): обложка сводится к сетке
// 48×48 и палитре из 24 цветов — как набор обложек по умолчанию.

export const GRID = 48;
export const COLORS = 24;

type Rgb = [number, number, number];

/// Сократить палитру картинки до `colors` цветов (медианное сечение) — на
/// месте. Прозрачные точки не участвуют и остаются прозрачными.
export function quantize(image: ImageData, colors = COLORS) {
  const d = image.data;
  const opaque: number[] = [];
  for (let i = 0; i < d.length; i += 4) {
    if (d[i + 3] >= 128) opaque.push(i);
    else d[i + 3] = 0;
  }
  if (!opaque.length) return;

  // Делим «коробку» цветов с самым большим разбросом пополам по медиане.
  let boxes: number[][] = [opaque];
  while (boxes.length < colors) {
    let best = -1;
    let bestRange = 0;
    let bestChannel = 0;
    boxes.forEach((box, b) => {
      if (box.length < 2) return;
      for (let c = 0; c < 3; c++) {
        let lo = 255;
        let hi = 0;
        for (const i of box) {
          lo = Math.min(lo, d[i + c]);
          hi = Math.max(hi, d[i + c]);
        }
        if (hi - lo > bestRange) [best, bestRange, bestChannel] = [b, hi - lo, c];
      }
    });
    if (best < 0) break;
    const box = boxes[best].sort((a, b) => d[a + bestChannel] - d[b + bestChannel]);
    const mid = box.length >> 1;
    boxes = [...boxes.slice(0, best), box.slice(0, mid), box.slice(mid), ...boxes.slice(best + 1)];
  }

  const palette: Rgb[] = boxes.map((box) => {
    const sum = [0, 0, 0];
    for (const i of box) for (let c = 0; c < 3; c++) sum[c] += d[i + c];
    return sum.map((s) => Math.round(s / box.length)) as Rgb;
  });
  for (const i of opaque) {
    let best = palette[0];
    let bestDist = Infinity;
    for (const p of palette) {
      const dist = (d[i] - p[0]) ** 2 + (d[i + 1] - p[1]) ** 2 + (d[i + 2] - p[2]) ** 2;
      if (dist < bestDist) [best, bestDist] = [p, dist];
    }
    [d[i], d[i + 1], d[i + 2], d[i + 3]] = [best[0], best[1], best[2], 255];
  }
}
