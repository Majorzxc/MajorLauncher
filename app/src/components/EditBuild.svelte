<script lang="ts">
  import { untrack } from "svelte";
  import { api, type Entry } from "../lib/api";
  import { builds, refresh } from "../lib/builds.svelte";
  import { pickCover } from "../lib/cover";
  import { attempt, toast } from "../lib/toast.svelte";
  import GroupSelect from "./GroupSelect.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let { entry, onclose }: { entry: Entry; onclose: () => void } = $props();

  // Форма правит копию: начальные значения берём один раз и дальше не следим за entry.
  const initial = untrack(() => entry);
  let name = $state(initial.name);
  let group: string | null = $state(initial.group);
  let pinned = $state(initial.pinned);
  let error = $state("");
  const cover = $derived(builds.covers[entry.id]);

  async function chooseCover() {
    const png = await attempt("Обложка", pickCover);
    if (!png) return;
    if (await attempt("Обложка", () => api.setCover(entry.id, png).then(() => true))) {
      builds.covers[entry.id] = `data:image/png;base64,${png}`;
    }
  }

  async function removeCover() {
    if (await attempt("Обложка", () => api.removeCover(entry.id).then(() => true))) {
      builds.covers[entry.id] = null;
    }
  }

  async function save() {
    error = "";
    try {
      await api.update(entry.id, { name, group, pinned });
      toast("Сохранено");
      await refresh();
      onclose();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<Modal title="Сборка" width={560} {onclose}>
  <div class="form">
    <div class="cover-row">
      <div class="cover">
        {#if cover}<img src={cover} alt="" />{:else}<span>{name.trim()[0]?.toUpperCase() ?? "?"}</span>{/if}
      </div>
      <div class="cover-actions">
        <b>Обложка</b>
        <p class="muted">Любая картинка: обрежется до квадрата и уменьшится до 256×256.</p>
        <div class="buttons">
          <button class="btn btn-sm" onclick={chooseCover}><Icon name="screenshot" size={16} />Выбрать картинку</button>
          {#if cover}<button class="btn btn-sm btn-quiet" onclick={removeCover}>Убрать</button>{/if}
        </div>
      </div>
    </div>

    <label class="field">
      <span>Название</span>
      <input
        bind:value={name}
        maxlength="64"
        onkeydown={(e) => e.key === "Enter" && save()}
      />
    </label>

    <div class="field">
      <span>Группа</span>
      <GroupSelect bind:value={group} />
    </div>

    <label class="check">
      <input type="checkbox" bind:checked={pinned} />
      <span>Закрепить сверху списка</span>
    </label>

    <p class="muted small">Версия игры: {entry.version}. Папка: instances\{entry.id}</p>
    {#if error}<p class="error"><Icon name="error" size={16} />{error}</p>{/if}
  </div>

  {#snippet footer()}
    <button class="btn btn-quiet" onclick={onclose}>Отмена</button>
    <button class="btn btn-primary" onclick={save}>Сохранить</button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--s4);
  }
  .cover-row {
    display: flex;
    gap: var(--s4);
    align-items: center;
  }
  .cover {
    width: 96px;
    height: 96px;
    flex: none;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--surface-raised);
    overflow: hidden;
    font-size: 40px;
    font-weight: 600;
    color: var(--text-muted);
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .cover-actions p {
    margin: 2px 0 var(--s2);
    font-size: var(--fs-14);
  }
  .buttons {
    display: flex;
    gap: var(--s2);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field > span {
    font-size: var(--fs-14);
    font-weight: 600;
  }
  input:not([type="checkbox"]) {
    height: var(--h-md);
    padding: 0 var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--bg);
    outline: none;
    user-select: text;
  }
  input:focus {
    border-color: var(--accent);
  }
  .check {
    display: flex;
    align-items: center;
    gap: var(--s2);
    cursor: pointer;
  }
  .check input {
    width: 18px;
    height: 18px;
    accent-color: var(--accent);
  }
  .small {
    margin: 0;
    font-size: var(--fs-12);
  }
  .error {
    display: flex;
    gap: var(--s2);
    margin: 0;
    color: var(--danger);
    font-size: var(--fs-14);
  }
</style>
