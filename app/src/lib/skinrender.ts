// Фигура игрока целиком по картинке скина (для сеток библиотеки и каталога):
// спереди, с внешним слоем (куртка, шлем) и тонкими руками. Рисуем сами по
// развёртке скина — без сторонних сервисов картинок. 3D — в SkinViewer3D.

export interface Decoded {
  /// Пиксели скина 64×64 или 64×32.
  image: ImageData;
  slim: boolean;
}

/// Байты PNG → пиксели. Через Blob: окну не нужен адрес картинки (CSP).
export async function decode(bytes: ArrayBuffer): Promise<Decoded> {
  const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
  const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
  const ctx = canvas.getContext("2d")!;
  ctx.drawImage(bitmap, 0, 0);
  bitmap.close();
  const image = ctx.getImageData(0, 0, canvas.width, canvas.height);
  return { image, slim: detectSlim(image) };
}

/// Тонкие руки: столбцы 54–55 правой руки (строки 20–31) пустые — как в ядре.
export function detectSlim(img: ImageData): boolean {
  if (img.height < 64) return false;
  for (let y = 20; y < 32; y++)
    for (let x = 54; x < 56; x++) if (img.data[(y * img.width + x) * 4 + 3] !== 0) return false;
  return true;
}

type Part = [sx: number, sy: number, w: number, h: number, dx: number, dy: number, mirror?: boolean];

/// Части фигуры спереди: откуда на развёртке → куда на фигуре 16×32.
/// Правая рука и нога игрока — слева на картинке (он смотрит на нас).
function parts(slim: boolean, modern: boolean): { base: Part[]; overlay: Part[] } {
  const a = slim ? 3 : 4;
  const base: Part[] = [
    [8, 8, 8, 8, 4, 0], // голова
    [20, 20, 8, 12, 4, 8], // тело
    [44, 20, a, 12, 4 - a, 8], // правая рука
    [4, 20, 4, 12, 4, 20], // правая нога
    // Левые: в старом формате 64×32 их нет — зеркало правых.
    modern ? [36, 52, a, 12, 12, 8] : [44, 20, a, 12, 12, 8, true],
    modern ? [20, 52, 4, 12, 8, 20] : [4, 20, 4, 12, 8, 20, true],
  ];
  const overlay: Part[] = [[40, 8, 8, 8, 4, 0]];
  if (modern)
    overlay.push(
      [20, 36, 8, 12, 4, 8],
      [44, 36, a, 12, 4 - a, 8],
      [4, 36, 4, 12, 4, 20],
      [52, 52, a, 12, 12, 8],
      [4, 52, 4, 12, 8, 20],
    );
  return { base, overlay };
}

/// Фигура 16×32 (тонкие руки — по центру своего места).
export function front(img: ImageData, slim: boolean): ImageData {
  const out = new ImageData(16, 32);
  const { base, overlay } = parts(slim, img.height >= 64);
  const put = ([sx, sy, w, h, dx, dy, mirror]: Part) => {
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        const fx = mirror ? w - 1 - x : x;
        const si = ((sy + y) * img.width + sx + fx) * 4;
        const alpha = img.data[si + 3];
        if (alpha === 0) continue;
        const di = ((dy + y) * 16 + dx + x) * 4;
        out.data[di] = img.data[si];
        out.data[di + 1] = img.data[si + 1];
        out.data[di + 2] = img.data[si + 2];
        out.data[di + 3] = 255;
      }
  };
  // Внешний слой поверх основного; прозрачное в нём основное не закрывает.
  base.forEach(put);
  overlay.forEach(put);
  return out;
}

/// Фигура картинкой (data:-адрес) — для <img> с image-rendering: pixelated.
export function figureUrl(img: ImageData, slim: boolean): string {
  const canvas = document.createElement("canvas");
  canvas.width = 16;
  canvas.height = 32;
  canvas.getContext("2d")!.putImageData(front(img, slim), 0, 0);
  return canvas.toDataURL("image/png");
}

/// Готовые фигуры — по ключу (id скина или адрес текстуры): сетка каталога
/// перерисовывается часто, а картинки не меняются.
const cache = new Map<string, Promise<{ url: string; slim: boolean; image: ImageData }>>();

export function figure(
  key: string,
  load: () => Promise<ArrayBuffer>,
  slim: boolean | null = null,
): Promise<{ url: string; slim: boolean; image: ImageData }> {
  const k = `${key}|${slim}`;
  let p = cache.get(k);
  if (!p) {
    p = load()
      .then(decode)
      .then((d) => {
        const s = slim ?? d.slim;
        return { url: figureUrl(d.image, s), slim: s, image: d.image };
      });
    p.catch(() => cache.delete(k));
    cache.set(k, p);
    if (cache.size > 600) cache.delete(cache.keys().next().value!);
  }
  return p;
}

/// Забыть фигуру (скин изменили в редакторе).
export function forget(key: string) {
  for (const k of [...cache.keys()]) if (k.startsWith(`${key}|`)) cache.delete(k);
}
