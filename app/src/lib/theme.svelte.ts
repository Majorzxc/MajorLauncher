// Оформление окна: чёрная и белая темы плюс свои цвета игрока. Хранит ядро
// (config.json), здесь — применение к окну и живой предпросмотр.

import { call } from "./tauri";
import { contrastWarnings, palette, type Palette } from "./color";

export type ThemeMode = "dark" | "light" | "system";

export interface Appearance {
  mode: ThemeMode;
  accent: string | null;
  background: string | null;
}

/// Готовые чёрная и белая темы: акцент монохромный, как и всё остальное.
/// Фон — те же значения, что DARK_BACKGROUND / LIGHT_BACKGROUND в core/src/config.rs.
const BASE = {
  dark: { bg: "#0e0e10", accent: "#ededef" },
  light: { bg: "#f4f4f5", accent: "#121214" },
};

const systemDark = window.matchMedia("(prefers-color-scheme: dark)");

export const theme = $state({
  appearance: { mode: "system", accent: null, background: null } as Appearance,
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
  root.setProperty("color-scheme", p.dark ? "dark" : "light");

  theme.appearance = a;
  theme.palette = p;
  theme.warnings = contrastWarnings(p);
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
