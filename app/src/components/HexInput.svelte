<script lang="ts" module>
  /// Код цвета в любом привычном виде → `#rrggbb`; `null`, если это не цвет.
  /// Принимает `#7c3aed`, `7C3AED`, короткий `#73e`, с пробелами по краям.
  export function normalizeHex(text: string): string | null {
    const raw = text.trim().replace(/^#/, "").toLowerCase();
    if (/^[0-9a-f]{6}$/.test(raw)) return "#" + raw;
    if (/^[0-9a-f]{3}$/.test(raw)) return "#" + [...raw].map((c) => c + c).join("");
    return null;
  }
</script>

<script lang="ts">
  let {
    value,
    label,
    onpreview,
    oncommit,
  }: {
    value: string;
    label: string;
    /// Цвет набран верно — показать его сразу, без сохранения.
    onpreview: (hex: string) => void;
    /// Ввод закончен (Enter или уход из поля) — сохранить.
    oncommit: (hex: string) => void;
  } = $props();

  let text = $state("");
  let editing = $state(false);
  let invalid = $state(false);

  // Пока игрок не печатает, поле показывает текущий цвет (в том числе выбранный в палитре).
  $effect(() => {
    if (!editing) text = value;
  });

  function input(e: Event & { currentTarget: HTMLInputElement }) {
    editing = true;
    text = e.currentTarget.value;
    const hex = normalizeHex(text);
    invalid = hex === null && text.trim() !== "";
    if (hex) onpreview(hex);
  }

  function commit() {
    if (!editing) return;
    editing = false;
    const hex = normalizeHex(text);
    if (hex) {
      invalid = false;
      oncommit(hex);
    } else {
      // Неверный код не сохраняем — возвращаем текущий цвет.
      invalid = false;
      text = value;
      onpreview(value);
    }
  }
</script>

<span class="wrap">
  <input
    class:invalid
    type="text"
    spellcheck="false"
    maxlength="9"
    aria-label={label}
    aria-invalid={invalid}
    value={text}
    oninput={input}
    onblur={commit}
    onkeydown={(e) => {
      if (e.key === "Enter") e.currentTarget.blur();
      if (e.key === "Escape") {
        editing = false;
        invalid = false;
        text = value;
        onpreview(value);
        e.currentTarget.blur();
      }
    }}
  />
  {#if invalid}<span class="hint">Нужен код вида #7c3aed</span>{/if}
</span>

<style>
  .wrap {
    position: relative;
  }
  input {
    width: 96px;
    height: var(--h-sm);
    padding: 0 var(--s2);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--bg);
    font-family: Consolas, monospace;
    font-size: var(--fs-14);
    user-select: text;
  }
  input:hover {
    border-color: var(--border-strong);
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  input.invalid {
    border-color: var(--warning);
  }
  .hint {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 5;
    white-space: nowrap;
    font-size: var(--fs-12);
    color: var(--warning);
  }
</style>
