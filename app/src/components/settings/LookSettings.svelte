<script lang="ts">
  // «Настройки → Внешний вид» (раздел 6.0 ТЗ): тема, свои цвета, картинка фона.
  import { open as openFile } from "@tauri-apps/plugin-dialog";
  import { api } from "../../lib/api";
  import { inTauri } from "../../lib/tauri";
  import { apply, loadImage, save, theme, type Appearance } from "../../lib/theme.svelte";
  import HexInput from "../HexInput.svelte";
  import Icon from "../Icon.svelte";
  import ThemeModes from "../ThemeModes.svelte";

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
</script>

<div class="card block">
  <h2>Тема</h2>
  <ThemeModes onpick={(mode) => update({ mode })} />
</div>

<div class="card block">
  {#snippet color(key: "accent" | "background", title: string, hint: string)}
    {@const value = key === "accent" ? theme.palette.accent : theme.palette.bg}
    <div class="row">
      <div class="label">
        <h2>{title}</h2>
        <p class="muted">{hint}</p>
      </div>
      <label class="swatch" style:background={value}>
        <input
          type="color"
          {value}
          oninput={(e) => preview(key, e.currentTarget.value)}
          onchange={(e) => update({ [key]: e.currentTarget.value })}
          aria-label="Выбрать: {title.toLowerCase()}"
        />
      </label>
      <HexInput
        label="Код: {title.toLowerCase()}"
        {value}
        onpreview={(hex) => preview(key, hex)}
        oncommit={(hex) => update({ [key]: hex })}
      />
      <button class="btn btn-sm" disabled={!theme.appearance[key]} onclick={() => update({ [key]: null })}>Как в теме</button>
    </div>
  {/snippet}
  {@render color("accent", "Акцентный цвет", "Кнопка «Играть», выбранный раздел, переключатели. Можно выбрать в палитре или вставить код.")}
  {@render color("background", "Цвет фона", "Панели, рамки и текст подстроятся под него сами.")}
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

<style>
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
    flex: none;
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
</style>
