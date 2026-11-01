<script lang="ts">
  // Левая панель раздела «Скины»: выбранный скин в 3D (поворот мышью,
  // анимации) и что с ним можно сделать.
  import { save as saveFile, open as openFile } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "../../lib/api";
  import { currentAccount } from "../../lib/accounts.svelte";
  import { forget } from "../../lib/skinrender";
  import { knownSlim, loadCape, loadSkins, pixels, saveSkinSettings, skins, wear, type Selected } from "../../lib/skins.svelte";
  import { inTauri } from "../../lib/tauri";
  import { attempt, toast } from "../../lib/toast.svelte";
  import type { Decoded } from "../../lib/skinrender";
  import Icon from "../Icon.svelte";
  import SkinViewer3D, { type Pose } from "../SkinViewer3D.svelte";

  const POSES: { id: Pose; title: string }[] = [
    { id: "idle", title: "Стоит" },
    { id: "walk", title: "Идёт" },
    { id: "run", title: "Бежит" },
    { id: "fly", title: "Летит" },
  ];
  let pose = $state<Pose>("walk");
  let shown = $state<{ skin: Decoded; cape: ImageData | null } | null>(null);
  let failed = $state("");
  let renaming = $state(false);
  let newName = $state("");
  let busy = $state(false);

  const sel = $derived(skins.selected);
  const account = $derived(currentAccount());
  const ely = $derived(account?.kind === "ely");
  const offline = $derived(account?.kind === "offline");

  /// Офлайн-ник: надеть скин из библиотеки (или снять).
  async function wearOffline(on: boolean) {
    if (sel?.kind !== "lib" || !account) return;
    const { id, name } = sel.skin;
    const nick = account.name;
    const done = await attempt("Скин", () => wear(account.id, on ? id : null).then(() => true));
    if (done) toast(on ? `«${name}» надет на ${nick}` : `Скин снят с ${nick}`);
  }

  // Картинка выбранного скина; после правки в редакторе — заново.
  $effect(() => {
    const s = sel;
    void skins.version;
    failed = "";
    renaming = false;
    if (!s) {
      shown = null;
      return;
    }
    pixels(s)
      .then((p) => {
        if (skins.selected === s) shown = p;
      })
      .catch((e) => (failed = String(e)));
  });

  // На надетом скине офлайн-ника — его плащ.
  const cape = $derived(
    shown?.cape ?? (offline && sel?.kind === "lib" && skins.worn === sel.skin.id ? skins.cape : null),
  );

  async function takeCape() {
    if (sel?.kind !== "nick" || !sel.textures.cape || !account) return;
    const url = sel.textures.cape;
    const acc = account;
    const done = await attempt("Плащ", () => api.skinCapeSet(acc.id, null, url).then(() => true));
    if (done) {
      await loadCape(acc.id);
      toast(`Плащ надет на ${acc.name}`);
    }
  }

  const slim = $derived(sel ? (knownSlim(sel) ?? shown?.skin.slim ?? false) : false);

  function title(s: Selected): string {
    switch (s.kind) {
      case "lib":
        return s.skin.name;
      case "cat":
        return s.item.name ?? (s.item.tags.length ? s.item.tags.slice(0, 3).join(", ") : "Скин из каталога");
      case "nick":
        return `Скин игрока ${s.textures.name}`;
      case "account":
        return `Ваш скин на Ely.by — ${s.textures.name}`;
    }
  }

  function link(url: string) {
    if (inTauri) openUrl(url).catch(() => {});
    else window.open(url);
  }

  async function addToLibrary() {
    if (!sel || sel.kind === "lib") return;
    busy = true;
    const s = sel;
    const added = await attempt("Библиотека", () =>
      s.kind === "cat"
        ? api.skinAddTexture(
            s.item.url,
            s.item.name ?? (s.item.tags[0] || "Скин из каталога"),
            s.catalog === "ely" ? "каталог Ely.by" : "MineSkin",
            s.catalog === "ely" ? s.item.page : null,
            s.item.slim ?? shown?.skin.slim ?? null,
          )
        : api.skinAddTexture(
            s.textures.skin!,
            s.textures.name,
            s.kind === "account" ? "аккаунт Ely.by" : `ник ${s.textures.name} (${s.from === "ely" ? "Ely.by" : "Minecraft"})`,
            null,
            s.textures.slim,
          ),
    );
    busy = false;
    if (added) {
      await loadSkins();
      toast(`«${added.name}» — в библиотеке скинов`);
    }
  }

  /// Сменить скин на Ely.by: API для этого у Ely.by нет — открываем сайт.
  async function wearOnEly() {
    if (!sel) return;
    if (sel.kind === "cat" && sel.catalog === "ely" && sel.item.page) return link(sel.item.page);
    if (sel.kind === "lib" && sel.skin.page) return link(sel.skin.page);
    if (sel.kind === "lib") {
      const id = sel.skin.id;
      await attempt("Папка", () => api.skinReveal(id));
      link("https://ely.by/skins/add");
      toast("Перетащите файл скина из папки на страницу загрузки Ely.by, потом нажмите там «Надеть»");
    }
  }

  async function edit() {
    if (sel?.kind !== "lib") return;
    const id = sel.skin.id;
    await attempt("Редактор", () => api.skinEdit(id));
    toast("Сохраните скин в редакторе — 3D здесь обновится само");
  }

  async function pickEditor() {
    if (!inTauri) return;
    const path = await openFile({
      multiple: false,
      directory: false,
      title: "Программа для редактирования скинов",
      filters: [{ name: "Программы", extensions: ["exe"] }],
    });
    if (typeof path !== "string") return;
    await attempt("Редактор", () => saveSkinSettings({ editor: path }));
    toast("Готово: «Редактировать» откроет скин в этой программе");
  }

  async function rename() {
    if (sel?.kind !== "lib") return;
    const id = sel.skin.id;
    const s = await attempt("Название", () => api.skinRename(id, newName));
    if (s) {
      renaming = false;
      skins.selected = { kind: "lib", skin: s };
      await loadSkins();
    }
  }

  async function setSlim(v: boolean) {
    if (sel?.kind !== "lib") return;
    const id = sel.skin.id;
    const s = await attempt("Модель", () => api.skinSetSlim(id, v));
    if (s) {
      forget(id);
      skins.selected = { kind: "lib", skin: s };
      await loadSkins();
    }
  }

  async function exportSkin() {
    if (sel?.kind !== "lib" || !inTauri) return;
    const { id, name } = sel.skin;
    const to = await saveFile({ title: "Сохранить скин", defaultPath: `${name}.png`, filters: [{ name: "PNG", extensions: ["png"] }] });
    if (typeof to === "string") {
      const done = await attempt("Сохранение", () => api.skinExport(id, to).then(() => true));
      if (done) toast("Скин сохранён");
    }
  }

  async function remove() {
    if (sel?.kind !== "lib") return;
    const { id, name } = sel.skin;
    const done = await attempt("Удаление", () => api.skinRemove(id).then(() => true));
    if (done) {
      skins.selected = null;
      await loadSkins();
      toast(`«${name}» удалён из библиотеки`);
    }
  }

  const editorName = $derived(skins.settings.editor?.split(/[\\/]/).pop()?.replace(/\.exe$/i, "") ?? null);
</script>

<div class="stage card">
  {#if sel}
    <div class="head">
      {#if renaming && sel.kind === "lib"}
        <form
          class="rename"
          onsubmit={(e) => {
            e.preventDefault();
            rename();
          }}
        >
          <!-- svelte-ignore a11y_autofocus -->
          <input class="input" bind:value={newName} maxlength="64" autofocus />
          <button class="btn btn-sm btn-primary" type="submit">OK</button>
        </form>
      {:else}
        <h2 title={title(sel)}>{title(sel)}</h2>
        {#if sel.kind === "lib"}
          <button
            class="btn btn-sm btn-quiet"
            title="Переименовать"
            aria-label="Переименовать"
            onclick={() => {
              newName = sel.kind === "lib" ? sel.skin.name : "";
              renaming = true;
            }}><Icon name="edit" size={16} /></button
          >
        {/if}
      {/if}
    </div>
    <p class="muted small sub">
      {#if sel.kind === "lib"}
        {sel.skin.source || "библиотека"} · {slim ? "тонкие руки" : "обычные руки"}
      {:else if sel.kind === "cat"}
        {sel.catalog === "ely" ? "Каталог Ely.by" : "MineSkin"}{#if sel.item.likes !== null}{` · ♥ ${sel.item.likes.toLocaleString("ru-RU")}`}{/if}{#if sel.item.wearers !== null}{` · носят ${sel.item.wearers.toLocaleString("ru-RU")}`}{/if}
      {:else}
        {sel.kind === "account" || sel.from === "ely" ? "Ely.by" : "Minecraft"}{sel.textures.cape ? " · с плащом" : ""}
      {/if}
    </p>

    <div class="view">
      {#if failed}
        <p class="warn"><Icon name="error" size={18} />{failed}</p>
      {:else}
        <SkinViewer3D skin={shown?.skin.image ?? null} {slim} {cape} {pose} width={280} height={360} />
      {/if}
    </div>
    <div class="poses" role="radiogroup" aria-label="Анимация">
      {#each POSES as p (p.id)}
        <button class="chip" class:on={pose === p.id} role="radio" aria-checked={pose === p.id} onclick={() => (pose = p.id)}>{p.title}</button>
      {/each}
    </div>

    {#if sel.kind === "cat" && sel.item.tags.length}
      <p class="tags">{#each sel.item.tags as t (t)}<span class="tag">{t}</span>{/each}</p>
    {/if}

    <div class="actions">
      {#if sel.kind === "lib"}
        {#if ely}
          <button class="btn btn-primary" onclick={wearOnEly}><Icon name="skins" size={18} />Надеть на Ely.by</button>
        {:else if offline && account}
          {#if skins.worn === sel.skin.id}
            <button class="btn" onclick={() => wearOffline(false)}><Icon name="check" size={18} />Надет на {account.name} · снять</button>
          {:else}
            <button class="btn btn-primary" onclick={() => wearOffline(true)}><Icon name="skins" size={18} />Надеть на {account.name}</button>
          {/if}
        {/if}
        <button class="btn" onclick={edit}><Icon name="edit" size={18} />Редактировать{editorName ? ` в ${editorName}` : ""}</button>
        <label class="opt">
          <input type="checkbox" checked={sel.skin.slim} onchange={(e) => setSlim(e.currentTarget.checked)} />
          Тонкие руки (модель «Алекс»)
        </label>
        <div class="row">
          <button class="btn btn-sm btn-quiet" onclick={exportSkin}><Icon name="export" size={16} />Сохранить как…</button>
          <button class="btn btn-sm btn-quiet" onclick={() => sel?.kind === "lib" && attempt("Папка", () => api.skinReveal(sel.skin.id))}
            ><Icon name="folder" size={16} />В папке</button
          >
          <button class="btn btn-sm btn-quiet danger" onclick={remove}><Icon name="trash" size={16} />Удалить</button>
        </div>
        <button class="link small" onclick={pickEditor}>
          {editorName ? "Другая программа для «Редактировать»…" : "Выбрать программу для «Редактировать» (Blockbench, Paint.NET…)"}
        </button>
        {#if editorName}<button class="link small" onclick={() => saveSkinSettings({ editor: null })}>Открывать программой Windows для PNG</button>{/if}
      {:else}
        <button class="btn btn-primary" disabled={busy} onclick={addToLibrary}><Icon name="plus" size={18} />В библиотеку</button>
        {#if sel.kind === "nick" && sel.textures.cape && offline}
          <button class="btn" onclick={takeCape}><Icon name="plus" size={18} />Взять этот плащ себе</button>
        {/if}
        {#if sel.kind === "cat"}
          {#if sel.catalog === "ely" && ely}
            <button class="btn" onclick={wearOnEly}><Icon name="skins" size={18} />Надеть на Ely.by</button>
          {/if}
          {#if sel.item.page}
            <button class="btn btn-sm btn-quiet" onclick={() => sel?.kind === "cat" && sel.item.page && link(sel.item.page)}
              ><Icon name="link" size={16} />Открыть на сайте</button
            >
          {/if}
        {/if}
      {/if}
    </div>
    {#if (sel.kind === "lib" || (sel.kind === "cat" && sel.catalog === "ely")) && ely}
      <p class="muted small">
        Сменить скин через API Ely.by нельзя — такого API у них нет, поэтому кнопка открывает сайт: там нажмите «Надеть».
      </p>
    {/if}
  {:else}
    <div class="empty">
      <Icon name="skins" size={40} />
      <p class="muted">Выберите скин — он появится здесь в 3D. Тяните мышью, чтобы повернуть, колесо — приблизить.</p>
    </div>
  {/if}
</div>

<style>
  .stage {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s3);
    padding: var(--s4);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s2);
    align-self: stretch;
    min-width: 0;
  }
  h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
    overflow: hidden;
    font-size: var(--fs-16);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename {
    display: flex;
    flex: 1;
    gap: var(--s2);
  }
  .rename input {
    flex: 1;
  }
  .sub {
    align-self: stretch;
    margin: 0;
  }
  .view {
    display: grid;
    place-items: center;
    min-height: 360px;
  }
  .poses,
  .row {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--s2);
  }
  .actions {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: var(--s2);
    align-self: stretch;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    font-size: var(--fs-14);
    cursor: pointer;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s1);
    margin: 0;
  }
  .tag {
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--surface-raised);
    font-size: var(--fs-12);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-muted);
    text-align: left;
    text-decoration: underline;
    cursor: pointer;
  }
  .danger:hover {
    color: var(--danger);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s3);
    padding: var(--s6) var(--s3);
    text-align: center;
    color: var(--text-muted);
  }
  p {
    margin: 0;
  }
</style>
