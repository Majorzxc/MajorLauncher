<script lang="ts">
  // Сборка из каталога Modrinth: какая версия стоит и «Обновить сборку».
  // Карточка видна, только если лаунчер знает, откуда сборка.
  import { api, type Entry, type PackInfo } from "../lib/api";
  import { refresh } from "../lib/builds.svelte";
  import { play } from "../lib/play.svelte";
  import { toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let { entry }: { entry: Entry } = $props();
  let info = $state<PackInfo | null>(null);
  let error = $state("");
  let asking = $state(false);
  let working = $state(false);

  async function load() {
    try {
      info = await api.packInfo(entry.id);
      error = "";
    } catch (e) {
      error = String(e);
    }
  }
  load();

  const playing = $derived(play.status.phase !== "idle" && play.status.build === entry.id);

  async function update() {
    const v = info!.update!;
    asking = false;
    working = true;
    try {
      await api.packUpdate(entry.id, v.version_id);
      toast(`Сборка обновлена до ${v.version_number}`);
      refresh();
      load();
    } catch (e) {
      error = String(e);
    } finally {
      working = false;
    }
  }
</script>

{#if info?.origin}
  {@const o = info.origin}
  <div class="card block">
    <h3>Сборка из каталога</h3>
    <p class="muted small">
      {o.title ?? "Сборка Modrinth"}{o.version_number ? ` · версия ${o.version_number}` : ""}. Лаунчер следит за её
      обновлениями (настраивается в «Настройки → Обновления»).
    </p>
    {#if info.update}
      <p class="small">
        <Icon name="info" size={16} />Вышла версия <b>{info.update.version_number}</b>.
      </p>
      <div>
        <button class="btn btn-primary" disabled={working || playing} onclick={() => (asking = true)}>
          <Icon name="download" size={18} />{working ? "Обновляю…" : `Обновить до ${info.update.version_number}`}
        </button>
      </div>
    {:else}
      <p class="muted small">Стоит последняя версия.</p>
    {/if}
    {#if error}<p class="bad"><Icon name="error" size={16} />{error}</p>{/if}
  </div>
{/if}

{#if asking && info?.update}
  <Modal title="Обновить сборку?" width={520} onclose={() => (asking = false)}>
    <p>«{entry.name}» обновится до версии {info.update.version_number} из каталога Modrinth.</p>
    <ul class="what">
      <li>моды сборки — новые версии, убранные автором — уйдут</li>
      <li>добавленные вами моды останутся, выключенные — останутся выключенными</li>
      <li>настройки модов из сборки поставятся заново</li>
      <li>миры не меняются</li>
    </ul>
    <p class="muted small">Перед обновлением сохранится снапшот — всё можно откатить на вкладке «Снапшоты».</p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (asking = false)}>Отмена</button>
      <button class="btn btn-primary" onclick={update}>Обновить</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
    padding: var(--s5);
    margin-bottom: var(--s4);
    max-width: 720px;
  }
  h3 {
    margin: 0;
    font-size: var(--fs-16);
  }
  p {
    margin: 0;
  }
  .small :global(svg) {
    vertical-align: -3px;
    margin-right: 4px;
  }
  .what {
    margin: var(--s3) 0;
    padding-left: var(--s5);
  }
  .bad {
    color: var(--danger);
  }
</style>
