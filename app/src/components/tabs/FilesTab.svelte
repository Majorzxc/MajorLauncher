<script lang="ts">
  // Моды, ресурспаки, шейдеры: файлы в папке сборки. Моды можно выключать,
  // файлы — добавить с диска (каталог Modrinth — этап 5).
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type FileEntry, type Folder, type Loader } from "../../lib/api";
  import { openCatalog } from "../../lib/catalog.svelte";
  import { date, size } from "../../lib/format";
  import type { IconName } from "../../lib/icons";
  import { inTauri } from "../../lib/tauri";
  import { attempt, toast } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";

  let { id, folder, loader }: { id: string; folder: Exclude<Folder, "screenshots">; loader: Loader } = $props();

  const ICON: Record<typeof folder, IconName> = { mods: "mods", resourcepacks: "resourcepack", shaderpacks: "shader" };
  const EMPTY: Record<typeof folder, string> = {
    mods: "Модов нет. Найдите их в каталоге Modrinth или добавьте свои файлы .jar кнопкой «Добавить».",
    resourcepacks: "Ресурспаков нет. Положите zip-файлы в папку — игра увидит их в настройках.",
    shaderpacks: "Шейдеров нет. Для них нужен мод Iris или OptiFine.",
  };

  let list: FileEntry[] | null = $state(null);
  let error = $state("");

  async function load() {
    try {
      list = await api.files(id, folder);
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    list = null;
    load();
  });

  async function add() {
    if (!inTauri) return toast("В браузере файлы не выбрать", { error: true });
    const jar = folder === "mods";
    const picked = await open({
      multiple: true,
      title: jar ? "Моды (.jar)" : "Архивы (.zip)",
      filters: [{ name: jar ? "Моды" : "Архивы", extensions: [jar ? "jar" : "zip"] }],
    });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (!paths.length) return;
    const errors = await attempt("Файлы", () => api.addFiles(id, folder, paths));
    errors?.forEach((e) => toast(e, { error: true }));
    if (errors && errors.length < paths.length) toast(`Добавлено: ${paths.length - errors.length}`);
    load();
  }

  async function toggle(f: FileEntry) {
    await attempt("Мод", () => api.setEnabled(id, folder, f.file, !f.enabled));
    load();
  }
</script>

{#if folder === "mods" && loader === "vanilla"}
  <p class="warn">
    <Icon name="warning" size={18} />В ванильной сборке моды не загрузятся: выберите загрузчик на вкладке «Настройки».
  </p>
{/if}

<div class="bar">
  <span class="muted small grow">
    {#if list?.length}{list.length} · {list.filter((f) => f.enabled).length} включено{/if}
  </span>
  <button
    class="btn btn-sm btn-primary"
    onclick={() => openCatalog(id, folder === "mods" ? "mod" : folder === "resourcepacks" ? "resourcepack" : "shader")}
  >
    <Icon name="search" size={16} />Каталог
  </button>
  <button class="btn btn-sm" onclick={add}><Icon name="plus" size={16} />Добавить файл</button>
  <button class="btn btn-sm" onclick={() => attempt("Папка", () => api.open(id, { kind: "folder", name: folder }))}>
    <Icon name="folder" size={16} />Открыть папку
  </button>
  <button class="btn btn-sm btn-quiet" aria-label="Обновить" title="Обновить" onclick={load}><Icon name="refresh" size={16} /></button>
</div>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if list && list.length === 0}
  <p class="note">{EMPTY[folder]}</p>
{:else if list}
  <ul class="rows">
    {#each list as f (f.file)}
      <li class:off={!f.enabled}>
        <Icon name={ICON[folder]} size={20} />
        <span class="grow">{f.name}</span>
        <span class="muted small">{f.is_dir ? "папка" : size(f.size)}</span>
        <span class="muted small when">{date(f.modified)}</span>
        {#if folder === "mods"}
          <label class="switch" title={f.enabled ? "Выключить" : "Включить"}>
            <input type="checkbox" checked={f.enabled} onchange={() => toggle(f)} aria-label="Мод {f.name} включён" />
            <span></span>
          </label>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .off .grow {
    color: var(--text-muted);
    text-decoration: line-through;
  }
  .when {
    width: 140px;
    text-align: right;
  }
</style>
