// Описание проекта с Modrinth: Markdown с вкраплениями HTML от автора мода.
// Это чужой текст в окне, у которого есть доступ к командам ядра, поэтому
// после разбора он проходит DOMPurify: без скриптов, обработчиков, фреймов,
// форм и стилей. Картинки — только по https (то же требует и CSP окна),
// ссылки окно открывает во внешнем браузере.

import DOMPurify from "dompurify";
import { marked } from "marked";

DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "IMG") {
    const src = node.getAttribute("src") ?? "";
    if (!src.startsWith("https://")) node.removeAttribute("src");
    node.setAttribute("loading", "lazy");
    node.setAttribute("referrerpolicy", "no-referrer");
  }
  if (node.tagName === "A") {
    const href = node.getAttribute("href") ?? "";
    if (!href.startsWith("https://") && !href.startsWith("http://")) node.removeAttribute("href");
  }
});

export function renderMarkdown(text: string): string {
  const html = marked.parse(text, { async: false, gfm: true }) as string;
  return DOMPurify.sanitize(html, {
    FORBID_TAGS: ["style", "form", "input", "button", "textarea", "select", "iframe", "object", "embed", "video", "audio"],
    FORBID_ATTR: ["style", "class", "id"],
  });
}
