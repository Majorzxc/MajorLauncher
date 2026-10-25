<script lang="ts">
  // «Быстрая игра» (раздел 5.18 ТЗ): любая ванильная версия без создания
  // сборки. Одна общая папка: миры и настройки общие, как в официальном
  // лаунчере. Перед запуском старой версии лаунчер копирует миры из новой.
  import { api, QUICK_ID, type VersionInfo } from "../lib/api";
  import { builds } from "../lib/builds.svelte";
  import { busyLabel, start } from "../lib/play.svelte";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";

  let versions: VersionInfo[] = $state([]);
  let snapshots = $state(false);
  let chosen: string | null = $state(null);
  let checking = $state(false);
  let error = $state("");

  const quick = $derived(builds.list.find((b) => b.id === QUICK_ID));
  // По умолчанию — версия прошлой быстрой игры, иначе последний релиз.
  const version = $derived(chosen ?? quick?.version ?? versions.find((v) => v.kind === "release")?.id ?? null);

  $effect(() => {
    api
      .versions(snapshots)
      .then((v) => ((versions = v), (error = "")))
      .catch((e) => (error = String(e)));
  });

  const options = $derived.by(() => {
    const list = versions.map((v) => ({ value: v.id as string | null, label: v.kind === "snapshot" ? `${v.id} · снапшот` : v.id }));
    // Версия прошлой игры могла выпасть из списка (снапшот при выключенных снапшотах).
    if (version && !list.some((o) => o.value === version)) list.unshift({ value: version, label: version });
    return list;
  });

  const busy = $derived(busyLabel(QUICK_ID));

  async function play() {
    if (!version) return;
    checking = true;
    try {
      // Миры из более новой версии проверяет сам запуск (окно в App).
      await start(QUICK_ID, version);
    } finally {
      checking = false;
    }
  }
</script>

<section class="quick card">
  <div class="text">
    <b>Быстрая игра</b>
    <span class="muted small">Любая ванильная версия без сборки. Миры и настройки общие для всех версий.</span>
  </div>
  <label class="snap muted small">
    <input type="checkbox" bind:checked={snapshots} />Снапшоты
  </label>
  <div class="version">
    <Select label="Версия" value={version} options={options} onchange={(v) => (chosen = v)} />
  </div>
  <button class="btn btn-primary" disabled={!version || !!busy || checking} onclick={play}>
    <Icon name="play" size={18} />{busy ?? "Играть"}
  </button>
</section>
{#if error}<p class="warn"><Icon name="error" size={18} />Список версий не загрузился: {error}</p>{/if}


<style>
  .quick {
    display: flex;
    align-items: center;
    gap: var(--s4);
    padding: var(--s4) var(--s5);
    margin-top: var(--s4);
  }
  .text {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .snap {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .version {
    width: 200px;
  }
  p {
    margin: 0 0 var(--s3);
  }
</style>
