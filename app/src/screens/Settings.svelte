<script lang="ts">
  import OptimizeSettings from "../components/OptimizeSettings.svelte";
  import UpdateSettings from "../components/UpdateSettings.svelte";
  import AiSettings from "../components/AiSettings.svelte";
  import { open as openFile } from "@tauri-apps/plugin-dialog";
  import { call, inTauri } from "../lib/tauri";
  import { api } from "../lib/api";
  import { openExternal } from "../lib/links";
  import { size } from "../lib/format";
  import DataMove from "../components/DataMove.svelte";
  import GameOptions from "../components/GameOptions.svelte";
  import GameProfile from "../components/GameProfile.svelte";
  import { settingsPage } from "../lib/nav.svelte";
  import { restartTutorial, stopTutorial, tutorial } from "../lib/tutorial.svelte";
  import { catalog, setAutoDeps, setTranslate } from "../lib/catalog.svelte";
  import HexInput from "../components/HexInput.svelte";
  import Icon from "../components/Icon.svelte";
  import Logo from "../components/Logo.svelte";
  import ThemeModes from "../components/ThemeModes.svelte";
  import { apply, loadImage, save, theme, type Appearance } from "../lib/theme.svelte";

  type Page = "profile" | "look" | "game" | "catalog" | "optimize" | "updates" | "ai" | "data" | "about";
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
      await openExternal(url);
    } catch (e) {
      linkError = `Не удалось открыть ссылку: ${e}`;
    }
  }

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
  <nav class="pages" aria-label="Разделы настроек" data-tour="settings-pages">
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
    <button class="page" aria-current={page === "updates" ? "page" : undefined} onclick={() => (page = "updates")}>
      Обновления
    </button>
    <button class="page" aria-current={page === "ai" ? "page" : undefined} onclick={() => (page = "ai")}>
      ИИ-разбор
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
        <ThemeModes onpick={(mode) => update({ mode })} />
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
    {:else if page === "updates"}
      <h1>Обновления</h1>
      <UpdateSettings />
    {:else if page === "ai"}
      <h1>ИИ-разбор ошибок</h1>
      <AiSettings />
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
      <div class="card block">
        <h2>Обучение</h2>
        <p class="muted">
          Короткая экскурсия по главному экрану, а в каждом разделе — пара подсказок при первом открытии.
          {tutorial.state.enabled ? "Сейчас подсказки разделов включены." : "Сейчас подсказки разделов выключены."}
        </p>
        <div class="row-buttons">
          <button class="btn" onclick={restartTutorial}><Icon name="info" size={18} />Пройти обучение заново</button>
          {#if tutorial.state.enabled}
            <button class="btn btn-quiet" onclick={stopTutorial}>Выключить подсказки</button>
          {/if}
        </div>
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
  .row-buttons {
    display: flex;
    gap: var(--s2);
  }
  .credits {
    margin: 0;
    font-size: var(--fs-14);
  }
</style>
