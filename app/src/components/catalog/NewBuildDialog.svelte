<script lang="ts">
  // «Новая сборка…» в каталоге: версия игры и загрузчик выбираются сразу,
  // а сама сборка создаётся при первой установке (раздел 5.4 ТЗ).
  import { api, type Loader, type VersionInfo } from "../../lib/api";
  import type { Pick } from "../../lib/catalog.svelte";
  import { LOADERS, loaderTitle } from "../../lib/loaders";
  import LoaderVersion from "../LoaderVersion.svelte";
  import Modal from "../Modal.svelte";
  import Select from "../Select.svelte";

  let { ondone }: { ondone: (pick: Pick | null) => void } = $props();

  let versions: VersionInfo[] = $state([]);
  let game = $state("");
  let loader = $state<Loader>("fabric");
  let loaderVersion: string | null = $state(null);
  let games: Set<string> | null = $state(null);
  let name = $state("");
  let touched = $state(false);

  api.versions(false).then((list) => {
    versions = list;
    if (!game) game = list[0]?.id ?? "";
  });

  $effect(() => {
    const l = loader;
    games = null;
    if (l === "vanilla") return;
    let stale = false;
    api.loaderGames(l).then((g) => !stale && (games = new Set(g)));
    return () => (stale = true);
  });

  // Выбранной версии для загрузчика нет — последний релиз, где он есть.
  $effect(() => {
    if (games && versions.length && !games.has(game)) game = versions.find((v) => games!.has(v.id))?.id ?? "";
  });

  $effect(() => {
    if (!touched) name = loader === "vanilla" ? game : `${game} ${loaderTitle(loader)}`;
  });

  const options = $derived(
    versions.filter((v) => !games || games.has(v.id)).map((v) => ({ value: v.id, label: v.id })),
  );
  const ready = $derived(!!game && !!name.trim() && (loader === "vanilla" || !!loaderVersion));
</script>

<Modal title="Новая сборка" width={560} onclose={() => ondone(null)}>
  <div class="form">
    <div class="loaders" role="radiogroup" aria-label="Загрузчик модов">
      {#each LOADERS as l (l.id)}
        <button class="chip" role="radio" aria-checked={loader === l.id} class:on={loader === l.id} onclick={() => (loader = l.id)}>
          {l.id === "vanilla" ? "Ванилла" : l.title}
        </button>
      {/each}
    </div>
    <div class="row">
      <span class="label">Версия игры</span>
      <div class="select"><Select label="Версия игры" value={game} {options} onchange={(v) => (game = v)} /></div>
    </div>
    {#if loader !== "vanilla" && game}
      <LoaderVersion {loader} {game} bind:value={loaderVersion} />
    {/if}
    <label class="row">
      <span class="label">Название</span>
      <input type="search" class="input grow" bind:value={name} maxlength="64" oninput={() => (touched = true)} />
    </label>
    <p class="muted small">Сборка появится, когда вы поставите в неё первый мод. Каталог сразу покажет то, что к ней подходит.</p>
  </div>
  {#snippet footer()}
    <button class="btn btn-quiet" onclick={() => ondone(null)}>Отмена</button>
    <button
      class="btn btn-primary"
      disabled={!ready}
      onclick={() => ondone({ kind: "new", name: name.trim(), game, loader, loaderVersion: loader === "vanilla" ? null : loaderVersion })}
    >
      Готово
    </button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
  }
  .loaders {
    display: flex;
    gap: var(--s2);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--s3);
  }
  .label {
    width: 110px;
    font-size: var(--fs-14);
    font-weight: 600;
  }
  .select {
    width: 280px;
  }
  .grow {
    flex: 1;
  }
  p {
    margin: 0;
  }
</style>
