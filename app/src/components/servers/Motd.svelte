<script lang="ts">
  // Описание сервера, как в игре: цвета и начертание из готовых кусков ядра.
  // Никакого HTML от сервера — только текст и цвет #rrggbb. Фон всегда
  // тёмный, как в меню игры: описания рассчитаны на него (белый и жёлтый
  // текст на светлой теме не прочитать).
  import type { Span } from "../../lib/api";

  let { spans, lines = 2 }: { spans: Span[]; lines?: number } = $props();

  // «Мельтешащий» текст (§k) — без анимации: процессору слабого ПК она не нужна.
  const NOISE = "▓▒░█▚▞";
  function noise(text: string) {
    return [...text].map((c, i) => (c === " " || c === "\n" ? c : NOISE[(c.charCodeAt(0) + i) % NOISE.length])).join("");
  }
</script>

<div class="motd" style:--lines={lines}>
  {#each spans as s}<span
      style:color={s.color}
      class:b={s.bold}
      class:it={s.italic}
      class:u={s.underlined}
      class:st={s.strikethrough}>{s.obfuscated ? noise(s.text) : s.text}</span
    >{/each}
</div>

<style>
  .motd {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: var(--lines);
    line-clamp: var(--lines);
    overflow: hidden;
    padding: 2px var(--s2);
    border-radius: var(--r-sm);
    background: #101012;
    color: #aaaaaa;
    font-size: var(--fs-14);
    line-height: 1.45;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }
  .b {
    font-weight: 700;
  }
  .it {
    font-style: italic;
  }
  .u {
    text-decoration: underline;
  }
  .st {
    text-decoration: line-through;
  }
  .u.st {
    text-decoration: underline line-through;
  }
</style>
