<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { call, inTauri } from "../lib/tauri";
  import HexInput from "../components/HexInput.svelte";
  import Icon from "../components/Icon.svelte";
  import Logo from "../components/Logo.svelte";
  import { apply, save, theme, type Appearance, type ThemeMode } from "../lib/theme.svelte";

  type Page = "look" | "about";
  let page: Page = $state("look");
  const version = call<string>("core_version");

  // Ссылки — во внешнем браузере, а не внутри окна лаунчера.
  let linkError = $state("");
  async function open(url: string) {
    linkError = "";
    try {
      if (inTauri) await openUrl(url);
      else window.open(url);
    } catch (e) {
      linkError = `Не удалось открыть ссылку: ${e}`;
    }
  }

  const MODES: { id: ThemeMode; title: string; hint: string }[] = [
    { id: "dark", title: "Чёрная", hint: "Тёмный фон, светлый текст" },
    { id: "light", title: "Белая", hint: "Светлый фон, тёмный текст" },
    { id: "system", title: "Как в Windows", hint: "Следует за темой системы" },
  ];

  let error = $state("");

  async function update(patch: Partial<Appearance>) {
    error = "";
    try {
      await save({ ...theme.appearance, ...patch });
    } catch (e) {
      error = String(e);
    }
  }

  // Во время перетаскивания в палитре только показываем цвет, сохраняем по отпусканию.
  function preview(key: "accent" | "background", value: string) {
    apply({ ...theme.appearance, [key]: value });
  }
</script>

<div class="settings">
  <nav class="pages" aria-label="Разделы настроек">
    <button class="page" aria-current={page === "look" ? "page" : undefined} onclick={() => (page = "look")}>
      Внешний вид
    </button>
    <button class="page" aria-current={page === "about" ? "page" : undefined} onclick={() => (page = "about")}>
      О программе
    </button>
    <p class="later muted">Остальные разделы — по мере готовности функций.</p>
  </nav>

  <section class="body">
    {#if page === "look"}
      <h1>Внешний вид</h1>

      <div class="card block">
        <h2>Тема</h2>
        <div class="modes" role="radiogroup" aria-label="Тема">
          {#each MODES as m (m.id)}
            <button
              class="mode"
              role="radio"
              aria-checked={theme.appearance.mode === m.id}
              onclick={() => update({ mode: m.id })}
            >
              <span class="mini {m.id}" aria-hidden="true">
                <i class="mini-nav"></i><i class="mini-card"></i><i class="mini-btn"></i>
              </span>
              <b>{m.title}</b>
              <span class="muted">{m.hint}</span>
            </button>
          {/each}
        </div>
      </div>

      <div class="card block">
        <div class="row">
          <div class="label">
            <h2>Акцентный цвет</h2>
            <p class="muted">Кнопка «Играть», выбранный раздел, переключатели. Можно выбрать в палитре или вставить код.</p>
          </div>
          <label class="swatch" style:background={theme.palette.accent}>
            <input
              type="color"
              value={theme.palette.accent}
              oninput={(e) => preview("accent", e.currentTarget.value)}
              onchange={(e) => update({ accent: e.currentTarget.value })}
              aria-label="Выбрать акцентный цвет"
            />
          </label>
          <HexInput
            label="Код акцентного цвета"
            value={theme.palette.accent}
            onpreview={(hex) => preview("accent", hex)}
            oncommit={(hex) => update({ accent: hex })}
          />
          <button class="btn btn-sm" disabled={!theme.appearance.accent} onclick={() => update({ accent: null })}>
            Как в теме
          </button>
        </div>

        <div class="row">
          <div class="label">
            <h2>Цвет фона</h2>
            <p class="muted">Панели, рамки и текст подстроятся под него сами.</p>
          </div>
          <label class="swatch" style:background={theme.palette.bg}>
            <input
              type="color"
              value={theme.palette.bg}
              oninput={(e) => preview("background", e.currentTarget.value)}
              onchange={(e) => update({ background: e.currentTarget.value })}
              aria-label="Выбрать цвет фона"
            />
          </label>
          <HexInput
            label="Код цвета фона"
            value={theme.palette.bg}
            onpreview={(hex) => preview("background", hex)}
            oncommit={(hex) => update({ background: hex })}
          />
          <button
            class="btn btn-sm"
            disabled={!theme.appearance.background}
            onclick={() => update({ background: null })}
          >
            Как в теме
          </button>
        </div>

        {#each theme.warnings as w}
          <p class="warn"><Icon name="warning" size={18} />{w}</p>
        {/each}
        {#if error}<p class="warn"><Icon name="error" size={18} />{error}</p>{/if}
      </div>

      <button
        class="btn"
        disabled={!theme.appearance.accent && !theme.appearance.background}
        onclick={() => update({ accent: null, background: null })}
      >
        <Icon name="restore" size={18} /> Сбросить свои цвета
      </button>
    {:else}
      <h1>О программе</h1>
      <div class="card block about">
        <Logo size={64} />
        <div>
          <h2>MajorLauncher</h2>
          {#await version then v}<p class="muted">Версия {v}</p>{/await}
          <p class="muted">Открытый код, лицензия GPL-3.0. Без рекламы и телеметрии.</p>
        </div>
      </div>
      <div class="card block">
        <button class="btn" onclick={() => open("https://github.com/Majorzxc/MajorLauncher")}>
          <Icon name="link" size={18} /> Исходный код на GitHub
        </button>
        {#if linkError}<p class="warn"><Icon name="error" size={18} />{linkError}</p>{/if}
        <p class="muted credits">
          Иконки — Lucide (ISC). Шрифт — Onest (SIL Open Font License).
        </p>
      </div>
    {/if}
  </section>
</div>

<style>
  .settings {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: var(--s6);
    align-items: start;
  }
  .pages {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    position: sticky;
    top: 0;
  }
  .page {
    height: var(--h-md);
    padding: 0 var(--s3);
    border: 0;
    border-radius: var(--r);
    background: none;
    text-align: left;
    color: var(--text-muted);
    cursor: pointer;
  }
  .page:hover {
    background: var(--surface-raised);
    color: var(--text);
  }
  .page[aria-current="page"] {
    background: var(--surface-raised);
    color: var(--text);
    font-weight: 600;
  }
  .later {
    margin: var(--s3) var(--s3) 0;
    font-size: var(--fs-12);
    line-height: 16px;
  }
  .body {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--s4);
    max-width: 760px;
  }
  h1 {
    margin: 0 0 var(--s2);
    font-size: var(--fs-24);
    font-weight: 600;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-16);
    font-weight: 600;
  }
  .block {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: var(--s4);
    padding: var(--s5);
  }

  .modes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--s3);
  }
  .mode {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--bg);
    text-align: left;
    cursor: pointer;
  }
  .mode:hover {
    border-color: var(--border-strong);
  }
  .mode[aria-checked="true"] {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .mode b {
    margin-top: var(--s2);
    font-weight: 600;
  }
  .mode .muted {
    font-size: var(--fs-14);
  }

  /* Мини-окно для превью темы: фиксированные цвета, не зависят от текущей темы. */
  .mini {
    position: relative;
    width: 100%;
    height: 72px;
    border-radius: var(--r-sm);
    overflow: hidden;
    border: 1px solid #00000022;
  }
  .mini i {
    position: absolute;
    display: block;
    border-radius: 3px;
  }
  .mini-nav {
    left: 0;
    top: 0;
    bottom: 0;
    width: 14px;
    border-radius: 0 !important;
  }
  .mini-card {
    left: 24px;
    right: 10px;
    top: 10px;
    height: 30px;
  }
  .mini-btn {
    left: 24px;
    width: 46px;
    bottom: 10px;
    height: 12px;
  }
  .mini.dark {
    background: #0e0e10;
  }
  .mini.dark .mini-nav,
  .mini.dark .mini-card {
    background: #1d1d20;
  }
  .mini.dark .mini-btn {
    background: #ededef;
  }
  .mini.light {
    background: #f4f4f5;
  }
  .mini.light .mini-nav,
  .mini.light .mini-card {
    background: #ffffff;
  }
  .mini.light .mini-btn {
    background: #121214;
  }
  .mini.system {
    background: linear-gradient(90deg, #0e0e10 50%, #f4f4f5 50%);
  }
  .mini.system .mini-nav {
    background: #1d1d20;
  }
  .mini.system .mini-card {
    background: linear-gradient(90deg, #1d1d20 50%, #ffffff 50%);
  }
  .mini.system .mini-btn {
    background: #ededef;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--s3);
  }
  .label {
    flex: 1;
  }
  .label p {
    margin: 0;
    font-size: var(--fs-14);
  }
  .swatch {
    position: relative;
    width: 40px;
    height: 40px;
    border-radius: var(--r);
    border: 1px solid var(--border-strong);
    cursor: pointer;
  }
  .swatch input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: default;
    transform: none;
  }
  .warn {
    display: flex;
    align-items: flex-start;
    gap: var(--s2);
    margin: 0;
    font-size: var(--fs-14);
    color: var(--warning);
  }
  .about {
    flex-direction: row;
    align-items: center;
  }
  .about p {
    margin: 0;
  }
  .credits {
    margin: 0;
    font-size: var(--fs-14);
  }
</style>
