<script lang="ts">
  // Главный экран (раздел 6.1 ТЗ, вариант A): последняя сборка, большая
  // кнопка «Играть», ряд недавних сборок.
  import ContextMenu, { type MenuItem } from "../components/ContextMenu.svelte";
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import PlayButton from "../components/PlayButton.svelte";
  import { accounts } from "../lib/accounts.svelte";
  import { builds, refresh } from "../lib/builds.svelte";
  import { ago, playtime } from "../lib/format";
  import { go, nav, openBuild } from "../lib/nav.svelte";
  import { busyLabel, start } from "../lib/play.svelte";

  refresh();

  const byRecent = $derived(
    builds.list
      .filter((b) => b.state === "active")
      .sort((a, b) => (b.last_played ?? 0) - (a.last_played ?? 0) || b.created - a.created),
  );
  const current = $derived(byRecent.find((b) => b.id === nav.homeBuild) ?? byRecent[0]);
  const recent = $derived(byRecent.filter((b) => b.id !== current?.id).slice(0, 4));

  let menu: { x: number; y: number } | null = $state(null);
  const switchItems = $derived<MenuItem[]>(
    byRecent.map((b) => ({
      label: b.name,
      icon: b.id === current?.id ? "check" : "builds",
      hint: b.version,
      onclick: () => (nav.homeBuild = b.id),
    })),
  );

  function lastRun(at: number | null): string {
    return at ? `запуск ${ago(at)}` : "ещё не запускалась";
  }
</script>

{#if !builds.loaded}
  <!-- пусто: список сборок загружается -->
{:else if !current}
  <div class="empty card">
    <Icon name="builds" size={40} />
    <h1>Сборок пока нет</h1>
    <p class="muted">Сборка — это версия игры со своими мирами, модами и настройками.</p>
    <button class="btn btn-primary" onclick={() => go("builds")}><Icon name="plus" size={18} />Создать сборку</button>
  </div>
{:else}
  <section class="hero card">
    <button class="cover-btn" aria-label="Открыть сборку {current.name}" onclick={() => openBuild(current.id)}>
      <Cover id={current.id} size={176} />
    </button>
    <div class="info">
      <div class="title">
        <h1>{current.name}</h1>
        {#if byRecent.length > 1}
          <button
            class="btn btn-quiet btn-sm switch"
            aria-label="Сменить сборку"
            title="Сменить сборку"
            onclick={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              menu = { x: r.left, y: r.bottom + 4 };
            }}><Icon name="chevron-down" size={20} /></button
          >
        {/if}
      </div>
      <p class="muted">{current.version} · Ванилла · без модов</p>
      <p class="meta muted">
        <span><Icon name="time" size={16} />{playtime(current.playtime)} в игре</span>
        <span>{lastRun(current.last_played)}</span>
      </p>
      <div class="actions">
        <PlayButton id={current.id} large />
        <button class="btn" onclick={() => openBuild(current.id)}>Открыть сборку</button>
      </div>
      {#if accounts.loaded && accounts.list.length === 0}
        <p class="warn"><Icon name="info" size={18} />Чтобы играть, добавьте ник или войдите через Ely.by — справа вверху.</p>
      {/if}
    </div>
  </section>

  {#if recent.length}
    <h2>Недавние</h2>
    <div class="recent">
      {#each recent as b (b.id)}
        <div
          class="mini card"
          role="button"
          tabindex="0"
          title="Поставить на главный экран"
          onclick={() => (nav.homeBuild = b.id)}
          onkeydown={(e) => e.key === "Enter" && (nav.homeBuild = b.id)}
        >
          <Cover id={b.id} size={56} />
          <div class="mini-info">
            <b class="grow">{b.name}</b>
            <span class="muted small grow">{b.version} · {b.last_played ? ago(b.last_played) : "не запускалась"}</span>
          </div>
          <button
            class="mini-play"
            aria-label="Играть в {b.name}"
            title={busyLabel(b.id) ?? "Играть"}
            disabled={!!busyLabel(b.id)}
            onclick={(e) => {
              e.stopPropagation();
              start(b.id);
            }}><Icon name="play" size={18} /></button
          >
        </div>
      {/each}
    </div>
  {/if}
{/if}

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={switchItems} onclose={() => (menu = null)} />
{/if}

<style>
  h1 {
    margin: 0;
    font-size: var(--fs-32);
    line-height: 1.2;
  }
  h2 {
    margin: var(--s6) 0 var(--s3);
    font-size: var(--fs-20);
  }
  .hero {
    display: flex;
    gap: var(--s6);
    padding: var(--s6);
  }
  .cover-btn {
    padding: 0;
    border: 0;
    background: none;
    border-radius: var(--r);
    cursor: pointer;
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: var(--s2);
    min-width: 0;
  }
  .title {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
  .info p {
    margin: 0;
  }
  .meta {
    display: flex;
    gap: var(--s4);
    font-size: var(--fs-14);
  }
  .meta span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .actions {
    display: flex;
    gap: var(--s3);
    align-items: center;
    margin-top: auto;
    padding-top: var(--s4);
  }
  .warn {
    margin-top: var(--s2);
  }
  .recent {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: var(--s3);
  }
  .mini {
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: var(--s3);
    cursor: pointer;
  }
  .mini:hover {
    border-color: var(--border-strong);
  }
  .mini-info {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .mini-play {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    flex: none;
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--surface-raised);
    cursor: pointer;
  }
  .mini-play:hover:not(:disabled) {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .mini-play:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s3);
    max-width: 520px;
    margin: var(--s7) auto;
    padding: var(--s7) var(--s6);
    text-align: center;
  }
  .empty p {
    margin: 0;
  }
</style>
