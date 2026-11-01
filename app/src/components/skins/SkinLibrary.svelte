<script lang="ts">
  // «Мои скины»: скин аккаунта, добавление (файл, ник, ссылка) и библиотека.
  import { open as openFile } from "@tauri-apps/plugin-dialog";
  import { api, type NickSource, type Textures } from "../../lib/api";
  import { currentAccount } from "../../lib/accounts.svelte";
  import { loadCape, loadSkins, loadWorn, skins } from "../../lib/skins.svelte";
  import SkinLoaderBuilds from "./SkinLoaderBuilds.svelte";
  import { inTauri } from "../../lib/tauri";
  import { attempt, toast } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";
  import SkinFigure from "../SkinFigure.svelte";

  const account = $derived(currentAccount());

  // Скин аккаунта Ely.by — как его видят другие игроки.
  let accountSkin = $state<Textures | null>(null);
  let accountState = $state<"loading" | "none" | "ok" | "error">("loading");
  $effect(() => {
    const a = account;
    accountSkin = null;
    if (a?.kind !== "ely") return;
    accountState = "loading";
    api
      .skinLookup(a.name, "ely")
      .then((t) => {
        accountSkin = t;
        accountState = "ok";
      })
      .catch((e) => (accountState = String(e).includes("нет скина") ? "none" : "error"));
  });

  // Офлайн-ник: какой скин на нём надет.
  $effect(() => {
    loadWorn(account?.kind === "offline" ? account.id : null);
  });
  const wornSkin = $derived(skins.list.find((s) => s.id === skins.worn) ?? null);

  /// Плащ офлайн-ника — из файла PNG.
  async function capeFromFile() {
    if (!inTauri || account?.kind !== "offline") return;
    const acc = account;
    const path = await openFile({ multiple: false, directory: false, title: "Плащ (PNG 64×32)", filters: [{ name: "PNG", extensions: ["png"] }] });
    if (typeof path !== "string") return;
    const done = await attempt("Плащ", () => api.skinCapeSet(acc.id, path, null).then(() => true));
    if (done) {
      await loadCape(acc.id);
      toast(`Плащ надет на ${acc.name}`);
    }
  }

  async function capeOff() {
    if (account?.kind !== "offline") return;
    const acc = account;
    await attempt("Плащ", () => api.skinCapeSet(acc.id, null, null));
    await loadCape(acc.id);
  }

  let adding = $state<null | "nick" | "url">(null);
  let nick = $state("");
  let from = $state<NickSource>("mojang");
  let url = $state("");
  let busy = $state(false);

  async function fromFile() {
    if (!inTauri) return;
    const picked = await openFile({
      multiple: true,
      directory: false,
      title: "Скины (PNG 64×64)",
      filters: [{ name: "Скин PNG", extensions: ["png"] }],
    });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    await addPaths(paths);
  }

  /// Файлы с диска (выбор или перетаскивание в окно).
  export async function addPaths(paths: string[]) {
    let last = null;
    for (const p of paths) {
      const s = await attempt(p.split(/[\\/]/).pop() ?? "Скин", () => api.skinAddFile(p));
      if (s) last = s;
    }
    if (last) {
      await loadSkins();
      skins.selected = { kind: "lib", skin: last };
      toast(paths.length > 1 ? "Скины добавлены в библиотеку" : `«${last.name}» — в библиотеке`);
    }
  }

  async function findNick(e: Event) {
    e.preventDefault();
    busy = true;
    const t = await attempt("Скин по нику", () => api.skinLookup(nick, from));
    busy = false;
    if (t) skins.selected = { kind: "nick", textures: t, from };
  }

  async function addUrl(e: Event) {
    e.preventDefault();
    busy = true;
    const s = await attempt("Скин по ссылке", () => api.skinAddUrl(url));
    busy = false;
    if (s) {
      url = "";
      adding = null;
      await loadSkins();
      skins.selected = { kind: "lib", skin: s };
      toast(`«${s.name}» — в библиотеке`);
    }
  }

  const selectedId = $derived(skins.selected?.kind === "lib" ? skins.selected.skin.id : null);
</script>

{#if account}
  <div class="account card">
    {#if account.kind === "ely"}
      {#if accountState === "ok" && accountSkin?.skin}
        {@const t = accountSkin}
        <button
          class="acc-fig"
          class:on={skins.selected?.kind === "account"}
          onclick={() => (skins.selected = { kind: "account", textures: t })}
          aria-label="Показать скин аккаунта в 3D"
        >
          <SkinFigure key={t.skin!} load={() => api.skinTexture(t.skin!)} slim={t.slim} height={96} />
        </button>
      {/if}
      <div class="grow-col">
        <b>{account.name} · Ely.by</b>
        <span class="muted small">
          {#if accountState === "loading"}Загружаю скин аккаунта…
          {:else if accountState === "ok"}Этот скин видят другие игроки на серверах с Ely.by. Нажмите на фигуру — покажу в 3D.
          {:else if accountState === "none"}У аккаунта пока стандартный скин. Выберите скин в библиотеке или каталоге и нажмите «Надеть на Ely.by».
          {:else}Скин аккаунта не загрузился — нет связи с Ely.by.{/if}
        </span>
      </div>
    {:else}
      {#if wornSkin}
        {@const w = wornSkin}
        <button
          class="acc-fig"
          class:on={skins.selected?.kind === "lib" && skins.selected.skin.id === w.id}
          onclick={() => (skins.selected = { kind: "lib", skin: w })}
          aria-label="Показать надетый скин в 3D"
        >
          <SkinFigure key={w.id} load={() => api.skinPng(w.id)} slim={w.slim} height={96} />
        </button>
      {/if}
      <div class="grow-col">
        <b>{account.name} · офлайн{wornSkin ? ` · надет «${wornSkin.name}»` : ""}</b>
        <span class="muted small">
          {#if wornSkin}
            Вы видите этот скин в игре (вид от третьего лица, F5) в сборках с модом CustomSkinLoader — лаунчер кладёт
            его туда перед каждым запуском. Другие игроки увидят его, только если на сервере стоит SkinsRestorer.
          {:else}
            Без скина Minecraft показывает офлайн-нику Стива или Алекс. Выберите скин в библиотеке и нажмите «Надеть» —
            вы увидите его в игре в сборках с модом CustomSkinLoader. Другим игрокам он виден, только если на сервере
            стоит SkinsRestorer.
          {/if}
        </span>
        <div class="cape small">
          {#if skins.cape}
            <span>Плащ надет.</span>
            <button class="btn btn-sm btn-quiet" onclick={capeOff}>Снять плащ</button>
          {:else}
            <span class="muted">Плащ: из файла PNG или у игрока, найденного по нику («Взять этот плащ себе»).</span>
          {/if}
          <button class="btn btn-sm btn-quiet" onclick={capeFromFile}>{skins.cape ? "Другой из файла…" : "Плащ из файла…"}</button>
        </div>
        {#if wornSkin || skins.cape}<SkinLoaderBuilds />{/if}
      </div>
    {/if}
  </div>
{/if}

<div class="bar">
  <button class="btn btn-sm" onclick={fromFile}><Icon name="folder" size={16} />Из файла</button>
  <button class="btn btn-sm" class:btn-primary={adding === "nick"} onclick={() => (adding = adding === "nick" ? null : "nick")}>
    <Icon name="user" size={16} />По нику
  </button>
  <button class="btn btn-sm" class:btn-primary={adding === "url"} onclick={() => (adding = adding === "url" ? null : "url")}>
    <Icon name="link" size={16} />По ссылке
  </button>
  <span class="muted small grow">Файлы скинов можно и просто перетащить в окно.</span>
</div>

{#if adding === "nick"}
  <form class="add card" onsubmit={findNick}>
    <input class="input grow-in" placeholder="Ник игрока" bind:value={nick} maxlength="16" required />
    <div class="seg" role="radiogroup" aria-label="Где искать">
      <button type="button" class="chip" class:on={from === "mojang"} role="radio" aria-checked={from === "mojang"} onclick={() => (from = "mojang")}>Minecraft</button>
      <button type="button" class="chip" class:on={from === "ely"} role="radio" aria-checked={from === "ely"} onclick={() => (from = "ely")}>Ely.by</button>
    </div>
    <button class="btn btn-sm btn-primary" type="submit" disabled={busy || !nick.trim()}>{busy ? "Ищу…" : "Найти"}</button>
  </form>
{:else if adding === "url"}
  <form class="add card" onsubmit={addUrl}>
    <input class="input grow-in" placeholder="https://…/skin.png" bind:value={url} required />
    <button class="btn btn-sm btn-primary" type="submit" disabled={busy || !url.trim()}>{busy ? "Скачиваю…" : "Добавить"}</button>
  </form>
{/if}

{#if skins.loaded && skins.list.length === 0}
  <p class="note">
    В библиотеке пока пусто. Добавьте скин из файла, по нику игрока или по ссылке — или найдите в каталоге.
  </p>
{:else}
  <ul class="grid">
    {#each skins.list as s (s.id)}
      <li>
        <button class="cell" class:on={selectedId === s.id} onclick={() => (skins.selected = { kind: "lib", skin: s })}>
          {#key skins.version}
            <SkinFigure key={s.id} load={() => api.skinPng(s.id)} slim={s.slim} height={112} />
          {/key}
          <span class="name" title={s.name}>{s.name}</span>
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .account {
    display: flex;
    align-items: center;
    gap: var(--s4);
    padding: var(--s3) var(--s4);
    margin-bottom: var(--s4);
  }
  .acc-fig {
    padding: var(--s1);
    border: 1px solid transparent;
    border-radius: var(--r);
    background: none;
    cursor: pointer;
  }
  .acc-fig.on,
  .acc-fig:hover {
    border-color: var(--accent);
  }
  .grow-col {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--s1);
  }
  .cape {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s2);
  }
  .add {
    display: flex;
    align-items: center;
    gap: var(--s2);
    padding: var(--s3);
    margin-bottom: var(--s3);
  }
  .grow-in {
    flex: 1;
  }
  .seg {
    display: flex;
    gap: var(--s1);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: var(--s3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .cell {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s2);
    width: 100%;
    padding: var(--s3) var(--s2);
    border: 1px solid var(--border);
    border-radius: var(--r-card);
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
  }
  .cell:hover {
    border-color: var(--border-strong);
  }
  .cell.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .name {
    max-width: 100%;
    overflow: hidden;
    font-size: var(--fs-12);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
