// Оформление окна: чёрная и белая темы плюс свои цвета игрока. Хранит ядро
// (config.json), здесь — применение к окну и живой предпросмотр.

import { call } from "./tauri";
import { contrastWarnings, palette, type Palette } from "./color";

export type ThemeMode = "dark" | "light" | "system";

export interface Appearance {
  mode: ThemeMode;
  accent: string | null;
  background: string | null;
  /// Картинка фона (файл хранит ядро).
  image: boolean;
  /// Насколько приглушить картинку цветом фона, %.
  image_dim: number;
}

/// Готовые чёрная и белая темы: акцент монохромный, как и всё остальное.
/// Фон — те же значения, что DARK_BACKGROUND / LIGHT_BACKGROUND в core/src/config.rs.
const BASE = {
  dark: { bg: "#0e0e10", accent: "#ededef" },
  light: { bg: "#f4f4f5", accent: "#121214" },
};

const systemDark = window.matchMedia("(prefers-color-scheme: dark)");

export const theme = $state({
  appearance: { mode: "system", accent: null, background: null, image: false, image_dim: 60 } as Appearance,
  /// Картинка фона, data URL.
  image: null as string | null,
  palette: palette(BASE.dark.bg, BASE.dark.accent) as Palette,
  warnings: [] as string[],
});

function isDark(mode: ThemeMode): boolean {
  return mode === "dark" || (mode === "system" && systemDark.matches);
}

/// Применить оформление к окну (без сохранения).
export function apply(a: Appearance) {
  const base = isDark(a.mode) ? BASE.dark : BASE.light;
  const bg = a.background ?? base.bg;
  // Свой фон без своего акцента: акцент — цвет текста, монохромно.
  const p = palette(bg, a.accent ?? (a.background ? palette(bg, base.accent).text : base.accent));

  const root = document.documentElement.style;
  root.setProperty("--bg", p.bg);
  root.setProperty("--surface", p.surface);
  root.setProperty("--surface-raised", p.surfaceRaised);
  root.setProperty("--border", p.border);
  root.setProperty("--border-strong", p.borderStrong);
  root.setProperty("--text", p.text);
  root.setProperty("--text-muted", p.textMuted);
  root.setProperty("--accent", p.accent);
  root.setProperty("--accent-hover", p.accentHover);
  root.setProperty("--on-accent", p.onAccent);
  // Красный и жёлтый — только для предупреждений и опасных действий,
  // подобраны отдельно для тёмного и светлого фона ради контраста.
  root.setProperty("--danger", p.dark ? "#f0716a" : "#c81e1e");
  root.setProperty("--warning", p.dark ? "#d9ad3c" : "#8a5a14");
  root.setProperty("color-scheme", p.dark ? "dark" : "light");
  // Картинка приглушается цветом фона: в тёмной теме темнеет, в светлой светлеет.
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(p.bg.slice(i, i + 2), 16));
  root.setProperty("--bg-dim", `rgb(${r} ${g} ${b} / ${a.image_dim / 100})`);
  document.body.classList.toggle("with-image", a.image && !!theme.image);

  theme.appearance = a;
  theme.palette = p;
  theme.warnings = contrastWarnings(p);
}

/// Подгрузить картинку фона у ядра (при старте и после смены картинки).
export async function loadImage() {
  theme.image = theme.appearance.image ? await call<string | null>("background_image") : null;
  // Картинка — в переменной: ползунок приглушения не пересобирает её заново.
  document.documentElement.style.setProperty("--bg-image", theme.image ? `url("${theme.image}")` : "none");
  apply(theme.appearance);
}

/// Применить и сохранить через ядро. Ядро проверяет цвета само.
export async function save(a: Appearance) {
  apply(a);
  await call("set_appearance", { appearance: a });
}

// «Как в Windows» следит за сменой темы системы на лету.
systemDark.addEventListener("change", () => {
  if (theme.appearance.mode === "system") apply(theme.appearance);
});
