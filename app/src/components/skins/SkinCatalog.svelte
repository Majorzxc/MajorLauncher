<script lang="ts">
  // Каталог скинов: Ely.by (теги, разделы, популярность) и MineSkin
  // (миллионы скинов, поиск по названию). Сетка — фигуры во весь рост.
  import { api, type CatalogSkin, type SkinCatalogId } from "../../lib/api";
  import { saveSkinSettings, skins } from "../../lib/skins.svelte";
  import { attempt } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";
  import Select from "../Select.svelte";
  import SkinFigure from "../SkinFigure.svelte";

  const KINDS = [
    { value: "", label: "Все разделы" },
    { value: "Fantasy", label: "Фэнтези" },
    { value: "Heroes", label: "Герои" },
    { value: "Evildoers", label: "Злодеи" },
    { value: "Characters", label: "Персонажи" },
    { value: "Adventure", label: "Приключения" },
    { value: "Comics", label: "Комиксы" },
    { value: "Historical", label: "Исторические" },
    { value: "Weekend", label: "На выходные" },
  ];

  let catalog = $state<SkinCatalogId>("ely");
  let search = $state("");
  let sort = $state("best");
  let kind = $state("");
  let slimOnly = $state(false);
  let items = $state<CatalogSkin[]>([]);
  let next = $state<string | null>(null);
  let total = $state<number | null>(null);
  let hidden = $state(0);
  let loading = $state(false);
  let error = $state("");
  /// Номер запроса: ответ на устаревший запрос не показываем.
  let request = 0;

  const SORTS = $derived(
    catalog === "ely"
      ? [
          { value: "best", label: "Лучшие" },
          { value: "time", label: "Новые" },
        ]
      : [
          { value: "popular", label: "Популярные" },
          { value: "new", label: "Новые" },
        ],
  );

  async function load(more = false) {
    const id = ++request;
    loading = true;
    error = "";
    try {
      const page = await api.skinCatalog({
        catalog,
        search,
        sort,
        kind: catalog === "ely" ? kind : "",
        slim_only: catalog === "ely" && slimOnly,
        page: more ? (next ?? "") : "",
        show_sensitive: skins.settings.show_sensitive,
      });
      if (id !== request) return;
      // Один скин может прийти и на следующей странице — без повторов.
      const seen = new Set(more ? items.map((i) => i.url) : []);
      const fresh = page.items.filter((i) => !seen.has(i.url) && seen.add(i.url));
      items = more ? [...items, ...fresh] : fresh;
      next = page.next;
      total = page.total;
      hidden = more ? hidden + page.hidden : page.hidden;
    } catch (e) {
      if (id === request) error = String(e);
    } finally {
      if (id === request) loading = false;
    }
  }

  // Сменили фильтры — с первой страницы.
  $effect(() => {
    void [catalog, sort, kind, slimOnly, skins.settings.show_sensitive];
    load();
  });

  function setCatalog(c: SkinCatalogId) {
    catalog = c;
    sort = c === "ely" ? "best" : "popular";
    search = "";
  }

  function submit(e: Event) {
    e.preventDefault();
    load();
  }

  const selectedUrl = $derived(skins.selected?.kind === "cat" ? skins.selected.item.url : null);
</script>

<div class="sources" role="radiogroup" aria-label="Каталог">
  <button class="chip" class:on={catalog === "ely"} role="radio" aria-checked={catalog === "ely"} onclick={() => setCatalog("ely")}>Ely.by</button>
  <button class="chip" class:on={catalog === "mineskin"} role="radio" aria-checked={catalog === "mineskin"} onclick={() => setCatalog("mineskin")}>MineSkin</button>
  <span class="muted small">
    {catalog === "ely"
      ? "Скины игроков Ely.by: теги, разделы, лайки. Понравившийся можно сразу надеть на аккаунт Ely.by."
      : "Миллионы скинов, загруженных на MineSkin. Названия есть не у всех — поиск находит только названные."}
  </span>
</div>

<form class="bar" onsubmit={submit}>
  <input
    class="input grow"
    placeholder={catalog === "ely" ? "Теги через запятую: knight, рыцарь, girl…" : "Название: knight, ninja…"}
    bind:value={search}
  />
  <button class="btn btn-sm" type="submit"><Icon name="search" size={16} />Найти</button>
  <Select label="Сортировка" icon="sort" value={sort} options={SORTS} onchange={(v) => (sort = v)} />
  {#if catalog === "ely"}
    <Select label="Раздел" value={kind} options={KINDS} onchange={(v) => (kind = v)} />
  {/if}
</form>
<div class="filters">
  {#if catalog === "ely"}
    <label class="opt"><input type="checkbox" bind:checked={slimOnly} />Только тонкие руки</label>
  {/if}
  <label class="opt">
    <input type="checkbox" checked={skins.settings.show_sensitive} onchange={(e) => attempt("Настройки", () => saveSkinSettings({ show_sensitive: e.currentTarget.checked }))} />
    Показывать спорные скины
  </label>
  <span class="muted small found">
    {#if total !== null}Найдено: {total >= 10000 ? "больше 10 000" : total.toLocaleString("ru-RU")}.{/if}
    {#if hidden}Скрыто спорных: {hidden} (каталоги никто не проверяет — лаунчер прячет скины с темами вроде нацизма и 18+).{/if}
  </span>
</div>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if !loading && items.length === 0}
  <p class="note">Ничего не нашлось. {catalog === "ely" ? "Попробуйте другой тег — по-английски их больше." : "Попробуйте другое слово."}</p>
{/if}

<ul class="grid">
  {#each items as it (it.url)}
    <li>
      <button class="cell" class:on={selectedUrl === it.url} onclick={() => (skins.selected = { kind: "cat", item: it, catalog })}>
        <SkinFigure
          key={it.url}
          load={() => api.skinTexture(it.url)}
          slim={it.slim}
          height={112}
          onslim={(s) => {
            if (it.slim === null) it.slim = s;
          }}
        />
        <span class="name" title={it.name ?? it.tags.join(", ")}>
          {it.name ?? (it.tags.length ? it.tags.slice(0, 2).join(", ") : "без названия")}
        </span>
        {#if it.likes !== null}<span class="muted likes">♥ {it.likes.toLocaleString("ru-RU")}</span>{/if}
      </button>
    </li>
  {/each}
</ul>

{#if loading}
  <p class="muted center">Загружаю…</p>
{:else if next && items.length}
  <div class="center"><button class="btn" onclick={() => load(true)}>Ещё скины</button></div>
{/if}

<style>
  .sources {
    display: flex;
    align-items: center;
    gap: var(--s2);
    margin-bottom: var(--s3);
  }
  .filters {
    display: flex;
    align-items: center;
    gap: var(--s4);
    margin-bottom: var(--s3);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    font-size: var(--fs-14);
    cursor: pointer;
    white-space: nowrap;
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
    gap: var(--s1);
    width: 100%;
    padding: var(--s3) var(--s2) var(--s2);
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
  .found {
    flex: 1;
    min-width: 0;
  }
  .likes {
    font-size: var(--fs-12);
  }
  .center {
    display: flex;
    justify-content: center;
    margin: var(--s4) 0;
    text-align: center;
  }
</style>
