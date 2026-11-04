// Ссылки открываются во внешнем браузере, а не внутри окна лаунчера: у окна
// есть доступ к командам ядра, чужим страницам в нём не место.

import { openUrl } from "@tauri-apps/plugin-opener";
import { inTauri } from "./tauri";

export async function openExternal(url: string | null | undefined): Promise<void> {
  if (!url) return;
  if (inTauri) await openUrl(url);
  else window.open(url);
}

/// Клик внутри отрисованного Markdown (описания модов, ответы ИИ, заметки
/// выпуска): ссылка — в браузер, остальное — как обычно.
export function interceptLinks(e: MouseEvent) {
  const a = (e.target as HTMLElement).closest("a");
  if (!a) return;
  e.preventDefault();
  const href = a.getAttribute("href");
  if (href?.startsWith("http")) openExternal(href).catch(() => {});
}
