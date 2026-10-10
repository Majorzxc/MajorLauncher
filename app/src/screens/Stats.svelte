<script lang="ts">
  // Статистика игрового времени (раздел 5.17 ТЗ): всего, по сборкам, по дням,
  // где именно играл — миры, серверы, меню. Фильтры по видам и местам.
  import Icon from "../components/Icon.svelte";
  import { api, type PlaceKind, type Summary } from "../lib/api";
  import { builds, refresh } from "../lib/builds.svelte";
  import { date, duration } from "../lib/format";
  import type { IconName } from "../lib/icons";

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
  let place: string | null = $state(null);
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

  function placeName(p: { kind: PlaceKind; name?: string }): string {
    return p.kind === "menu" ? "Меню и загрузка" : (p.name ?? "");
  }

  function pickKind(k: PlaceKind | null) {
    kind = k;
    place = null;
  }

  const buildOptions = $derived(
    [...builds.list].filter((b) => b.state === "active").sort((a, b) => a.name.localeCompare(b.name, "ru")),
  );
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
        <button class="chip on" title="Убрать фильтр" onclick={() => (place = null)}>
          {place}<Icon name="close" size={14} />
        </button>
      {/if}
    </div>
    <select class="input" bind:value={build} aria-label="Сборка">
      <option value={null}>Все сборки</option>
      {#each buildOptions as b (b.id)}<option value={b.id}>{b.name}</option>{/each}
    </select>
  </div>

  {#if summary.sessions === 0}
    <p class="note">
      {#if kind || place || build}По этому фильтру игр нет.{:else}Пока пусто. Время считается, когда игра запущена из
        лаунчера: всего, по сборкам, по мирам и серверам.{/if}
    </p>
  {:else}
    <section class="card block">
      <h2>Последние 30 дней</h2>
      <div class="chart">
        {#each chart.days as d, i (d.day)}
          <div class="col" title="{dayLabel(d.day, true)}: {d.seconds ? duration(d.seconds) : 'не играл'}">
            <span class="bar-v" style:height="{(d.seconds / chart.max) * 100}%"></span>
            <span class="label">{i % 5 === 4 || i === DAYS - 1 ? dayLabel(d.day) : ""}</span>
          </div>
        {/each}
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
          {#each summary.places as p (p.kind + (p.name ?? ""))}
            <li>
              <Icon name={PLACE_ICON[p.kind]} size={18} />
              {#if p.kind === "menu"}
                <span class="grow">{placeName(p)}</span>
              {:else}
                <button
                  class="link grow"
                  class:on={place === p.name}
                  title="Показать только это место"
                  onclick={() => ((kind = p.kind), (place = p.name ?? null))}>{placeName(p)}</button
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
  select {
    min-width: 220px;
  }
  .block {
    padding: var(--s5);
    margin-bottom: var(--s4);
  }
  .chart {
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
