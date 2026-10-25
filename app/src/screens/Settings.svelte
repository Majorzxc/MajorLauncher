<script lang="ts">
  import OptimizeSettings from "../components/OptimizeSettings.svelte";
  import { open as openFile } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { call, inTauri } from "../lib/tauri";
  import { api } from "../lib/api";
  import { size } from "../lib/format";
  import DataMove from "../components/DataMove.svelte";
  import GameOptions from "../components/GameOptions.svelte";
  import GameProfile from "../components/GameProfile.svelte";
  import { settingsPage } from "../lib/nav.svelte";
  import { catalog, setAutoDeps, setTranslate } from "../lib/catalog.svelte";
  import HexInput from "../components/HexInput.svelte";
  import Icon from "../components/Icon.svelte";
  import Logo from "../components/Logo.svelte";
  import { apply, loadImage, save, theme, type Appearance, type ThemeMode } from "../lib/theme.svelte";

  type Page = "profile" | "look" | "game" | "catalog" | "optimize" | "data" | "about";
  // Страница настроек из адреса (#settings/profile) — чтобы открыть «Настройки игры» по ссылке.
  let page: Page = $state(settingsPage.value ?? "look");
  settingsPage.value = null;
  const version = call<string>("core_version");
  const inspectData = () => api.config().then((c) => (c.data_dir ? api.inspectDir(c.data_dir) : null));
  let dataDir = $state(inspectData());

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

  // Картинка фона: ядро уменьшает её под экран (с учётом масштаба Windows).
  let picking = $state(false);
  async function pickImage() {
    if (!inTauri) return;
    const path = await openFile({
      multiple: false,
      directory: false,
      title: "Картинка фона",
      filters: [{ name: "Картинки", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
    });
    if (typeof path !== "string") return;
    picking = true;
    error = "";
    try {
      const dpr = window.devicePixelRatio || 1;
      const c = await api.setBackgroundImage(path, Math.round(screen.width * dpr), Math.round(screen.height * dpr));
      apply(c.appearance);
      await loadImage();
    } catch (e) {
      error = String(e);
    } finally {
      picking = false;
    }
  }

  async function removeImage() {
    error = "";
    try {
      apply((await api.removeBackgroundImage()).appearance);
      await loadImage();
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
    <button class="page" aria-current={page === "profile" ? "page" : undefined} onclick={() => (page = "profile")}>
      Настройки игры
    </button>
    <button class="page" aria-current={page === "look" ? "page" : undefined} onclick={() => (page = "look")}>
      Внешний вид
    </button>
    <button class="page" aria-current={page === "game" ? "page" : undefined} onclick={() => (page = "game")}>
      Запуск игры
    </button>
    <button class="page" aria-current={page === "catalog" ? "page" : undefined} onclick={() => (page = "catalog")}>
      Каталог Modrinth
    </button>
    <button class="page" aria-current={page === "optimize" ? "page" : undefined} onclick={() => (page = "optimize")}>
      Оптимизация
    </button>
    <button class="page" aria-current={page === "data" ? "page" : undefined} onclick={() => (page = "data")}>
      Папка данных
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

      <div class="card block">
        <div class="row">
          <div class="label">
            <h2>Картинка фона</h2>
            <p class="muted">Статичная, за содержимым окна. Лаунчер уменьшит её под ваш экран.</p>
          </div>
          {#if theme.image}<img class="bg-thumb" src={theme.image} alt="Текущая картинка фона" />{/if}
          <button class="btn btn-sm" disabled={picking} onclick={pickImage}>
            {picking ? "Готовим…" : theme.appearance.image ? "Другая картинка" : "Выбрать картинку"}
          </button>
          {#if theme.appearance.image}
            <button class="btn btn-sm" onclick={removeImage}>Убрать</button>
          {/if}
        </div>
        {#if theme.appearance.image}
          <div class="row">
            <div class="label">
              <h2>{theme.palette.dark ? "Затемнение" : "Осветление"}</h2>
              <p class="muted">Чем сильнее, тем лучше читается текст поверх картинки.</p>
            </div>
            <input
              class="dim"
              type="range"
              min="0"
              max="90"
              step="5"
              value={theme.appearance.image_dim}
              oninput={(e) => apply({ ...theme.appearance, image_dim: Number(e.currentTarget.value) })}
              onchange={(e) => update({ image_dim: Number(e.currentTarget.value) })}
              aria-label="Насколько приглушить картинку"
            />
            <b class="pct">{theme.appearance.image_dim}%</b>
          </div>
          {#if theme.appearance.image_dim < 40}
            <p class="warn"><Icon name="warning" size={18} />Текст поверх яркой картинки может читаться плохо.</p>
          {/if}
        {/if}
      </div>

      <button
        class="btn"
        disabled={!theme.appearance.accent && !theme.appearance.background}
        onclick={() => update({ accent: null, background: null })}
      >
        <Icon name="restore" size={18} /> Сбросить свои цвета
      </button>
    {:else if page === "profile"}
      <h1>Настройки игры</h1>
      <GameProfile />
    {:else if page === "game"}
      <h1>Запуск игры</h1>
      <GameOptions />
    {:else if page === "catalog"}
      <h1>Каталог Modrinth</h1>
      <div class="card block">
        <label class="opt">
          <input type="checkbox" checked={catalog.translate} onchange={(e) => setTranslate(e.currentTarget.checked)} />
          <b>Переводить описания модов на русский</b>
        </label>
        <p class="muted">
          Описания на странице мода и краткие описания в выдаче будут показываться по-русски, рядом — «Показать оригинал».
        </p>
        <p class="muted small">
          Перевод делает Google: текст описания мода отправляется на translate.googleapis.com. Ваши данные — ник, аккаунты,
          сборки, файлы — не отправляются, только текст описания. Это единственное место, где лаунчер что-то передаёт чужому
          сервису, поэтому по умолчанию перевод выключен. Готовые переводы хранятся на компьютере и повторно не запрашиваются.
        </p>
      </div>
      <div class="card block">
        <label class="opt">
          <input type="checkbox" checked={catalog.autoDeps} onchange={(e) => setAutoDeps(e.currentTarget.checked)} />
          <b>Ставить нужные зависимости без окна подтверждения</b>
        </label>
        <p class="muted">Окно всё равно появится, если чего-то не хватает или есть несовместимость с уже стоящими модами.</p>
      </div>
    {:else if page === "optimize"}
      <h1>Оптимизация</h1>
      <OptimizeSettings />
    {:else if page === "data"}
      <h1>Папка данных</h1>
      <div class="card block">
        <p class="muted">Здесь лежат версии игры, Java, сборки с мирами и модами, аккаунты (без паролей).</p>
        {#await dataDir then d}
          {#if d}
            <code class="path">{d.path}</code>
            {#if d.free_bytes !== null}<p class="muted">Свободно на диске: {size(d.free_bytes)}</p>{/if}
            {#each d.warnings as w}<p class="warn"><Icon name="warning" size={18} />{w}</p>{/each}
          {/if}
        {/await}
        <div>
          <button class="btn" onclick={() => api.openDataDir().catch((e) => (linkError = String(e)))}>
            <Icon name="folder" size={18} /> Открыть в проводнике
          </button>
        </div>
        {#if linkError}<p class="warn"><Icon name="error" size={18} />{linkError}</p>{/if}
      </div>
      <DataMove onmoved={() => (dataDir = inspectData())} />
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
  .opt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    cursor: pointer;
  }
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
  .bg-thumb {
    width: 96px;
    height: 54px;
    object-fit: cover;
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
  }
  .dim {
    width: 240px;
    accent-color: var(--accent);
  }
  .pct {
    min-width: 44px;
    text-align: right;
    font-variant-numeric: tabular-nums;
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
  .path {
    padding: var(--s2) var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--bg);
    font-family: Consolas, monospace;
    user-select: text;
  }
  .block > p {
    margin: 0;
  }
  .small {
    font-size: var(--fs-12);
  }
  .credits {
    margin: 0;
    font-size: var(--fs-14);
  }
</style>
