// Работа с цветом: игрок задаёт только фон и акцент, остальные оттенки
// (панели, рамки, вторичный текст) вычисляются, чтобы интерфейс нельзя
// было случайно сделать нечитаемым (раздел 6.0 ТЗ).

type Rgb = [number, number, number];

export function parseHex(hex: string): Rgb {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function toHex([r, g, b]: Rgb): string {
  return "#" + [r, g, b].map((v) => Math.round(v).toString(16).padStart(2, "0")).join("");
}

/// Смешать `a` и `b`: t = 0 — чистый `a`, t = 1 — чистый `b`.
export function mix(a: string, b: string, t: number): string {
  const x = parseHex(a);
  const y = parseHex(b);
  return toHex([0, 1, 2].map((i) => x[i] + (y[i] - x[i]) * t) as Rgb);
}

/// Относительная яркость по WCAG.
export function luminance(hex: string): number {
  const [r, g, b] = parseHex(hex).map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/// Контраст по WCAG: 1 — никакого, 21 — чёрное на белом.
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const BLACK = "#000000";
const WHITE = "#ffffff";
const INK_DARK = "#121214";
const INK_LIGHT = "#ededef";

/// Чем писать поверх цвета: тёмным или светлым — что контрастнее.
export function inkOn(bg: string): string {
  return contrast(INK_DARK, bg) >= contrast(INK_LIGHT, bg) ? INK_DARK : INK_LIGHT;
}

export interface Palette {
  bg: string;
  surface: string;
  surfaceRaised: string;
  border: string;
  borderStrong: string;
  text: string;
  textMuted: string;
  accent: string;
  accentHover: string;
  onAccent: string;
  dark: boolean;
}

/// Полная палитра из двух цветов.
export function palette(bg: string, accent: string): Palette {
  const text = inkOn(bg);
  const dark = text === INK_LIGHT;
  // Тёмная тема: панели светлее фона. Светлая: панели белее фона, рамки темнее.
  const up = (t: number) => mix(bg, WHITE, t);
  const down = (t: number) => mix(bg, BLACK, t);
  return {
    bg,
    surface: dark ? up(0.05) : up(0.8),
    surfaceRaised: dark ? up(0.1) : down(0.04),
    border: dark ? up(0.15) : down(0.11),
    borderStrong: dark ? up(0.36) : down(0.42),
    text,
    textMuted: mix(text, bg, 0.36),
    accent,
    accentHover: mix(accent, dark ? WHITE : BLACK, 0.12),
    onAccent: inkOn(accent),
    dark,
  };
}

/// Предупреждения о плохом контрасте — показываются рядом с выбором цвета.
export function contrastWarnings(p: Palette): string[] {
  const warnings: string[] = [];
  if (contrast(p.text, p.bg) < 4.5) {
    warnings.push("Фон слишком средний по яркости: текст на нём читается хуже. Возьмите цвет темнее или светлее.");
  }
  // Порог 2:1, а не 3:1 из WCAG: насыщенный цвет глаз различает и при меньшем
  // контрасте — предупреждаем, только когда кнопку действительно почти не видно.
  if (contrast(p.accent, p.surface) < 2) {
    warnings.push("Акцентный цвет почти сливается с панелями: кнопки «Играть» будет плохо видно.");
  }
  return warnings;
}
