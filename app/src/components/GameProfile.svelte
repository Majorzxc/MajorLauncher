<script lang="ts">
  // Настройки игры (раздел 5.18 ТЗ): всё нужное из меню игры на одной
  // странице, с поиском. Это общий профиль для всех сборок: лаунчер пишет его
  // в options.txt перед запуском и забирает изменения после игры.
  import { api, type Group, type OptionsView, type Setting } from "../lib/api";
  import { builds, refresh } from "../lib/builds.svelte";
  import { attempt, toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";

  const GROUPS: { id: Group; title: string; hint?: string }[] = [
    { id: "view", title: "Обзор и эффекты" },
    { id: "controls", title: "Управление" },
    { id: "sound", title: "Звук" },
    { id: "chat", title: "Чат" },
    { id: "interface", title: "Интерфейс" },
    { id: "language", title: "Язык" },
    { id: "graphics", title: "Графика", hint: "По умолчанию у каждой сборки своя: тяжёлым сборкам нужна другая дальность прорисовки." },
    { id: "keys", title: "Клавиши" },
    { id: "packs", title: "Включённые ресурспаки" },
  ];

  let opts: OptionsView | null = $state(null);
  let error = $state("");
  let query = $state("");

  api
    .options()
    .then((o) => (opts = o))
    .catch((e) => (error = String(e)));
  if (!builds.loaded) refresh();

  const q = $derived(query.trim().toLowerCase());
  function visible(s: Setting): boolean {
    return !q || s.label.toLowerCase().includes(q) || (s.hint ?? "").toLowerCase().includes(q);
  }
  const groups = $derived(
    GROUPS.map((g) => ({
      ...g,
      shared: opts?.shared.includes(g.id) ?? false,
      items: (opts?.settings ?? []).filter((s) => s.group === g.id && visible(s)),
    })).filter((g) => !q || g.items.length || g.title.toLowerCase().includes(q)),
  );

  const value = (s: Setting) => opts?.values[s.key] ?? s.default;

  async function set(s: Setting, v: string) {
    const next = await attempt(s.label, () => api.setOption(s.key, v));
    if (next) opts = next;
  }

  async function share(g: Group, shared: boolean) {
    const next = await attempt("Настройки игры", () => api.setShared(g, shared));
    if (next) opts = next;
  }

  /// Ползунок показывает градусы и проценты, а игра хранит дробь.
  function shown(s: Setting): number {
    const raw = Number(value(s));
    if (s.kind.type === "slider") return Math.round(raw * s.kind.mul + s.kind.add);
    return raw;
  }
  function stored(s: Setting, v: number): string {
    if (s.kind.type === "slider") return String(Math.round(((v - s.kind.add) / s.kind.mul) * 10000) / 10000);
    return String(Math.round(v));
  }
  function label(s: Setting, v: number): string {
    if (s.kind.type === "slider") return `${v}${s.kind.unit}`;
    if (s.kind.type === "int" && s.kind.special && s.kind.special[0] === v) return s.kind.special[1];
    return String(v);
  }

  // «Взять настройки из сборки».
  let seedFrom: string | null = $state(null);
  const buildOptions = $derived([
    { value: null as string | null, label: "Выберите сборку…" },
    ...builds.list
      .filter((b) => b.state === "active")
      .map((b) => ({ value: b.id as string | null, label: b.name })),
  ]);
  async function seed() {
    if (!seedFrom) return;
    const next = await attempt("Настройки из сборки", () => api.seedOptions(seedFrom!));
    if (next) {
      opts = next;
      toast(`Настройки взяты из сборки «${next.seeded_from}»`);
      seedFrom = null;
    }
  }
</script>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if opts}
  <div class="card block intro">
    <p>
      Общие для всех сборок: перед запуском лаунчер записывает их в сборку, а то, что вы поменяли в самой игре, после
      выхода попадает сюда и во все остальные сборки. Параметры, которых нет в старой версии, ей просто не передаются.
    </p>
    {#if opts.seeded_from}
      <p class="muted small">Сначала взяты из сборки «{opts.seeded_from}».</p>
    {:else if Object.keys(opts.values).length === 0}
      <p class="muted small">Пока пусто: заполнятся при первом запуске любой сборки — или возьмите их из сборки.</p>
    {/if}
    <div class="seed">
      <div class="grow-select"><Select label="Сборка" value={seedFrom} options={buildOptions} onchange={(v) => (seedFrom = v)} /></div>
      <button class="btn" disabled={!seedFrom} onclick={seed}>Взять настройки из сборки</button>
    </div>
  </div>

  <label class="search">
    <Icon name="search" size={18} />
    <input type="search" placeholder="Поиск: FOV, покачивание, чувствительность…" bind:value={query} />
  </label>

  {#each groups as g (g.id)}
    <section class="card block">
      <header>
        <h2>{g.title}</h2>
        <label class="shared" title="Общие для всех сборок">
          <span class="muted small">Для всех сборок</span>
          <span class="switch">
            <input type="checkbox" checked={g.shared} onchange={(e) => share(g.id, e.currentTarget.checked)} />
            <span></span>
          </span>
        </label>
      </header>
      {#if !g.shared}
        <p class="muted small">Сейчас у каждой сборки свои — меняются в игре. {g.hint ?? ""}</p>
      {:else if g.id === "keys"}
        <p class="muted small">
          Клавиши назначаются в самой игре, лаунчер переносит их во все сборки 1.13 и новее.
          {opts.keys ? `Сейчас перенесено назначений: ${opts.keys}.` : "Пока ни одного — появятся после первой игры."}
        </p>
      {:else if g.id === "packs"}
        <p class="muted small">
          Какие ресурспаки включены и в каком порядке — включаются в игре, переносятся во все сборки 1.13 и новее.
          Сами файлы кладите в «Библиотеку» (раздел «Моды»), чтобы они были в каждой сборке.
        </p>
        {#if opts.packs?.length}
          <ul class="packs">
            {#each opts.packs as p (p)}<li>{p.replace(/^file\//, "")}</li>{/each}
          </ul>
        {/if}
      {:else}
        <ul class="rows-plain">
          {#each g.items as s (s.key)}
            <li>
              <div class="what">
                <span>{s.label}{#if s.since}<span class="since">с {s.since}</span>{/if}</span>
                {#if s.hint}<span class="muted small">{s.hint}</span>{/if}
              </div>
              <div class="control">
                {#if s.kind.type === "bool"}
                  <label class="switch">
                    <input
                      type="checkbox"
                      aria-label={s.label}
                      checked={value(s) === "true"}
                      onchange={(e) => set(s, String(e.currentTarget.checked))}
                    />
                    <span></span>
                  </label>
                {:else if s.kind.type === "choice"}
                  <div class="choice">
                    <Select
                      label={s.label}
                      value={value(s)}
                      options={s.kind.options.map(([value, label]) => ({ value, label }))}
                      onchange={(v) => set(s, v)}
                    />
                  </div>
                {:else}
                  <input
                    type="range"
                    aria-label={s.label}
                    min={s.kind.min}
                    max={s.kind.max}
                    step={s.kind.type === "slider" ? s.kind.step : 1}
                    value={shown(s)}
                    oninput={(e) => (e.currentTarget.nextElementSibling!.textContent = label(s, Number(e.currentTarget.value)))}
                    onchange={(e) => set(s, stored(s, Number(e.currentTarget.value)))}
                  />
                  <b class="num">{label(s, shown(s))}</b>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/each}
{/if}

<style>
  .block {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: var(--s3);
    padding: var(--s5);
  }
  .intro p {
    margin: 0;
  }
  .seed {
    display: flex;
    gap: var(--s3);
    align-items: center;
  }
  .grow-select {
    flex: 1;
    max-width: 320px;
  }
  .search {
    align-self: stretch;
    display: flex;
    align-items: center;
    gap: var(--s2);
    height: var(--h-md);
    padding: 0 var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--surface);
    color: var(--text-muted);
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .search input {
    flex: 1;
    border: 0;
    background: none;
    outline: none;
    color: var(--text);
    user-select: text;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-16);
    font-weight: 600;
  }
  .shared {
    display: flex;
    align-items: center;
    gap: var(--s2);
    cursor: pointer;
  }
  .block > p {
    margin: 0;
  }
  .rows-plain {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .rows-plain li {
    display: flex;
    align-items: center;
    gap: var(--s4);
    min-height: 52px;
    padding: var(--s2) 0;
    border-top: 1px solid var(--border);
  }
  .rows-plain li:first-child {
    border-top: 0;
  }
  .what {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .since {
    margin-left: var(--s2);
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--surface-raised);
    color: var(--text-muted);
    font-size: var(--fs-12);
  }
  .control {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--s3);
    width: 320px;
    flex: none;
  }
  .control input[type="range"] {
    flex: 1;
    accent-color: var(--accent);
  }
  .num {
    /* Число — узко, «Без ограничения» — шире: ползунок уступает место. */
    min-width: 64px;
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .choice {
    width: 220px;
  }
  .packs {
    margin: 0;
    padding-left: var(--s5);
  }
</style>
