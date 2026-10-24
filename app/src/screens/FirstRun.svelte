<script lang="ts">
  import { untrack } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import Icon from "../components/Icon.svelte";
  import Logo from "../components/Logo.svelte";
  import { api, type DataStatus, type DirCheck } from "../lib/api";
  import { size } from "../lib/format";
  import { inTauri } from "../lib/tauri";

  // `missing` — папка данных была выбрана, но пропала (диск отключён или сменил букву).
  let {
    ondone,
    missing = null,
    onmoved,
  }: { ondone: () => void; missing?: string | null; onmoved?: (s: DataStatus) => void } = $props();

  let path = $state("");
  let check: DirCheck | null = $state(null);
  let checking = $state(true);
  let error = $state("");
  let timer: ReturnType<typeof setTimeout>;

  // Пропавшую папку предлагаем по старому пути: игрок поправит только букву.
  const lost = untrack(() => missing);
  (lost ? api.inspectDir(lost) : api.dataDirSuggestion())
    .then((c) => {
      path = c.path;
      check = c;
    })
    .catch((e) => (error = `Не удалось предложить папку: ${e}`))
    .finally(() => (checking = false));

  // Проверяем путь, пока игрок печатает, — с паузой, чтобы не дёргать диск на каждую букву.
  function typed() {
    checking = true;
    clearTimeout(timer);
    timer = setTimeout(async () => {
      check = path.trim() ? await api.inspectDir(path.trim()) : null;
      checking = false;
    }, 350);
  }

  async function choose() {
    if (!inTauri) return;
    const picked = await open({ directory: true, title: "Папка для игры и сборок", defaultPath: path });
    if (typeof picked === "string") {
      path = picked;
      typed();
    }
  }

  /// Диск подключили — лаунчер ищет папку снова (и на других буквах).
  let rechecking = $state(false);
  async function recheck() {
    error = "";
    rechecking = true;
    try {
      const status = await api.dataDirCheck();
      if (status.missing) {
        error = "Папки всё ещё нет ни на одном диске.";
        return;
      }
      onmoved?.(status);
      ondone();
    } catch (e) {
      error = String(e);
    } finally {
      rechecking = false;
    }
  }

  async function next() {
    error = "";
    try {
      await api.setDataDir(path.trim());
      ondone();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="first">
  <div class="card box">
    <Logo size={56} />
    {#if missing}
      <h1>Папка данных не найдена</h1>
      <p class="muted">
        Лаунчер не нашёл <b class="path">{missing}</b> — ни по этому пути, ни на других дисках. Если папка на
        внешнем диске, подключите его и нажмите «Проверить снова». Если папку переместили, укажите, где она
        теперь.
      </p>
    {:else}
      <h1>Где хранить игру?</h1>
      <p class="muted">
        В этой папке будут версии игры, Java и все сборки с мирами и модами. Понадобится несколько гигабайт.
        Если папка уже использовалась MajorLauncher, лаунчер продолжит с её данными.
      </p>
    {/if}

    <div class="pick">
      <input bind:value={path} oninput={typed} aria-label="Папка данных" spellcheck="false" />
      <button class="btn" onclick={choose}><Icon name="folder" size={18} />Выбрать…</button>
    </div>

    <div class="status">
      {#if checking}
        <p class="muted">Проверяю папку…</p>
      {:else if check}
        {#if !check.writable && check.warnings.length === 0}
          <p class="bad"><Icon name="error" size={18} />Сюда нельзя записывать — выберите другую папку.</p>
        {:else if !check.writable}
          <!-- причина — в предупреждениях ниже -->
        {:else if missing && !check.has_data}
          <p class="warn">
            <Icon name="warning" size={18} />Здесь нет данных MajorLauncher: лаунчер начнёт с пустой папки, а прежние
            сборки и миры останутся там, где были.
          </p>
        {:else}
          <p class="ok">
            <Icon name="check" size={18} />
            {check.has_data ? "Здесь уже есть данные MajorLauncher — продолжим с ними." : "Папка подходит."}
            {#if check.free_bytes !== null}<span class="muted">Свободно на диске: {size(check.free_bytes)}</span>{/if}
          </p>
        {/if}
        {#each check.warnings as w}<p class="warn"><Icon name="warning" size={18} />{w}</p>{/each}
      {/if}
      {#if error}<p class="bad"><Icon name="error" size={18} />{error}</p>{/if}
    </div>

    <div class="buttons">
      {#if missing}
        <button class="btn" disabled={rechecking} onclick={recheck}>
          <Icon name="refresh" size={18} />{rechecking ? "Ищу…" : "Проверить снова"}
        </button>
      {/if}
      <button class="btn btn-primary" disabled={checking || !check?.writable} onclick={next}>
        Продолжить<Icon name="chevron-right" size={18} />
      </button>
    </div>
  </div>
</div>

<style>
  .first {
    height: 100%;
    display: grid;
    place-items: center;
  }
  .box {
    width: 640px;
    display: flex;
    flex-direction: column;
    gap: var(--s4);
    padding: var(--s6);
  }
  h1 {
    margin: var(--s2) 0 0;
    font-size: var(--fs-24);
    font-weight: 600;
  }
  .box > p {
    margin: 0;
  }
  .pick {
    display: flex;
    gap: var(--s2);
  }
  input {
    flex: 1;
    height: var(--h-md);
    padding: 0 var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--bg);
    outline: none;
    font-family: Consolas, monospace;
    user-select: text;
  }
  input:focus {
    border-color: var(--accent);
  }
  .status {
    min-height: 64px;
    display: flex;
    flex-direction: column;
    gap: var(--s2);
    font-size: var(--fs-14);
  }
  .status p {
    display: flex;
    align-items: flex-start;
    flex-wrap: wrap;
    gap: var(--s2);
    margin: 0;
  }
  .warn {
    color: var(--warning);
  }
  .bad {
    color: var(--danger);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: var(--s2);
  }
  .buttons :disabled {
    opacity: 0.45;
    cursor: default;
  }
  .path {
    font-family: Consolas, monospace;
    font-weight: 400;
    color: var(--text);
  }
</style>
