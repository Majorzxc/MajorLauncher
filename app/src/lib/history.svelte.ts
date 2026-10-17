// «Назад» и «Вперёд»: боковые кнопки мыши и Alt+← / Alt+→. Помним раздел,
// открытую сборку и страницу проекта в каталоге.

import { catalog } from "./catalog.svelte";
import { nav } from "./nav.svelte";

interface Place {
  section: typeof nav.section;
  build: string | null;
  project: string | null;
}

const LIMIT = 50;
const back: Place[] = [];
const forward: Place[] = [];
let current: Place | null = null;
/// Переход сделан самой историей — его не записывать.
let restoring: Place | null = null;

function here(): Place {
  return { section: nav.section, build: nav.build, project: nav.section === "mods" ? catalog.project : null };
}

function same(a: Place, b: Place) {
  return a.section === b.section && a.build === b.build && a.project === b.project;
}

/// Вызывать из $effect в корне окна: читает nav и catalog, запоминает переходы.
export function track() {
  const now = here();
  if (current && same(now, current)) return;
  if (restoring && same(now, restoring)) {
    restoring = null;
  } else if (current) {
    back.push(current);
    if (back.length > LIMIT) back.shift();
    forward.length = 0;
  }
  current = now;
}

function open(p: Place) {
  restoring = p;
  nav.section = p.section;
  nav.build = p.build;
  if (p.section === "mods") catalog.project = p.project;
}

export function goBack() {
  const p = back.pop();
  if (!p || !current) return;
  forward.push(current);
  open(p);
}

export function goForward() {
  const p = forward.pop();
  if (!p || !current) return;
  back.push(current);
  open(p);
}
