// Клавиши игры для редактора: что нажали в окне (KeyboardEvent.code, кнопка
// мыши) → имя, как его пишет игра (`key.keyboard.w`), и обратно — понятная подпись.

const CODE: Record<string, string> = {
  Escape: "escape",
  Minus: "minus",
  Equal: "equal",
  Backspace: "backspace",
  Tab: "tab",
  BracketLeft: "left.bracket",
  BracketRight: "right.bracket",
  Enter: "enter",
  ControlLeft: "left.control",
  ControlRight: "right.control",
  Semicolon: "semicolon",
  Quote: "apostrophe",
  Backquote: "grave.accent",
  ShiftLeft: "left.shift",
  ShiftRight: "right.shift",
  Backslash: "backslash",
  Comma: "comma",
  Period: "period",
  Slash: "slash",
  AltLeft: "left.alt",
  AltRight: "right.alt",
  Space: "space",
  CapsLock: "caps.lock",
  NumLock: "num.lock",
  ScrollLock: "scroll.lock",
  NumpadMultiply: "keypad.multiply",
  NumpadSubtract: "keypad.subtract",
  NumpadAdd: "keypad.add",
  NumpadDecimal: "keypad.decimal",
  NumpadEqual: "keypad.equal",
  NumpadEnter: "keypad.enter",
  NumpadDivide: "keypad.divide",
  PrintScreen: "print.screen",
  Pause: "pause",
  Home: "home",
  End: "end",
  PageUp: "page.up",
  PageDown: "page.down",
  Insert: "insert",
  Delete: "delete",
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  MetaLeft: "left.win",
  MetaRight: "right.win",
};

/// Нажатая клавиша → имя для игры; `null` — такой в старых версиях нет.
export function fromKeyboard(code: string): string | null {
  let m: RegExpMatchArray | null;
  if ((m = code.match(/^Key([A-Z])$/))) return `key.keyboard.${m[1].toLowerCase()}`;
  if ((m = code.match(/^Digit(\d)$/))) return `key.keyboard.${m[1]}`;
  if ((m = code.match(/^Numpad(\d)$/))) return `key.keyboard.keypad.${m[1]}`;
  if ((m = code.match(/^F(\d{1,2})$/)) && Number(m[1]) <= 15) return `key.keyboard.f${m[1]}`;
  return CODE[code] ? `key.keyboard.${CODE[code]}` : null;
}

/// Кнопка мыши (MouseEvent.button) → имя для игры.
export function fromMouse(button: number): string {
  return ["key.mouse.left", "key.mouse.middle", "key.mouse.right"][button] ?? `key.mouse.${button + 1}`;
}

const LABEL: Record<string, string> = {
  escape: "Esc",
  minus: "-",
  equal: "=",
  backspace: "Backspace",
  tab: "Tab",
  "left.bracket": "[",
  "right.bracket": "]",
  enter: "Enter",
  "left.control": "Ctrl слева",
  "right.control": "Ctrl справа",
  semicolon: ";",
  apostrophe: "'",
  "grave.accent": "`",
  "left.shift": "Shift слева",
  "right.shift": "Shift справа",
  backslash: "\\",
  comma: ",",
  period: ".",
  slash: "/",
  "left.alt": "Alt слева",
  "right.alt": "Alt справа",
  space: "Пробел",
  "caps.lock": "Caps Lock",
  "num.lock": "Num Lock",
  "scroll.lock": "Scroll Lock",
  "print.screen": "Print Screen",
  pause: "Pause",
  home: "Home",
  end: "End",
  "page.up": "Page Up",
  "page.down": "Page Down",
  insert: "Insert",
  delete: "Delete",
  up: "↑",
  down: "↓",
  left: "←",
  right: "→",
  "left.win": "Win слева",
  "right.win": "Win справа",
  unknown: "не назначена",
};

/// Подпись клавиши: «W», «Пробел», «ЛКМ», «Цифр. 5».
export function keyLabel(name: string): string {
  const mouse = name.match(/^key\.mouse\.(.+)$/);
  if (mouse) return { left: "ЛКМ", right: "ПКМ", middle: "СКМ" }[mouse[1]] ?? `Мышь ${mouse[1]}`;
  const key = name.replace(/^key\.keyboard\./, "");
  const pad = key.match(/^keypad\.(.+)$/);
  if (pad) return `Цифр. ${{ multiply: "*", subtract: "-", add: "+", decimal: ".", equal: "=", enter: "Enter", divide: "/" }[pad[1]] ?? pad[1]}`;
  return LABEL[key] ?? key.toUpperCase();
}
