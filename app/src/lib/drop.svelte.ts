// Перетаскивание файлов в окно. Окно Tauri забирает перетаскивание себе
// (поэтому HTML5-перетаскивание внутри страницы и не работает) и отдаёт пути
// файлов; что с ними делать, решает раздел, который сейчас на экране.

import { getCurrentWebview } from "@tauri-apps/api/webview";
import { onDestroy } from "svelte";
import { inTauri } from "./tauri";
import { toast } from "./toast.svelte";

export interface DropTarget {
  /// Что будет, если отпустить: «Моды и паки добавятся в «Выживание»».
  hint: string;
  ondrop: (paths: string[]) => void;
}

export const dropping = $state({ over: false, hint: "" });

// Последний зарегистрированный раздел главнее: страница сборки поверх «Сборок».
const targets: (() => DropTarget)[] = [];

/// Принимать файлы, пока компонент на экране. `target` вызывается в момент
/// перетаскивания — подпись и действие могут зависеть от открытой вкладки.
export function acceptDrop(target: () => DropTarget) {
  targets.push(target);
  onDestroy(() => {
    const i = targets.indexOf(target);
    if (i >= 0) targets.splice(i, 1);
  });
}

const NOWHERE = "Сюда файлы не принимаются: перетащите их на страницу сборки, в «Сборки», «Моды → Библиотека» или «Скины»";

export async function connectDrop() {
  if (!inTauri) return;
  await getCurrentWebview().onDragDropEvent((e) => {
    const p = e.payload;
    const target = targets.at(-1)?.();
    if (p.type === "enter" || p.type === "over") {
      dropping.over = true;
      dropping.hint = target?.hint ?? NOWHERE;
    } else if (p.type === "leave") {
      dropping.over = false;
    } else if (p.type === "drop") {
      dropping.over = false;
      if (!p.paths.length) return;
      if (target) target.ondrop(p.paths);
      else toast(NOWHERE, { error: true });
    }
  });
}

/// Расширение файла маленькими буквами: `mods.jar` → `jar`.
export function ext(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}
