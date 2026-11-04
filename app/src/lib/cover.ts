// Обложка сборки: игрок выбирает любую картинку и обрезает её в окне обрезки
// (раздел 5.5 ТЗ, design/covers.md). Хранится уменьшенная копия 256×256.
// Большие фото ядро заранее уменьшает: окну не нужно держать их целиком.

import { open } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { crop } from "./crop.svelte";
import { inTauri } from "./tauri";

/// Спросить картинку у игрока, дать обрезать и вернуть PNG в base64 (без
/// префикса data:). `null` — игрок закрыл диалог или окно обрезки.
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
  return crop(dataUrl);
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

/// Готовые обложки из `assets/covers` (scripts/covers.py): 48×48, по порядку имён.
const FILES = import.meta.glob("../assets/covers/*.png", { eager: true, query: "?url", import: "default" }) as Record<
  string,
  string
>;
const TITLES: Record<string, string> = {
  forest: "Лес",
  mountains: "Горы",
  cave: "Пещера",
  inferno: "Огненный мир",
  "void-islands": "Острова в пустоте",
  ocean: "Остров",
  winter: "Зима",
  desert: "Пустыня",
  village: "Деревня",
  tech: "Техника",
  magic: "Магия",
  castle: "Замок",
};
export const DEFAULT_COVERS = Object.entries(FILES)
  .sort(([a], [b]) => a.localeCompare(b))
  .map(([path, url]) => {
    const id = path.split("/").pop()!.replace(/\.png$/, "");
    return { id, url, title: TITLES[id.replace(/^\d+-/, "")] ?? id };
  });

/// Готовая обложка — PNG 240×240 в base64: каждый пиксель увеличен ровно в 5 раз.
export async function defaultCover(url: string): Promise<string> {
  const img = new Image();
  img.src = url;
  await img.decode();
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 240;
  const g = canvas.getContext("2d")!;
  g.imageSmoothingEnabled = false;
  g.drawImage(img, 0, 0, 240, 240);
  return canvas.toDataURL("image/png").split(",")[1];
}
