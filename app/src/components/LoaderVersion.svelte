<script lang="ts">
  // Версия загрузчика для версии игры: список у ядра, по умолчанию —
  // рекомендованная (у Forge — «recommended», у Fabric и NeoForge — последняя стабильная).
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type Loader, type LoaderVersion } from "../lib/api";
  import { loaderTitle } from "../lib/loaders";
  import { inTauri } from "../lib/tauri";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";

  let {
    loader,
    game,
    value = $bindable(),
  }: { loader: Exclude<Loader, "vanilla">; game: string; value: string | null } = $props();

  let list = $state<LoaderVersion[] | null>(null);
  let error = $state("");

  $effect(() => {
    const [l, g] = [loader, game];
    list = null;
    error = "";
    let stale = false;
    api
      .loaderVersions(l, g)
      .then((versions) => {
        if (stale) return;
        list = versions;
        // Выбранной версии нет в списке (сменили игру) — берём рекомендованную.
        if (!versions.some((v) => v.version === value)) {
          value = versions.find((v) => v.recommended)?.version ?? versions[0]?.version ?? null;
        }
      })
      .catch((e) => !stale && (error = String(e)));
    return () => (stale = true);
  });

  const options = $derived(
    (list ?? []).slice(0, 150).map((v) => ({
      value: v.version as string | null,
      label: `${v.version}${v.recommended ? " — рекомендуемая" : ""}${v.stable ? "" : " (бета)"}`,
    })),
  );

  function support() {
    const url = "https://www.patreon.com/LexManos/";
    if (inTauri) openUrl(url);
    else window.open(url);
  }
</script>

<div class="row">
  <span class="label">Версия {loaderTitle(loader)}</span>
  {#if error}
    <p class="warn"><Icon name="error" size={16} />Не удалось получить список: {error}</p>
  {:else if list === null}
    <span class="muted small">Загружаю список…</span>
  {:else if list.length === 0}
    <span class="muted small">Для {game} {loaderTitle(loader)} нет — выберите другую версию игры.</span>
  {:else}
    <div class="select"><Select label="Версия {loaderTitle(loader)}" {value} {options} onchange={(v) => (value = v)} /></div>
  {/if}
</div>
{#if loader === "forge"}
  <p class="muted small">
    Forge живёт на рекламе со своего сайта и просит тех, кто ставит его не вручную, поддержать проект.
    <button class="link" onclick={support}>Поддержать Forge</button>
  </p>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: var(--s3);
    min-height: var(--h-md);
  }
  .label {
    min-width: 110px;
    font-size: var(--fs-14);
    font-weight: 600;
    white-space: nowrap;
  }
  .select {
    width: 280px;
  }
  .warn,
  p {
    margin: 0;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-decoration: underline;
    cursor: pointer;
  }
</style>
