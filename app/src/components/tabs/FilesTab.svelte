<script lang="ts">
  // Моды, ресурспаки, шейдеры: файлы в папке сборки. Моды можно выключать.
  import { api, type FileEntry, type Folder } from "../../lib/api";
  import { date, size } from "../../lib/format";
  import type { IconName } from "../../lib/icons";
  import { attempt } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";

  let { id, folder }: { id: string; folder: Exclude<Folder, "screenshots"> } = $props();

  const ICON: Record<typeof folder, IconName> = { mods: "mods", resourcepacks: "resourcepack", shaderpacks: "shader" };
  const EMPTY: Record<typeof folder, string> = {
    mods: "Модов нет. В ванильной сборке они и не загрузятся: Fabric и Forge появятся на этапе 4, каталог Modrinth — на этапе 5.",
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

  async function toggle(f: FileEntry) {
    await attempt("Мод", () => api.setEnabled(id, folder, f.file, !f.enabled));
    load();
  }
</script>

<div class="bar">
  <span class="muted small grow">
    {#if list?.length}{list.length} · {list.filter((f) => f.enabled).length} включено{/if}
  </span>
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
