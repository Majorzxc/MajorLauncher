import { mount } from "svelte";
import App from "./App.svelte";
import "./styles/base.css";

// Окно лаунчера — не браузер: меню WebView2 по правому клику («Назад»,
// «Печать», «Сохранить как») и его горячие клавиши здесь только мешают.
// В полях ввода меню остаётся — там «Вырезать / Копировать / Вставить».
window.addEventListener("contextmenu", (e) => {
  if (!(e.target as Element).closest?.("input, textarea, [contenteditable]")) e.preventDefault();
});

const BROWSER_KEYS = ["r", "p", "s", "f", "g", "u", "j", "h", "o", "n", "t", "w"];
window.addEventListener("keydown", (e) => {
  const key = e.key.toLowerCase();
  const ctrl = e.ctrlKey || e.metaKey;
  const blocked =
    key === "f5" ||
    key === "f3" ||
    key === "f7" ||
    key === "browserback" ||
    key === "browserforward" ||
    (ctrl && BROWSER_KEYS.includes(key)) ||
    (e.altKey && (key === "arrowleft" || key === "arrowright"));
  if (blocked) e.preventDefault();
});

export default mount(App, { target: document.getElementById("app")! });
