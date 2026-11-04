<script lang="ts">
  // Первый запуск (раздел 6.4 ТЗ): оформление, папка данных, аккаунт, импорт
  // из других лаунчеров (если нашлись), предложение обучения. Тот же экран
  // без шагов — «Папка данных не найдена» и выбор папки, если её забыли.
  import { untrack } from "svelte";
  import ElyForm from "../components/accounts/ElyForm.svelte";
  import NickForm from "../components/accounts/NickForm.svelte";
  import Icon from "../components/Icon.svelte";
  import ImportLauncher from "../components/ImportLauncher.svelte";
  import Logo from "../components/Logo.svelte";
  import DataDirStep from "../components/onboarding/DataDirStep.svelte";
  import ThemeModes from "../components/ThemeModes.svelte";
  import { KIND, accounts, loadAccounts } from "../lib/accounts.svelte";
  import { api, type DataStatus, type ImportFound } from "../lib/api";
  import { count, plural } from "../lib/format";
  import { save, theme, type Appearance } from "../lib/theme.svelte";
  import { finishOnboarding } from "../lib/tutorial.svelte";

  type Step = "look" | "data" | "account" | "import" | "tour";

  let {
    mode,
    missing = null,
    dataDir = null,
    onmoved,
    ondone,
  }: {
    /// `wizard` — весь мастер; `data` — только папка данных (её нет или она пропала).
    mode: "wizard" | "data";
    missing?: string | null;
    /// Папка уже выбрана: окно закрыли посреди мастера — продолжаем с аккаунта.
    dataDir?: string | null;
    onmoved?: (s: DataStatus) => void;
    ondone: () => void;
  } = $props();

  const TITLES: Record<Step, string> = {
    look: "Оформление",
    data: "Папка данных",
    account: "Аккаунт",
    import: "Импорт",
    tour: "Обучение",
  };

  let folder = $state(untrack(() => dataDir));
  let step = $state<Step>(untrack(() => (mode === "data" ? "data" : dataDir ? "account" : "look")));

  // Сборки других лаунчеров ищутся, пока игрок выбирает аккаунт. Не нашлось — шаг пропускается.
  let found = $state<ImportFound[] | null>(null);
  function scan() {
    found = null;
    api
      .importScan()
      .then((l) => (found = l.filter((f) => !f.problem)))
      .catch(() => (found = []));
  }
  if (untrack(() => dataDir)) {
    loadAccounts();
    scan();
  }

  const steps = $derived<Step[]>(
    mode === "data" ? ["data"] : (["look", "data", "account", "import", "tour"] as Step[]).filter((s) => s !== "import" || found?.length !== 0),
  );
  const at = $derived(steps.indexOf(step));

  function forward() {
    step = steps[at + 1] ?? step;
  }
  function backward() {
    step = steps[at - 1] ?? step;
  }

  // Пока игрок на шаге «Импорт», поиск закончился ничем — сразу дальше.
  $effect(() => {
    if (step === "import" && found?.length === 0) step = "tour";
  });

  async function dataChosen() {
    if (mode === "data") return ondone();
    const c = await api.config();
    folder = c.data_dir;
    loadAccounts();
    scan();
    forward();
  }

  // Оформление сохраняется сразу — окно перекрашивается на глазах.
  let lookError = $state("");
  async function look(patch: Partial<Appearance>) {
    lookError = "";
    try {
      await save({ ...theme.appearance, ...patch });
    } catch (e) {
      lookError = String(e);
    }
  }
  const ACCENTS = [
    { hex: "#7c3aed", name: "Фиолетовый" },
    { hex: "#2563eb", name: "Синий" },
    { hex: "#0d9488", name: "Бирюзовый" },
    { hex: "#16a34a", name: "Зелёный" },
    { hex: "#ea580c", name: "Оранжевый" },
    { hex: "#e11d48", name: "Малиновый" },
  ];

  let accountKind = $state<"nick" | "ely">("nick");

  let importing = $state(false);
  let imported = $state(0);
  const sources = $derived.by(() => {
    const by = new Map<string, number>();
    for (const f of found ?? []) by.set(f.source_title, (by.get(f.source_title) ?? 0) + 1);
    return [...by];
  });
  const buildsWord = (n: number) => `${count(n)} ${plural(n, ["сборка", "сборки", "сборок"])}`;

  let finishing = $state(false);
  async function finish(tour: boolean) {
    finishing = true;
    await finishOnboarding(tour);
    ondone();
  }
</script>

<div class="first">
  <div class="box card">
    <div class="top">
      <Logo size={40} />
      {#if steps.length > 1}
        <ol class="steps" aria-label="Шаги">
          {#each steps as s, i (s)}
            <li class:done={i < at} class:now={i === at} aria-current={i === at ? "step" : undefined}>
              <span class="num">{#if i < at}<Icon name="check" size={14} />{:else}{i + 1}{/if}</span>{TITLES[s]}
            </li>
          {/each}
        </ol>
      {/if}
    </div>

    {#if step === "look"}
      <h1>Добро пожаловать в MajorLauncher</h1>
      <p class="muted">Пара шагов — и можно играть. Всё это потом меняется в настройках.</p>
      <h2>Тема</h2>
      <ThemeModes onpick={(m) => look({ mode: m })} />
      <h2>Акцентный цвет</h2>
      <div class="accents" role="radiogroup" aria-label="Акцентный цвет">
        <button
          class="accent plain"
          role="radio"
          aria-checked={!theme.appearance.accent}
          title="Как в теме: чёрно-белый"
          aria-label="Как в теме"
          onclick={() => look({ accent: null })}
        ></button>
        {#each ACCENTS as a (a.hex)}
          <button
            class="accent"
            role="radio"
            aria-checked={theme.appearance.accent === a.hex}
            title={a.name}
            aria-label={a.name}
            style:background={a.hex}
            onclick={() => look({ accent: a.hex })}
          ></button>
        {/each}
      </div>
      <p class="muted small">Свой цвет, цвет фона и картинка — в «Настройки → Внешний вид».</p>
      {#if lookError}<p class="warn"><Icon name="error" size={18} />{lookError}</p>{/if}
      <div class="buttons">
        <button class="btn btn-primary" onclick={forward}>Далее<Icon name="chevron-right" size={18} /></button>
      </div>
    {:else if step === "data"}
      <DataDirStep
        {missing}
        current={folder}
        {onmoved}
        onback={mode === "wizard" ? backward : undefined}
        ondone={dataChosen}
      />
    {:else if step === "account"}
      <h1>Под каким ником играть?</h1>
      {#if accounts.list.length}
        <p class="ok small">
          <Icon name="check" size={18} />
          <span>
            Уже есть:
            {#each accounts.list as a, i (a.id)}{i ? ", " : ""}<b>{a.name}</b> <span class="muted">({KIND[a.kind]})</span>{/each}.
            Можно добавить ещё или идти дальше.
          </span>
        </p>
      {:else}
        <p class="muted">Офлайн-ник — сразу и без пароля. Ely.by — свой скин, который видят другие игроки.</p>
      {/if}
      <div class="chips" role="tablist" aria-label="Вид аккаунта">
        <button class="chip" role="tab" aria-selected={accountKind === "nick"} class:on={accountKind === "nick"} onclick={() => (accountKind = "nick")}>
          Офлайн-ник
        </button>
        <button class="chip" role="tab" aria-selected={accountKind === "ely"} class:on={accountKind === "ely"} onclick={() => (accountKind = "ely")}>
          Ely.by
        </button>
      </div>
      <div class="form">
        {#key accountKind}
          {#if accountKind === "nick"}<NickForm ondone={forward} />{:else}<ElyForm ondone={forward} />{/if}
        {/key}
      </div>
      <div class="buttons">
        <button class="btn btn-quiet back" onclick={backward}>Назад</button>
        <button class="btn" onclick={forward}>
          {accounts.list.length ? "Далее" : "Пропустить — добавлю потом"}<Icon name="chevron-right" size={18} />
        </button>
      </div>
    {:else if step === "import"}
      <h1>Перенести сборки из других лаунчеров?</h1>
      {#if found === null}
        <p class="muted"><span class="spin"></span>Ищу сборки других лаунчеров…</p>
      {:else}
        <p class="muted">Нашлись сборки — их можно забрать сюда. Файлы того лаунчера только копируются, в нём ничего не изменится.</p>
        <ul class="sources">
          {#each sources as [title, n] (title)}<li><b>{title}</b> <span class="muted">— {buildsWord(n)}</span></li>{/each}
        </ul>
        {#if imported}<p class="ok small"><Icon name="check" size={18} />Перенесено: {buildsWord(imported)}.</p>{/if}
        <div>
          <button class="btn" class:btn-primary={!imported} onclick={() => (importing = true)}>
            <Icon name="import" size={18} />{imported ? "Перенести ещё…" : "Выбрать и перенести…"}
          </button>
        </div>
      {/if}
      <div class="buttons">
        <button class="btn btn-quiet back" onclick={backward}>Назад</button>
        <button class="btn" class:btn-primary={imported > 0 || !found?.length} onclick={forward}>
          {imported ? "Далее" : "Пропустить"}<Icon name="chevron-right" size={18} />
        </button>
      </div>
    {:else if step === "tour"}
      <h1>Показать, что где?</h1>
      <p class="muted">
        Короткая экскурсия по главному экрану — несколько подсказок с подсветкой, меньше минуты. В остальных разделах
        подсказки появятся при первом открытии, их можно выключить в любой момент.
      </p>
      <p class="muted small">Если откажетесь, кнопка «Пройти обучение» неделю побудет в шапке, потом — в «Настройки → О программе».</p>
      <div class="buttons">
        <button class="btn btn-quiet back" disabled={finishing} onclick={backward}>Назад</button>
        <button class="btn" disabled={finishing} onclick={() => finish(false)}>Нет, спасибо</button>
        <button class="btn btn-primary" disabled={finishing} onclick={() => finish(true)}>Показать</button>
      </div>
    {/if}
  </div>
</div>

{#if importing}
  <ImportLauncher onclose={() => (importing = false)} onimported={() => imported++} />
{/if}

<style>
  .first {
    min-height: 100%;
    display: grid;
    place-items: center;
    padding: var(--s6);
  }
  .box {
    width: 680px;
    display: flex;
    flex-direction: column;
    gap: var(--s4);
    padding: var(--s6);
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--s4);
    margin-bottom: var(--s2);
  }
  .steps {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s1) var(--s3);
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-14);
    color: var(--text-muted);
  }
  .steps li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .steps .now {
    color: var(--text);
    font-weight: 600;
  }
  .num {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 1px solid var(--border-strong);
    font-size: var(--fs-12);
  }
  .now .num {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--on-accent);
  }
  .done .num {
    border-color: var(--accent);
    color: var(--accent);
  }
  .box :global(h1) {
    margin: 0;
    font-size: var(--fs-24);
    font-weight: 600;
  }
  .box :global(p) {
    margin: 0;
  }
  h2 {
    margin: var(--s2) 0 0;
    font-size: var(--fs-16);
    font-weight: 600;
  }
  .accents {
    display: flex;
    gap: var(--s2);
  }
  .accent {
    width: 36px;
    height: 36px;
    border: 1px solid var(--border-strong);
    border-radius: var(--r);
    cursor: pointer;
  }
  .accent.plain {
    background: linear-gradient(135deg, #0e0e10 50%, #f4f4f5 50%);
  }
  .accent[aria-checked="true"] {
    outline: 2px solid var(--text);
    outline-offset: 2px;
  }
  .ok {
    display: flex;
    align-items: flex-start;
    gap: var(--s2);
  }
  .ok :global(svg) {
    flex: none;
    color: var(--accent);
  }
  .form {
    padding: var(--s4);
    border: 1px solid var(--border);
    border-radius: var(--r-card);
  }
  .sources {
    margin: 0;
    padding-left: var(--s5);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: var(--s2);
    margin-top: var(--s2);
  }
  .back {
    margin-right: auto;
  }
  .buttons :disabled {
    opacity: 0.45;
    cursor: default;
  }
  .spin {
    display: inline-block;
    width: 12px;
    height: 12px;
    margin-right: 8px;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    vertical-align: -1px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
