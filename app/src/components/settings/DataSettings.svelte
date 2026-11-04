<script lang="ts">
  // «Настройки → Папка данных»: где лежит всё, перенос на другой диск,
  // снапшоты сборок и корзина (раздел 5.16 ТЗ).
  import { api, type StorageSettings } from "../../lib/api";
  import { size } from "../../lib/format";
  import { attempt } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";
  import DataMove from "./DataMove.svelte";

  const inspect = () => api.config().then((c) => (c.data_dir ? api.inspectDir(c.data_dir) : null));
  let dataDir = $state(inspect());
  let error = $state("");

  let storage = $state<StorageSettings | null>(null);
  api.config().then((c) => (storage = c.storage ?? { auto_snapshots: true, keep_snapshots: 20, trash_days: 30 }));

  async function set<K extends keyof StorageSettings>(key: K, value: StorageSettings[K]) {
    if (!storage) return;
    const saved = await attempt("Хранение", () => api.setStorage({ ...storage!, [key]: value }));
    if (saved?.storage) storage = saved.storage;
  }

  // Число из поля: пустое или не число — оставить как было.
  function number(e: Event, min: number, max: number): number | null {
    const n = Math.round(Number((e.currentTarget as HTMLInputElement).value));
    return Number.isFinite(n) && n >= min && n <= max ? n : null;
  }
</script>

<div class="card block">
  <p class="muted">Здесь лежат версии игры, Java, сборки с мирами и модами, аккаунты (без паролей).</p>
  {#await dataDir then d}
    {#if d}
      <code class="path">{d.path}</code>
      {#if d.free_bytes !== null}<p class="muted">Свободно на диске: {size(d.free_bytes)}</p>{/if}
      {#each d.warnings as w}<p class="warn"><Icon name="warning" size={18} />{w}</p>{/each}
    {/if}
  {/await}
  <div>
    <button class="btn" onclick={() => api.openDataDir().catch((e) => (error = String(e)))}>
      <Icon name="folder" size={18} /> Открыть в проводнике
    </button>
  </div>
  {#if error}<p class="warn"><Icon name="error" size={18} />{error}</p>{/if}
</div>

<DataMove onmoved={() => (dataDir = inspect())} />

{#if storage}
  <div class="card block">
    <h2>Снапшоты сборок</h2>
    <label class="opt">
      <input type="checkbox" checked={storage.auto_snapshots} onchange={(e) => set("auto_snapshots", e.currentTarget.checked)} />
      <span>
        <b>Сохранять снапшот перед каждым изменением сборки</b>
        <span class="muted small">
          Перед установкой и обновлением модов, переводом на другую версию, оптимизацией. Моды в снапшоте — жёсткими
          ссылками, поэтому места он почти не занимает. Без снапшота откатить изменение будет нельзя.
        </span>
      </span>
    </label>
    <label class="num" class:off={!storage.auto_snapshots}>
      <span>Хранить автоматических снапшотов на сборку</span>
      <input
        class="input"
        type="number"
        min="1"
        max="200"
        value={storage.keep_snapshots}
        disabled={!storage.auto_snapshots}
        onchange={(e) => {
          const n = number(e, 1, 200);
          if (n) set("keep_snapshots", n);
          else e.currentTarget.value = String(storage!.keep_snapshots);
        }}
      />
    </label>
    <p class="muted small">Сделанные вручную снапшоты сами не удаляются.</p>
  </div>

  <div class="card block">
    <h2>Корзина</h2>
    <label class="num">
      <span>Удалённая сборка хранится в корзине, дней</span>
      <input
        class="input"
        type="number"
        min="1"
        max="365"
        value={storage.trash_days}
        onchange={(e) => {
          const n = number(e, 1, 365);
          if (n) set("trash_days", n);
          else e.currentTarget.value = String(storage!.trash_days);
        }}
      />
    </label>
    <p class="muted small">Потом лаунчер удалит её вместе с мирами сам. Вернуть сборку раньше — «Сборки → Корзина».</p>
  </div>
{/if}

<style>
  .path {
    padding: var(--s2) var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--bg);
    font-family: Consolas, monospace;
    user-select: text;
  }
  .num {
    display: flex;
    align-items: center;
    gap: var(--s3);
  }
  .num span {
    flex: 1;
  }
  .num input {
    width: 96px;
  }
</style>
