// Обложка сборки: игрок выбирает любую картинку, окно обрезает её до квадрата
// по центру и уменьшает до 256×256 (раздел 5.5 ТЗ: хранится уменьшенная копия).
// Уменьшение делает окно — так ядру не нужна тяжёлая библиотека картинок.

import { open } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { inTauri } from "./tauri";

const SIZE = 256;

/// Спросить картинку у игрока и вернуть PNG 256×256 в base64 (без префикса data:).
/// `null` — игрок закрыл диалог.
export async function pickCover(): Promise<string | null> {
  let dataUrl: string;
  if (inTauri) {
    const path = await open({
      multiple: false,
      directory: false,
      title: "Обложка сборки",
      filters: [{ name: "Картинки", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
    });
    if (!path) return null;
    dataUrl = await api.readImage(path);
  } else {
    dataUrl = await pickInBrowser();
    if (!dataUrl) return null;
  }
  return shrink(dataUrl);
}

async function shrink(dataUrl: string): Promise<string> {
  const img = new Image();
  img.src = dataUrl;
  await img.decode();
  const side = Math.min(img.naturalWidth, img.naturalHeight);
  const canvas = document.createElement("canvas");
  canvas.width = SIZE;
  canvas.height = SIZE;
  const ctx = canvas.getContext("2d")!;
  // Маленькие картинки (пиксель-арт) увеличиваем без размытия.
  ctx.imageSmoothingEnabled = side >= SIZE;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(img, (img.naturalWidth - side) / 2, (img.naturalHeight - side) / 2, side, side, 0, 0, SIZE, SIZE);
  return canvas.toDataURL("image/png").split(",")[1];
}

/// Только для проверки вёрстки в браузере (`npm run dev`).
function pickInBrowser(): Promise<string> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) return resolve("");
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.readAsDataURL(file);
    };
    input.click();
  });
}
