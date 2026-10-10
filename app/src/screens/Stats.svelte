<script lang="ts">
  // Статистика игрового времени (раздел 5.17 ТЗ): всего, по сборкам, по дням,
  // где именно играл — миры, серверы, меню. Фильтры по видам и местам.
  import { fade } from "svelte/transition";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";
  import { api, type PlaceKind, type Summary } from "../lib/api";
  import { builds, refresh } from "../lib/builds.svelte";
  import { date, duration } from "../lib/format";
  import type { IconName } from "../lib/icons";
  import { ms } from "../lib/motion";

  // Смещение местного времени: ядро делит сессии по дням так, как их видит игрок.
  const offset = -new Date().getTimezoneOffset();
  const KINDS: { id: PlaceKind | null; title: string }[] = [
    { id: null, title: "Всё" },
    { id: "world", title: "Одиночная игра" },
    { id: "server", title: "Серверы" },
    { id: "menu", title: "Меню" },
  ];
  const PLACE_ICON: Record<PlaceKind, IconName> = { menu: "home", world: "globe", server: "servers" };

  let kind: PlaceKind | null = $state(null);
  /// Ключ выбранного мира или сервера (папка мира внутри сборки, адрес сервера).
  let place: string | null = $state(null);
  let hover: number | null = $state(null);
  let build: string | null = $state(null);
  let summary: Summary | null = $state(null);
  let error = $state("");

  if (!builds.loaded) refresh();

  $effect(() => {
    const filter = { kind, place, build, utc_offset_min: offset };
    api
      .stats(filter)
      .then((s) => ((summary = s), (error = "")))
      .catch((e) => (error = String(e)));
  });

  // Последние 30 дней, сегодня справа.
  const DAYS = 30;
  const chart = $derived.by(() => {
    const today = Math.floor((Date.now() / 1000 + offset * 60) / 86400);
    const byDay = new Map((summary?.days ?? []).map((d) => [d.day, d.seconds]));
    const days = Array.from({ length: DAYS }, (_, i) => {
      const day = today - (DAYS - 1 - i);
      return { day, seconds: byDay.get(day) ?? 0 };
    });
    const max = Math.max(1, ...days.map((d) => d.seconds));
    return { days, max };
  });

  function dayLabel(day: number, long = false): string {
    const d = new Date((day * 86400 - offset * 60) * 1000);
    return long
      ? d.toLocaleDateString("ru-RU", { day: "numeric", month: "long" })
      : d.toLocaleDateString("ru-RU", { day: "numeric", month: "numeric" });
  }

  function placeName(p: { kind: PlaceKind; name: string }): string {
    return p.kind === "menu" ? "Меню и загрузка" : p.name;
  }

  /// Нажатие на мир или сервер — показать только его, повторное — снять фильтр.
  let kindBefore: PlaceKind | null = null;
  function togglePlace(p: { key: string; kind: PlaceKind }) {
    if (place === p.key) {
      place = null;
      kind = kindBefore;
    } else {
      if (place === null) kindBefore = kind;
      kind = p.kind;
      place = p.key;
    }
  }

  // Название выбранного места для чипа: список мест сам не сужается фильтром.
  let placeLabel = $state("");
  $effect(() => {
    const found = summary?.places.find((p) => p.key === place);
    if (found) placeLabel = found.name;
  });

  function hoverLabel(day: number): string {
    const d = new Date((day * 86400 - offset * 60) * 1000);
    return d.toLocaleDateString("ru-RU", { weekday: "long", day: "numeric", month: "long" });
  }

  function pickKind(k: PlaceKind | null) {
    kind = k;
    place = null;
  }

  const buildOptions = $derived([
    { value: null as string | null, label: "Все сборки" },
    ...[...builds.list]
      .filter((b) => b.state === "active")
      .sort((a, b) => a.name.localeCompare(b.name, "ru"))
      .map((b) => ({ value: b.id as string | null, label: b.name })),
  ]);
</script>

<h1>Статистика</h1>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if summary}
  <div class="cards">
    <div class="card stat"><span class="muted small">Всего в игре</span><b>{duration(summary.total)}</b></div>
    <div class="card stat"><span class="muted small">Сессий</span><b>{summary.sessions}</b></div>
    <div class="card stat"><span class="muted small">Самая долгая</span><b>{duration(summary.longest)}</b></div>
    <div class="card stat">
      <span class="muted small">Считаем с</span><b>{summary.since ? date(summary.since) : "первой игры"}</b>
    </div>
  </div>

  <div class="filters">
    <div class="chips">
      {#each KINDS as k (k.id)}
        <button class="chip" class:on={kind === k.id} onclick={() => pickKind(k.id)}>{k.title}</button>
      {/each}
      {#if place}
        <button class="chip on" title="Убрать фильтр" onclick={() => ((place = null), (kind = kindBefore))}>
          {placeLabel}<Icon name="close" size={14} />
        </button>
      {/if}
    </div>
    <div class="build-select">
      <Select label="Сборка" icon="builds" value={build} options={buildOptions} onchange={(v) => (build = v)} />
    </div>
  </div>

  {#if summary.sessions === 0}
    <p class="note">
      {#if kind || place || build}По этому фильтру игр нет.{:else}Пока пусто. Время считается, когда игра запущена из
        лаунчера: всего, по сборкам, по мирам и серверам.{/if}
    </p>
  {:else}
    <section class="card block">
      <h2>Последние 30 дней</h2>
      <div class="chart" role="img" aria-label="Время в игре по дням" onmouseleave={() => (hover = null)}>
        {#each chart.days as d, i (d.day)}
          <div class="col" role="presentation" class:hover={hover === i} onmouseenter={() => (hover = i)}>
            <span class="bar-v" class:empty={!d.seconds} style:height="{(d.seconds / chart.max) * 100}%"></span>
            <span class="label">{i % 5 === 4 || i === DAYS - 1 ? dayLabel(d.day) : ""}</span>
          </div>
        {/each}
        {#if hover !== null}
          {@const d = chart.days[hover]}
          <div
            class="tip"
            style:left="{Math.min(88, Math.max(12, ((hover + 0.5) / DAYS) * 100))}%"
            transition:fade={{ duration: ms(100) }}
          >
            <span class="muted small">{hoverLabel(d.day)}</span>
            <b>{d.seconds ? duration(d.seconds) : "не играл"}</b>
          </div>
        {/if}
      </div>
    </section>

    <div class="cols">
      <section class="card block">
        <h2>По сборкам</h2>
        <ul class="list">
          {#each summary.builds as b (b.id)}
            <li>
              <span class="grow">{b.name}</span>
              <span class="muted small">{b.sessions} {b.sessions === 1 ? "сессия" : "сесс."}</span>
              <b class="time">{duration(b.seconds)}</b>
              <span class="track"><span style:width="{(b.seconds / summary.builds[0].seconds) * 100}%"></span></span>
            </li>
          {/each}
        </ul>
      </section>

      <section class="card block">
        <h2>Где играл</h2>
        <ul class="list">
          {#each summary.places as p (p.key)}
            <li>
              <Icon name={PLACE_ICON[p.kind]} size={18} />
              {#if p.kind === "menu"}
                <span class="grow">{placeName(p)}</span>
              {:else}
                <button
                  class="link grow"
                  class:on={place === p.key}
                  aria-pressed={place === p.key}
                  title={place === p.key ? "Показать все места" : "Показать только это место"}
                  onclick={() => togglePlace(p)}
                  >{placeName(p)}{#if p.build_name}<span class="muted small"> · {p.build_name}</span>{/if}</button
                >
              {/if}
              <b class="time">{duration(p.seconds)}</b>
              <span class="track"><span style:width="{(p.seconds / summary.places[0].seconds) * 100}%"></span></span>
            </li>
          {/each}
        </ul>
        <p class="muted small">
          Миры и серверы лаунчер узнаёт из лога игры. На версиях до 1.15 выход с сервера в меню не виден — это время
          считается сервером.
        </p>
      </section>
    </div>
  {/if}
{/if}

<style>
  h1 {
    margin: 0 0 var(--s5);
    font-size: var(--fs-32);
  }
  h2 {
    margin: 0 0 var(--s4);
    font-size: var(--fs-16);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--s3);
    margin-bottom: var(--s5);
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    padding: var(--s4) var(--s5);
  }
  .stat b {
    font-size: var(--fs-24);
    font-weight: 600;
  }
  .filters {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s3);
    margin-bottom: var(--s4);
  }
  .chip {
    gap: var(--s1);
  }
  .build-select {
    min-width: 240px;
  }
  .block {
    padding: var(--s5);
    margin-bottom: var(--s4);
  }
  .chart {
    position: relative;
    display: flex;
    align-items: stretch;
    gap: 4px;
    height: 160px;
  }
  .col {
    display: flex;
    flex: 1;
    flex-direction: column;
    justify-content: flex-end;
    gap: var(--s1);
  }
  .bar-v {
    display: block;
    min-height: 2px;
    border-radius: 3px 3px 0 0;
    background: var(--accent);
    opacity: 0.85;
    transition: opacity var(--t-fast) var(--ease);
  }
  .bar-v.empty {
    background: var(--border);
  }
  .col.hover .bar-v {
    opacity: 1;
  }
  .col.hover .bar-v.empty {
    background: var(--border-strong);
  }
  .tip {
    position: absolute;
    top: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: var(--s2) var(--s3);
    background: var(--surface-raised);
    border: 1px solid var(--border-strong);
    border-radius: var(--r);
    transform: translate(-50%, -105%);
    white-space: nowrap;
    pointer-events: none;
  }
  .label {
    height: 16px;
    font-size: var(--fs-12);
    color: var(--text-muted);
    text-align: center;
    white-space: nowrap;
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--s4);
  }
  .cols .block {
    margin: 0;
  }
  .list {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .list li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px var(--s3);
    padding: var(--s2) 0;
  }
  .time {
    font-variant-numeric: tabular-nums;
  }
  .track {
    flex-basis: 100%;
    height: 4px;
    border-radius: 2px;
    background: var(--surface-raised);
  }
  .track span {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .link:hover,
  .link.on {
    text-decoration: underline;
  }
  .block > p {
    margin: var(--s3) 0 0;
  }
</style>
