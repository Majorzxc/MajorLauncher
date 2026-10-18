// Перевод описаний с Modrinth на русский (ядро ходит в Google и кэширует).
// Переводятся только текстовые фрагменты уже очищенного HTML (после
// DOMPurify), и вставляются обратно как текст — разметка, картинки и ссылки
// остаются на месте, а чужой HTML через перевод попасть в окно не может.

import { api } from "./api";

/// Внутри этих тегов — код и команды: не переводим.
const SKIP = new Set(["CODE", "PRE", "KBD", "SAMP", "SCRIPT", "STYLE"]);

export async function translateHtml(html: string): Promise<string> {
  const tpl = document.createElement("template");
  tpl.innerHTML = html;
  const nodes: Text[] = [];
  const walker = document.createTreeWalker(tpl.content, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    const text = n as Text;
    if (!/[a-z]/i.test(text.data)) continue;
    let p = text.parentElement;
    let skip = false;
    while (p) {
      if (SKIP.has(p.tagName)) {
        skip = true;
        break;
      }
      p = p.parentElement;
    }
    if (!skip) nodes.push(text);
  }
  if (!nodes.length) return html;
  const translated = await api.catalogTranslate(nodes.map((n) => n.data));
  nodes.forEach((n, i) => {
    // Пробелы по краям — как были: «текст <b>жирный</b> текст».
    const lead = n.data.match(/^\s*/)![0];
    const trail = n.data.match(/\s*$/)![0];
    n.data = lead + translated[i].trim() + trail;
  });
  return tpl.innerHTML;
}
