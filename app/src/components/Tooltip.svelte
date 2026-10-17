<script lang="ts">
  // Подсказки в оформлении лаунчера вместо системных: у любого элемента с
  // title текст переносится в data-tip (системная подсказка не всплывает) и
  // показывается здесь после короткой задержки.
  import { onMount } from "svelte";

  let text = $state("");
  let x = $state(0);
  let y = $state(0);
  let below = $state(true);
  let tip = $state<HTMLDivElement | null>(null);

  onMount(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let target: HTMLElement | null = null;

    const hide = () => {
      clearTimeout(timer);
      target = null;
      text = "";
    };

    const over = (e: PointerEvent) => {
      const el = (e.target as Element | null)?.closest<HTMLElement>("[title], [data-tip]");
      if (!el || el === target) return;
      const title = el.getAttribute("title");
      if (title !== null) {
        el.dataset.tip = title;
        el.removeAttribute("title");
        // Подпись для экранных дикторов остаётся.
        if (!el.hasAttribute("aria-label") && !el.textContent?.trim()) el.setAttribute("aria-label", title);
      }
      hide();
      target = el;
      timer = setTimeout(() => {
        if (target !== el || !el.isConnected || !el.dataset.tip) return;
        const r = el.getBoundingClientRect();
        x = r.left + r.width / 2;
        below = r.bottom + 40 < window.innerHeight;
        y = below ? r.bottom + 6 : r.top - 6;
        text = el.dataset.tip;
      }, 450);
    };

    const out = (e: PointerEvent) => {
      if (!target) return;
      const to = e.relatedTarget as Node | null;
      if (to && target.contains(to)) return;
      hide();
    };

    document.addEventListener("pointerover", over);
    document.addEventListener("pointerout", out);
    document.addEventListener("pointerdown", hide, true);
    window.addEventListener("scroll", hide, true);
    window.addEventListener("blur", hide);
    return () => {
      document.removeEventListener("pointerover", over);
      document.removeEventListener("pointerout", out);
      document.removeEventListener("pointerdown", hide, true);
      window.removeEventListener("scroll", hide, true);
      window.removeEventListener("blur", hide);
    };
  });

  // Не вылезать за края окна.
  const shift = $derived.by(() => {
    if (!tip || !text) return 0;
    const half = tip.offsetWidth / 2;
    if (x - half < 8) return 8 - (x - half);
    if (x + half > window.innerWidth - 8) return window.innerWidth - 8 - (x + half);
    return 0;
  });
</script>

{#if text}
  <div
    bind:this={tip}
    class="tip"
    class:above={!below}
    role="tooltip"
    style="left: {x + shift}px; top: {y}px"
  >
    {text}
  </div>
{/if}

<style>
  .tip {
    position: fixed;
    z-index: 1000;
    transform: translateX(-50%);
    max-width: 320px;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--surface);
    color: var(--text);
    font-size: var(--fs-12);
    line-height: 1.35;
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.35);
    pointer-events: none;
    animation: appear 0.12s ease-out;
  }
  .above {
    transform: translate(-50%, -100%);
  }
  @keyframes appear {
    from {
      opacity: 0;
    }
  }
</style>
