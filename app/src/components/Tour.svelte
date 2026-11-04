<script lang="ts">
  // Подсказка обучения: затемнение окна, «окошко» вокруг элемента и пузырь с
  // текстом (раздел 6.4 ТЗ). Без размытия и теней — только рамка и полупрозрачный
  // фон: бережём слабые ПК. Шаги — lib/tours.ts.
  import { fade } from "svelte/transition";
  import { ms } from "../lib/motion";
  import { TOURS, type TourStep } from "../lib/tours";
  import { finishTour, stopTutorial, tutorial } from "../lib/tutorial.svelte";

  const PAD = 6;
  const GAP = 12;
  const W = 340;

  // Тур меняется только со сменой id: пересоздание того же тура его не сбрасывает.
  const id = $derived(tutorial.active?.id ?? null);
  let steps = $state<TourStep[]>([]);
  let index = $state(0);
  let rect = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  let bubbleH = $state(0);
  let vw = $state(window.innerWidth);
  let vh = $state(window.innerHeight);
  let nextBtn = $state<HTMLButtonElement>();

  function find(s: TourStep): Element | null {
    const el = document.querySelector(`[data-tour="${s.target}"]`);
    return (s.first ? el?.firstElementChild : el) ?? null;
  }

  function shown(el: Element | null): el is Element {
    if (!el) return false;
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  }

  // Раздел дорисовывается не сразу (списки приходят от ядра): ждём, пока
  // появится хоть один элемент тура, и берём только те шаги, что есть на экране.
  $effect(() => {
    const tour = id;
    steps = [];
    rect = null;
    if (!tour) return;
    const since = performance.now();
    let timer = setTimeout(function wait() {
      const found = TOURS[tour].filter((s) => shown(find(s)));
      const waited = performance.now() - since;
      if ((found.length && waited >= 400) || waited >= 2500) {
        if (!found.length) {
          // Показать нечего (раздел пуст) — тур не считается увиденным.
          tutorial.active = null;
          return;
        }
        steps = found;
        index = 0;
        place(true);
      } else {
        timer = setTimeout(wait, 100);
      }
    }, 100);
    return () => clearTimeout(timer);
  });

  function place(scroll: boolean) {
    const s = steps[index];
    const el = s ? find(s) : null;
    if (!shown(el)) {
      rect = null;
      return;
    }
    if (scroll) el.scrollIntoView({ block: "nearest" });
    const r = el.getBoundingClientRect();
    const next = { x: r.left, y: r.top, w: r.width, h: r.height };
    if (!rect || Object.entries(next).some(([k, v]) => Math.abs(v - rect![k as keyof typeof next]) > 0.5)) rect = next;
  }

  // Элемент может сдвинуться: догрузилась картинка, изменилось окно.
  $effect(() => {
    if (!steps.length) return;
    const timer = setInterval(() => place(false), 250);
    return () => clearInterval(timer);
  });

  $effect(() => {
    if (steps.length) nextBtn?.focus();
  });

  function next() {
    if (index < steps.length - 1) {
      index++;
      place(true);
    } else finishTour();
  }
  function back() {
    if (index > 0) {
      index--;
      place(true);
    }
  }

  function keys(e: KeyboardEvent) {
    if (!steps.length) return;
    if (e.key === "Escape") finishTour();
    else if (e.key === "ArrowRight") next();
    else if (e.key === "ArrowLeft" && !e.altKey) back();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(v, hi));

  // Пузырь — под элементом, над ним, справа или слева: где поместится.
  const pos = $derived.by(() => {
    const h = bubbleH || 160;
    if (!rect) return { top: (vh - h) / 2, left: (vw - W) / 2 };
    const r = { top: rect.y - PAD, bottom: rect.y + rect.h + PAD, left: rect.x - PAD, right: rect.x + rect.w + PAD };
    let top: number;
    let left: number;
    if (r.bottom + GAP + h <= vh - 8) [top, left] = [r.bottom + GAP, r.left];
    else if (r.top - GAP - h >= 8) [top, left] = [r.top - GAP - h, r.left];
    else if (r.right + GAP + W <= vw - 8) [top, left] = [r.top, r.right + GAP];
    else if (r.left - GAP - W >= 8) [top, left] = [r.top, r.left - GAP - W];
    else [top, left] = [vh - h - 16, (vw - W) / 2];
    return { top: clamp(top, 8, vh - h - 8), left: clamp(left, 8, vw - W - 8) };
  });

  const step = $derived(steps[index]);
</script>

<svelte:window bind:innerWidth={vw} bind:innerHeight={vh} onkeydowncapture={keys} />

{#if step}
  <div class="tour" transition:fade={{ duration: ms(120) }}>
    <!-- Клики мимо пузыря не доходят до окна: подсказка не уводит из раздела. -->
    <div class="catch" role="presentation"></div>
    {#if rect}
      <div
        class="hole"
        style:left="{rect.x - PAD}px"
        style:top="{rect.y - PAD}px"
        style:width="{rect.w + PAD * 2}px"
        style:height="{rect.h + PAD * 2}px"
        style:transition-duration="{ms(160)}ms"
      ></div>
    {:else}
      <div class="dim"></div>
    {/if}
    <div
      class="bubble card"
      role="dialog"
      aria-modal="true"
      aria-labelledby="tour-title"
      style:left="{pos.left}px"
      style:top="{pos.top}px"
      style:width="{W}px"
      bind:clientHeight={bubbleH}
    >
      <div class="top">
        <span class="small muted">{index + 1} из {steps.length}</span>
        <button class="link small muted" onclick={stopTutorial} title="Подсказки разделов больше не появятся сами"
          >Выключить подсказки</button
        >
      </div>
      <h2 id="tour-title">{step.title}</h2>
      <p class="text">{step.text}</p>
      <div class="buttons">
        {#if index > 0}<button class="btn btn-sm btn-quiet" onclick={back}>Назад</button>{/if}
        {#if index < steps.length - 1}
          <button class="btn btn-sm btn-quiet" onclick={finishTour}>Пропустить</button>
        {/if}
        <button class="btn btn-sm btn-primary" bind:this={nextBtn} onclick={next}>
          {index < steps.length - 1 ? "Далее" : "Готово"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .tour {
    position: fixed;
    inset: 0;
    z-index: 900;
  }
  .catch {
    position: absolute;
    inset: 0;
  }
  /* «Окошко»: огромная тень вокруг элемента затемняет всё остальное. */
  .hole {
    position: absolute;
    border-radius: var(--r);
    box-shadow: 0 0 0 200vmax rgb(0 0 0 / 0.58);
    outline: 2px solid var(--accent);
    outline-offset: 0;
    pointer-events: none;
    transition-property: left, top, width, height;
    transition-timing-function: ease-out;
  }
  .dim {
    position: absolute;
    inset: 0;
    background: rgb(0 0 0 / 0.58);
  }
  .bubble {
    position: absolute;
    padding: var(--s4) var(--s5);
    border-color: var(--border-strong);
  }
  .top {
    display: flex;
    justify-content: space-between;
    margin-bottom: 2px;
  }
  h2 {
    margin: 0 0 var(--s2);
    font-size: var(--fs-16);
    font-weight: 600;
  }
  .text {
    margin: 0 0 var(--s4);
    font-size: var(--fs-14);
    line-height: 1.45;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: var(--s1);
  }
  .link {
    padding: 0;
    white-space: nowrap;
    border: 0;
    background: none;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .link:hover {
    color: var(--text);
  }
</style>
