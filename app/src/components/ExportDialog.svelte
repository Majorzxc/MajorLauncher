<script lang="ts">
  // Экспорт сборки в .mrpack (раздел 5.5 ТЗ): моды с Modrinth — ссылками,
  // свои файлы и настройки — внутрь файла. Файл открывают Prism, Modrinth App
  // и MajorLauncher («Сборки → Из файла .mrpack»).
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, type Entry, type ExportPreview } from "../lib/api";
  import { count, plural, size } from "../lib/format";
  import { loaderLabel } from "../lib/loaders";
  import { inTauri } from "../lib/tauri";
  import { toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let { entry, onclose }: { entry: Entry; onclose: () => void } = $props();

  let preview = $state<ExportPreview | null>(null);
  let error = $state("");
  // svelte-ignore state_referenced_locally
  let name = $state(entry.name);
  let version = $state("1.0");
  let summary = $state("");
  let configs = $state(true);
  let packs = $state(true);
  let options = $state(false);
  let disabled = $state(false);
  let working = $state(false);

  // svelte-ignore state_referenced_locally
  api
    .exportPreview(entry.id)
    .then((p) => (preview = p))
    .catch((e) => (error = String(e)));

  const mods = (n: number) => `${count(n)} ${plural(n, ["мод", "мода", "модов"])}`;

  async function run() {
    const file = (name.trim() || entry.name).replace(/[\\/:*?"<>|]+/g, " ").trim();
    const path = inTauri
      ? await save({
          title: "Сохранить сборку",
          defaultPath: `${file} ${version.trim()}.mrpack`,
          filters: [{ name: "Сборка Modrinth", extensions: ["mrpack"] }],
        })
      : `D:\\${file}.mrpack`;
    if (!path) return;
    working = true;
    error = "";
    try {
      const r = await api.exportBuild(entry.id, { name, version, summary, configs, packs, options, disabled }, path);
      toast(`Сборка сохранена: ${r.path} (${size(r.size)})`);
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      working = false;
    }
  }
</script>

<Modal title="Экспорт сборки «{entry.name}»" width={600} closable={!working} {onclose}>
  <p class="muted small">
    Файл .mrpack открывают Prism Launcher, Modrinth App и MajorLauncher. Моды с Modrinth попадают в него ссылкой с
    хешами — файл весит немного, а сборка у друга получится точно такой же.
  </p>
  {#if !preview && !error}
    <p class="muted small"><span class="spin"></span>Смотрю, какие моды есть на Modrinth…</p>
  {/if}
  {#if preview}
    <p>
      {preview.game}{#if preview.loader !== "vanilla"}{" · "}{loaderLabel(preview)}{/if} · {mods(
        preview.modrinth + preview.own.length,
      )}
    </p>
    <div class="grid">
      <label class="field"><span>Название</span><input class="input" bind:value={name} maxlength="64" /></label>
      <label class="field small-field"><span>Версия сборки</span><input class="input" bind:value={version} maxlength="16" /></label>
    </div>
    <label class="field"
      ><span>Описание <span class="muted">(необязательно)</span></span><input class="input" bind:value={summary} maxlength="200" /></label
    >
    <div class="opts">
      <label class="opt"
        ><input type="checkbox" bind:checked={configs} />Настройки модов<span class="muted small"
          >{preview.config_size ? `— ${size(preview.config_size)}` : "— папки config нет"}</span
        ></label
      >
      <label class="opt"
        ><input type="checkbox" bind:checked={packs} />Ресурспаки и шейдеры сборки<span class="muted small"
          >— {count(preview.packs)}</span
        ></label
      >
      <label class="opt"
        ><input type="checkbox" bind:checked={options} disabled={!preview.has_options} />Настройки игры (клавиши, звук,
        графика)<span class="muted small">— у друга заменят его собственные</span></label
      >
      {#if preview.disabled}
        <label class="opt"
          ><input type="checkbox" bind:checked={disabled} />Выключенные моды<span class="muted small"
            >— {count(preview.disabled)}, останутся выключенными</span
          ></label
        >
      {/if}
    </div>
    {#if preview.own.length}
      <p class="warn-text small">
        <Icon name="warning" size={16} />
        <span>
          {mods(preview.own.length)} не с Modrinth — попадут в файл целиком: {preview.own.join(", ")}. Выкладывать
          чужие моды в открытый доступ можно не всегда — для друга это нормально.
        </span>
      </p>
    {/if}
    {#if preview.offline}
      <p class="warn-text small">
        <Icon name="warning" size={16} />Modrinth не ответил — без него моды не превратить в ссылки. Попробуйте позже.
      </p>
    {/if}
  {/if}
  {#if error}<p class="bad"><Icon name="error" size={18} />{error}</p>{/if}

  {#snippet footer()}
    <button class="btn btn-quiet" disabled={working} onclick={onclose}>Отмена</button>
    <button class="btn btn-primary" disabled={!preview || working || !name.trim()} onclick={run}>
      <Icon name="export" size={18} />{working ? "Сохраняю…" : "Сохранить .mrpack…"}
    </button>
  {/snippet}
</Modal>

<style>
  p {
    margin: 0 0 var(--s3);
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 140px;
    gap: var(--s3);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: var(--s3);
    font-size: var(--fs-14);
    font-weight: 600;
  }
  .opts {
    display: flex;
    flex-direction: column;
    gap: var(--s2);
    margin-bottom: var(--s3);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    cursor: pointer;
  }
  .opt .muted {
    margin-left: 2px;
  }
  .warn-text {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    color: var(--warning);
  }
  .bad {
    display: flex;
    gap: 6px;
    color: var(--danger);
  }
  .spin {
    display: inline-block;
    width: 12px;
    height: 12px;
    margin-right: 8px;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    vertical-align: -1px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
