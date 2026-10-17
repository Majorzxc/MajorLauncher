<script lang="ts">
  // Сборка из файла .mrpack (Modrinth, Prism, Modrinth App): что внутри — до
  // установки; название можно поменять.
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type PackInfo } from "../lib/api";
  import { refresh } from "../lib/builds.svelte";
  import { size } from "../lib/format";
  import { loaderTitle } from "../lib/loaders";
  import { openBuild } from "../lib/nav.svelte";
  import { inTauri } from "../lib/tauri";
  import { toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let path = $state<string | null>(null);
  let info = $state<PackInfo | null>(null);
  let name = $state("");
  let busy = $state(false);

  export async function pick() {
    if (!inTauri) return toast("В браузере файлы не выбрать", { error: true });
    const picked = await open({ title: "Сборка .mrpack", filters: [{ name: "Сборка Modrinth", extensions: ["mrpack"] }] });
    if (typeof picked !== "string") return;
    try {
      info = await api.mrpackInspect(picked);
      path = picked;
      name = info.name;
    } catch (e) {
      toast(String(e), { error: true });
    }
  }

  async function run() {
    if (!path || !info) return;
    busy = true;
    const file = path;
    const title = name.trim() || info.name;
    path = null;
    info = null;
    toast(`Ставлю «${title}» — ход виден в «Загрузках»`);
    try {
      const b = await api.mrpackImport(file, title);
      await refresh();
      toast(`Сборка «${b.name}» готова. Игра и загрузчик скачаются при первом запуске.`);
      openBuild(b.id, "mods");
    } catch (e) {
      toast(`Не удалось поставить сборку: ${e}`, { error: true });
    } finally {
      busy = false;
    }
  }
</script>

{#if info}
  <Modal title="Сборка из файла" width={480} onclose={() => ((info = null), (path = null))}>
    <label class="field">
      <span>Название</span>
      <input type="search" class="input" bind:value={name} maxlength="64" />
    </label>
    <p class="muted">
      {info.game} · {loaderTitle(info.loader)}{info.loader_version ? ` ${info.loader_version}` : ""} · файлов {info.files}{info.size ? `, ${size(info.size)}` : ""}
    </p>
    <p class="muted small">Моды лягут в библиотеку, настройки модов — в сборку. Файлы качаются только с Modrinth, GitHub и GitLab, каждый сверяется по хешу.</p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => ((info = null), (path = null))}>Отмена</button>
      <button class="btn btn-primary" disabled={busy} onclick={run}><Icon name="download" size={16} />Поставить</button>
    {/snippet}
  </Modal>
{/if}

<style>
  p {
    margin: var(--s3) 0 0;
  }
</style>
