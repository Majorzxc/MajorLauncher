<script lang="ts">
  // Страница проекта прямо в лаунчере (раздел 5.4 ТЗ): описание, картинки,
  // версии с отметкой «подходит к сборке», установка нужной версии.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type CatalogVersion, type Project, type ProjectType } from "../../lib/api";
  import { busy, catalog, install, installModpack, installed, searchTarget } from "../../lib/catalog.svelte";
  import { openSettings } from "../../lib/nav.svelte";
  import { categoryName } from "../../lib/categories";
  import { ago, count, size } from "../../lib/format";
  import { renderMarkdown } from "../../lib/markdown";
  import { translateHtml } from "../../lib/translate";
  import { inTauri } from "../../lib/tauri";
  import Icon from "../Icon.svelte";

  let { id }: { id: string } = $props();

  let project = $state<Project | null>(null);
  let error = $state("");
  let tab: "about" | "gallery" | "versions" = $state("about");
  let versions = $state<CatalogVersion[] | null>(null);
  let versionsError = $state("");
  let allVersions = $state(false);

  $effect(() => {
    const pid = id;
    project = null;
    error = "";
    api
      .catalogProject(pid)
      .then((p) => (project = p))
      .catch((e) => (error = String(e)));
  });

  const kind = $derived<ProjectType>(project?.project_type ?? catalog.kind);
  const html = $derived(project ? renderMarkdown(project.body) : "");

  // Перевод на русский: описание и краткое описание. Не вышло — оригинал.
  let ruHtml = $state<string | null>(null);
  let ruSummary = $state<string | null>(null);
  let translating = $state(false);
  let translateError = $state("");
  /// Показать оригинал этого описания (на время просмотра).
  let original = $state(false);
  $effect(() => {
    const [source, p, off] = [html, project, !catalog.translate];
    ruHtml = null;
    ruSummary = null;
    translateError = "";
    if (!p || off) return;
    translating = true;
    Promise.all([translateHtml(source), api.catalogTranslate([p.description])])
      .then(([body, [summary]]) => {
        if (project !== p) return;
        ruHtml = body;
        ruSummary = summary;
      })
      .catch((e) => (translateError = String(e)))
      .finally(() => (translating = false));
  });
  const useRu = $derived(catalog.translate && !original);
  const shownHtml = $derived(useRu && ruHtml ? ruHtml : html);
  const have = $derived(project ? installed.get(project.id) : undefined);

  $effect(() => {
    if (tab !== "versions" || !project || !catalog.pick) return;
    const [pid, all, t, k] = [project.id, allVersions, searchTarget(catalog.pick), kind];
    versions = null;
    versionsError = "";
    api
      .catalogVersions(t, k, pid, all)
      .then((v) => (versions = v))
      .catch((e) => (versionsError = String(e)));
  });

  function open(url: string | null) {
    if (!url) return;
    if (inTauri) openUrl(url);
    else window.open(url);
  }

  // Ссылки в описании — во внешнем браузере, а не внутри окна лаунчера.
  function intercept(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute("href");
    if (href?.startsWith("http")) open(href);
  }

  const links = $derived(
    project
      ? [
          { title: "Modrinth", url: `https://modrinth.com/${project.project_type}/${project.slug}` },
          { title: "Исходники", url: project.source_url },
          { title: "Сообщить об ошибке", url: project.issues_url },
          { title: "Вики", url: project.wiki_url },
          { title: "Discord", url: project.discord_url },
          ...project.donation_urls.slice(0, 1).map((d) => ({ title: "Поддержать автора", url: d.url })),
        ].filter((l) => l.url)
      : [],
  );
  const when = (iso: string) => ago(Math.floor(Date.parse(iso) / 1000));
  const games = (list: string[]) => (list.length > 3 ? `${list.slice(-3).join(", ")} и ещё ${list.length - 3}` : list.join(", "));
</script>

<button class="back btn btn-quiet btn-sm" onclick={() => (catalog.project = null)}>
  <Icon name="arrow-left" size={16} />Каталог
</button>

{#if error}
  <p class="warn"><Icon name="error" size={18} />Не удалось открыть проект: {error}</p>
{:else if !project}
  <p class="muted">Загружаю…</p>
{:else}
  <header class="head">
    {#if project.icon_url}<img class="icon" src={project.icon_url} alt="" width="96" height="96" />{/if}
    <div class="title">
      <h1>{project.title}</h1>
      <p class="muted">{(useRu && ruSummary) || project.description}</p>
      <p class="muted small stats">
        <Icon name="download" size={14} />{count(project.downloads)} скачиваний
        <span>· <Icon name="star" size={14} />{count(project.followers)}</span>
        <span>· обновлён {when(project.updated)}</span>
        {#if project.license}<span>· лицензия {project.license.name || project.license.id}</span>{/if}
      </p>
      <div class="cats">
        {#each [...project.loaders, ...project.categories].slice(0, 8) as c}<span class="cat">{categoryName(c)}</span>{/each}
      </div>
    </div>
    <div class="actions">
      {#if have}
        <span class="have"><Icon name="check" size={18} />{have.enabled ? "Установлен" : "Выключен в сборке"}</span>
      {:else}
        <button
          class="btn btn-primary"
          disabled={busy.has(project.id)}
          onclick={() => (kind === "modpack" ? installModpack(project!.id, null, project!.title) : install(project!.id, kind))}
        >
          <Icon name="download" size={18} />{busy.has(project.id) ? "Ставим…" : kind === "modpack" ? "Создать сборку" : "Установить"}
        </button>
      {/if}
      <div class="links">
        {#each links as l (l.title)}
          <button class="btn btn-sm btn-quiet" onclick={() => open(l.url)}><Icon name="link" size={14} />{l.title}</button>
        {/each}
      </div>
    </div>
  </header>

  <nav class="tabs" aria-label="Разделы проекта">
    <button class="tab" class:on={tab === "about"} onclick={() => (tab = "about")}>Описание</button>
    {#if project.gallery.length}
      <button class="tab" class:on={tab === "gallery"} onclick={() => (tab = "gallery")}>Картинки · {project.gallery.length}</button>
    {/if}
    <button class="tab" class:on={tab === "versions"} onclick={() => (tab = "versions")}>Версии</button>
  </nav>

  {#if tab === "about"}
    <!-- Описание прошло DOMPurify (lib/markdown.ts): без скриптов, стилей и фреймов. -->
    <p class="muted small lang">
      {#if !catalog.translate}
        Перевод описаний на русский можно включить в
        <button class="link" onclick={() => openSettings("catalog")}>настройках</button>.
      {:else if original}
        Оригинал. <button class="link" onclick={() => (original = false)}>Показать перевод</button>
      {:else if translating}
        Перевожу на русский…
      {:else if translateError}
        Перевод не удался — показан оригинал.
      {:else}
        Машинный перевод Google. <button class="link" onclick={() => (original = true)}>Показать оригинал</button>
      {/if}
    </p>
    <!-- Перевод вставлен текстом в уже очищенный HTML (lib/translate.ts). -->
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <article class="md" onclick={intercept}>{@html shownHtml}</article>
  {:else if tab === "gallery"}
    <div class="gallery">
      {#each project.gallery as g (g.url)}
        <figure>
          <button class="shot" onclick={() => open(g.raw_url ?? g.url)} title="Открыть в браузере">
            <img src={g.url} alt={g.title ?? ""} loading="lazy" />
          </button>
          {#if g.title}<figcaption class="small">{g.title}</figcaption>{/if}
        </figure>
      {/each}
    </div>
  {:else}
    <label class="opt">
      <input type="checkbox" bind:checked={allVersions} />Показать все версии, а не только подходящие к сборке
    </label>
    {#if versionsError}
      <p class="warn"><Icon name="error" size={18} />{versionsError}</p>
    {:else if !versions}
      <p class="muted">Загружаю версии…</p>
    {:else if !versions.length}
      <p class="note">Подходящих версий нет. Включите «Показать все версии».</p>
    {:else}
      <ul class="rows">
        {#each versions.slice(0, 100) as v (v.id)}
          <li class:off={!v.compatible}>
            <span class="grow">
              <b>{v.version_number}</b>
              {#if v.version_type !== "release"}<span class="cat">{v.version_type === "beta" ? "бета" : "альфа"}</span>{/if}
              {#if have?.version_id === v.id}<span class="cat">стоит</span>{/if}
              <span class="muted small">
                {games(v.game_versions)} · {v.loaders.map(categoryName).join(", ")} · {when(v.date_published)}
                {#if v.files[0]}· {size(v.files.find((f) => f.primary)?.size ?? v.files[0].size)}{/if}
                {#if !v.compatible}· не подходит к сборке{/if}
              </span>
            </span>
            <button
              class="btn btn-sm"
              disabled={busy.has(v.project_id) || have?.version_id === v.id}
              onclick={() => (kind === "modpack" ? installModpack(v.project_id, v.id, project!.title) : install(v.project_id, kind, v.id))}
            >
              <Icon name="download" size={16} />Установить
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
{/if}

<style>
  .lang {
    margin: 0 0 var(--s3);
  }
  .lang .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-muted);
    text-decoration: underline;
    cursor: pointer;
    font-size: inherit;
  }
  .lang .link:hover {
    color: var(--text);
  }
  .back {
    margin-bottom: var(--s3);
  }
  .head {
    display: flex;
    gap: var(--s5);
    align-items: flex-start;
    margin-bottom: var(--s4);
  }
  .icon {
    flex: none;
    border-radius: var(--r-card);
    background: var(--surface-raised);
  }
  .title {
    flex: 1;
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-size: var(--fs-32);
  }
  .title p {
    margin: var(--s1) 0;
  }
  .stats {
    display: flex;
    align-items: center;
    gap: var(--s1);
    flex-wrap: wrap;
  }
  .stats span {
    display: inline-flex;
    align-items: center;
    gap: var(--s1);
  }
  .cats {
    display: flex;
    gap: var(--s1);
    flex-wrap: wrap;
  }
  .cat {
    display: inline-block;
    padding: 0 var(--s2);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: var(--fs-12);
    color: var(--text-muted);
  }
  .actions {
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--s2);
    max-width: 260px;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--s1);
  }
  .have {
    display: flex;
    align-items: center;
    gap: var(--s2);
    color: var(--text-muted);
  }
  .tabs {
    margin-bottom: var(--s4);
  }
  .md {
    max-width: 860px;
    line-height: 1.6;
    user-select: text;
    overflow-wrap: anywhere;
  }
  .md :global(img) {
    max-width: 100%;
    height: auto;
    border-radius: var(--r);
  }
  .md :global(a) {
    color: var(--text);
    text-decoration: underline;
  }
  .md :global(h1),
  .md :global(h2),
  .md :global(h3) {
    margin: var(--s5) 0 var(--s2);
    line-height: 1.25;
  }
  .md :global(pre),
  .md :global(code) {
    font-family: Consolas, monospace;
    font-size: var(--fs-14);
    background: var(--surface-raised);
    border-radius: var(--r-sm);
  }
  .md :global(pre) {
    padding: var(--s3);
    overflow: auto;
  }
  .md :global(table) {
    border-collapse: collapse;
  }
  .md :global(td),
  .md :global(th) {
    padding: var(--s1) var(--s2);
    border: 1px solid var(--border);
  }
  .md :global(blockquote) {
    margin: var(--s3) 0;
    padding-left: var(--s3);
    border-left: 3px solid var(--border-strong);
    color: var(--text-muted);
  }
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: var(--s3);
  }
  figure {
    margin: 0;
  }
  .shot {
    display: block;
    width: 100%;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--r);
    overflow: hidden;
    background: var(--surface-raised);
    cursor: zoom-in;
  }
  .shot img {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: cover;
  }
  figcaption {
    margin-top: var(--s1);
    color: var(--text-muted);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    margin-bottom: var(--s3);
    font-size: var(--fs-14);
    cursor: pointer;
  }
  .grow {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .off b {
    color: var(--text-muted);
  }
</style>
