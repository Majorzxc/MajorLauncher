<script lang="ts">
  // Статистика игрового времени (раздел 5.17 ТЗ): за неделю, месяц, год или
  // всё время; по сборкам, мирам и серверам. Клик по миру или серверу —
  // подробности и вся страница только про него.
  import { fade } from "svelte/transition";
  import Icon from "../components/Icon.svelte";
  import Select from "../components/Select.svelte";
  import { api, type Summary } from "../lib/api";
  import { builds, refresh } from "../lib/builds.svelte";
  import { ago, date, duration } from "../lib/format";
  import { ms } from "../lib/motion";

  type Period = "week" | "month" | "year" | "all";
  type Place = Summary["places"][number];

  // Смещение местного времени: ядро делит сессии по дням так, как их видит игрок.
  const offset = -new Date().getTimezoneOffset();
  const DAY = 86400;
  const PERIODS: { id: Period; title: string }[] = [
    { id: "week", title: "Неделя" },
    { id: "month", title: "Месяц" },
    { id: "year", title: "Год" },
    { id: "all", title: "Всё время" },
  ];

  let period: Period = $state("month");
  let build: string | null = $state(null);
  /// Ключ выбранного мира или сервера.
  let place: string | null = $state(null);
  let summary = $state<Summary | null>(null);
  let error = $state("");
  let hover: number | null = $state(null);

  if (!builds.loaded) refresh();

  const today = Math.floor((Date.now() / 1000 + offset * 60) / DAY);
  /// Местная полночь дня `day` в секундах с 1970.
  const midnight = (day: number) => day * DAY - offset * 60;
  const dayDate = (day: number) => new Date(midnight(day) * 1000);

  function periodStart(p: Period): number | null {
    if (p === "week") return midnight(today - 6);
    if (p === "month") return midnight(today - 29);
    if (p === "year") {
      const d = dayDate(today);
      return new Date(d.getFullYear(), d.getMonth() - 11, 1).getTime() / 1000;
    }
    return null;
  }

  $effect(() => {
    const filter = { build, place, utc_offset_min: offset, from: periodStart(period) };
    api
      .stats(filter)
      .then((s) => ((summary = s), (error = "")))
      .catch((e) => (error = String(e)));
  });

  // Столбики: по дням за неделю и месяц, по месяцам за год и всё время.
  const chart = $derived.by(() => {
    const days = summary?.days ?? [];
    let bars: { label: string; tip: string; seconds: number; mark: boolean }[];
    if (period === "week" || period === "month") {
      const n = period === "week" ? 7 : 30;
      const byDay = new Map(days.map((d) => [d.day, d.seconds]));
      bars = Array.from({ length: n }, (_, i) => {
        const day = today - (n - 1 - i);
        const d = dayDate(day);
        return {
          label: d.toLocaleDateString("ru-RU", n === 7 ? { weekday: "short" } : { day: "numeric", month: "numeric" }),
          tip: d.toLocaleDateString("ru-RU", { weekday: "long", day: "numeric", month: "long" }),
          seconds: byDay.get(day) ?? 0,
          mark: n === 7 || i % 5 === 4 || i === n - 1,
        };
      });
    } else {
      const month = (d: Date) => d.getFullYear() * 12 + d.getMonth();
      const byMonth = new Map<number, number>();
      for (const d of days) {
        const m = month(dayDate(d.day));
        byMonth.set(m, (byMonth.get(m) ?? 0) + d.seconds);
      }
      const last = month(dayDate(today));
      const first = period === "year" ? last - 11 : Math.min(last - 11, ...byMonth.keys());
      bars = Array.from({ length: last - first + 1 }, (_, i) => {
        const m = first + i;
        const d = new Date(Math.floor(m / 12), m % 12, 1);
        return {
          label: d.toLocaleDateString("ru-RU", { month: "short" }).replace(".", ""),
          tip: d.toLocaleDateString("ru-RU", { month: "long", year: "numeric" }),
          seconds: byMonth.get(m) ?? 0,
          mark: true,
        };
      });
    }
    return { bars, max: Math.max(1, ...bars.map((b) => b.seconds)) };
  });

  const worlds = $derived(summary?.places.filter((p) => p.kind === "world") ?? []);
  const servers = $derived(summary?.places.filter((p) => p.kind === "server") ?? []);
  const menuSeconds = $derived(summary?.places.find((p) => p.kind === "menu")?.seconds ?? 0);
  const selected = $derived(summary?.places.find((p) => p.key === place) ?? null);
  const playedDays = $derived(summary?.days.filter((d) => d.seconds > 0).length ?? 0);

  function toggle(p: Place) {
    place = place === p.key ? null : p.key;
  }

  const buildOptions = $derived([
    { value: null as string | null, label: "Все сборки" },
    ...[...builds.list]
      .filter((b) => b.state === "active")
      .sort((a, b) => a.name.localeCompare(b.name, "ru"))
      .map((b) => ({ value: b.id as string | null, label: b.name })),
  ]);

  const percent = (part: number, whole: number) => Math.round((part / Math.max(1, whole)) * 100);
</script>

{#snippet placeList(title: string, list: Place[], empty: string)}
  <section class="card block">
    <h2>{title}</h2>
    {#if list.length === 0}
      <p class="muted small">{empty}</p>
    {:else}
      <ul class="list">
        {#each list as p (p.key)}
          <li>
            <button
              class="link grow"
              class:on={place === p.key}
              aria-pressed={place === p.key}
              title={place === p.key ? "Показать всё" : "Подробнее"}
              onclick={() => toggle(p)}
              >{p.name}{#if p.build_name}<span class="muted small"> · {p.build_name}</span>{/if}</button
            >
            <b class="time">{duration(p.seconds)}</b>
            <span class="track"><span style:width="{(p.seconds / list[0].seconds) * 100}%"></span></span>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{/snippet}

<h1>Статистика</h1>

<div class="filters">
  <div class="chips" role="radiogroup" aria-label="Период">
    {#each PERIODS as p (p.id)}
      <button class="chip" role="radio" aria-checked={period === p.id} class:on={period === p.id} onclick={() => (period = p.id)}>
        {p.title}
      </button>
    {/each}
  </div>
  <div class="build-select">
    <Select shrink label="Сборка" icon="builds" value={build} options={buildOptions} onchange={(v) => ((build = v), (place = null))} />
  </div>
</div>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if summary}
  {#if selected}
    <div class="selected card" transition:fade={{ duration: ms(120) }}>
      <Icon name={selected.kind === "world" ? "globe" : "servers"} size={24} />
      <div class="grow">
        <b>{selected.name}</b>
        <span class="muted small">
          {selected.kind === "world" ? `одиночный мир · ${selected.build_name ?? ""}` : "сервер"} · впервые {date(
            selected.first,
          )} · последний раз {ago(selected.last)}
        </span>
      </div>
      <button class="btn btn-sm btn-quiet" onclick={() => (place = null)}><Icon name="close" size={16} />Показать всё</button>
    </div>
  {/if}

  <div class="cards">
    <div class="card stat">
      <span class="muted small">{selected ? "Всего здесь" : "Всего в игре"}</span>
      <b>{duration(summary.total)}</b>
      {#if !selected && menuSeconds}
        <span class="muted small">из них в меню и загрузке {percent(menuSeconds, summary.total)}%</span>
      {/if}
    </div>
    <div class="card stat"><span class="muted small">Сессий</span><b>{summary.sessions}</b></div>
    <div class="card stat"><span class="muted small">Самая долгая</span><b>{duration(summary.longest)}</b></div>
    <div class="card stat">
      {#if selected}
        <span class="muted small">В среднем за раз</span><b>{duration(Math.round(summary.total / Math.max(1, summary.sessions)))}</b>
      {:else if period === "all"}
        <span class="muted small">Считаем с</span><b>{summary.since ? date(summary.since) : "первой игры"}</b>
      {:else}
        <span class="muted small">Дней с игрой</span><b>{playedDays}</b>
      {/if}
    </div>
  </div>

  {#if summary.sessions === 0}
    <p class="note">
      {#if build || period !== "all"}За этот период игр нет.{:else}Пока пусто. Время считается, когда игра запущена из
        лаунчера: всего, по сборкам, по мирам и серверам.{/if}
    </p>
  {:else}
    <section class="card block">
      <div class="chart" role="img" aria-label="Время в игре" onmouseleave={() => (hover = null)}>
        {#each chart.bars as b, i (i)}
          <div class="col" role="presentation" class:hover={hover === i} onmouseenter={() => (hover = i)}>
            <span class="bar-v" class:empty={!b.seconds} style:height="{(b.seconds / chart.max) * 100}%"></span>
            <span class="label">{b.mark ? b.label : ""}</span>
          </div>
        {/each}
        {#if hover !== null && chart.bars[hover]}
          {@const b = chart.bars[hover]}
          <div
            class="tip"
            style:left="{Math.min(88, Math.max(12, ((hover + 0.5) / chart.bars.length) * 100))}%"
            transition:fade={{ duration: ms(100) }}
          >
            <span class="muted small">{b.tip}</span>
            <b>{b.seconds ? duration(b.seconds) : "не играл"}</b>
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
      {@render placeList("Миры", worlds, "В одиночных мирах не играл.")}
      {@render placeList("Серверы", servers, "На серверах не играл.")}
    </div>
    <p class="muted small foot">
      Миры и серверы лаунчер узнаёт из лога игры. На версиях до 1.15 выход с сервера в меню не виден — это время
      считается сервером.
    </p>
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
  .filters {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s3);
    margin-bottom: var(--s4);
  }
  .build-select {
    min-width: 240px;
  }
  .selected {
    display: flex;
    align-items: center;
    gap: var(--s4);
    padding: var(--s4) var(--s5);
    margin-bottom: var(--s4);
    border-color: var(--accent);
  }
  .selected .grow {
    display: flex;
    flex-direction: column;
    white-space: normal;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--s3);
    margin-bottom: var(--s4);
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
    line-height: 1.2;
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
    margin-top: var(--s5);
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
  .label {
    height: 16px;
    font-size: var(--fs-12);
    color: var(--text-muted);
    text-align: center;
    white-space: nowrap;
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
    transform: translate(-50%, -100%);
    white-space: nowrap;
    pointer-events: none;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
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
  .link.on {
    font-weight: 600;
  }
  .foot {
    margin: var(--s4) 0 0;
  }
</style>
