<script lang="ts">
  // Перенос папки данных (раздел 5.15 ТЗ). На том же диске — переименование,
  // на другой — копия со сверкой; старую папку удаляет только игрок.
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type MovePlan, type MoveProgress, type Moved } from "../lib/api";
  import { size } from "../lib/format";
  import { inTauri } from "../lib/tauri";
  import { toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let { onmoved }: { onmoved: () => void } = $props();

  let path = $state("");
  let used = $state<number | null>(null);
  let plan = $state<MovePlan | null>(null);
  let checking = $state(false);
  let timer: ReturnType<typeof setTimeout>;

  let moving = $state(false);
  let progress = $state<MoveProgress | null>(null);
  let cancelling = $state(false);
  let error = $state("");
  let done = $state<Moved | null>(null);
  let removing = $state(false);

  api
    .dataSize()
    .then((s) => (used = s))
    .catch(() => (used = 0));

  function typed() {
    plan = null;
    clearTimeout(timer);
    if (!path.trim()) return;
    checking = true;
    timer = setTimeout(async () => {
      try {
        plan = await api.dataMovePlan(path.trim(), used ?? 0);
      } catch (e) {
        plan = { to: path.trim(), same_disk: false, free: null, error: String(e) };
      }
      checking = false;
    }, 350);
  }

  async function choose() {
    if (!inTauri) return;
    const picked = await open({ directory: true, title: "Куда перенести данные лаунчера" });
    if (typeof picked === "string") {
      path = picked;
      typed();
    }
  }

  async function start() {
    const to = plan!.to;
    error = "";
    progress = null;
    cancelling = false;
    moving = true;
    const stop = inTauri ? await listen<MoveProgress>("data-move", (e) => (progress = e.payload)) : () => {};
    try {
      const result = await api.dataMove(to);
      path = "";
      plan = null;
      onmoved();
      if (result.method === "renamed") toast("Папка данных перенесена");
      else done = result;
    } catch (e) {
      error = String(e);
    } finally {
      stop();
      moving = false;
    }
  }

  async function removeOld() {
    removing = true;
    try {
      await api.dataRemoveOld();
      toast("Старая папка удалена");
      done = null;
    } catch (e) {
      toast(`Не удалось удалить старую папку: ${e}`, { error: true });
    } finally {
      removing = false;
    }
  }

  const percent = $derived(progress && progress.total ? Math.floor((progress.done / progress.total) * 100) : 0);
  const bar = $derived(progress?.stage === "copy" ? percent : progress?.stage === "verify" ? 100 : 0);
  const stageText = $derived(
    !progress
      ? "Готовимся…"
      : progress.stage === "scan"
        ? "Считаем файлы…"
        : progress.stage === "copy"
          ? `Копируем: ${size(progress.done)} из ${size(progress.total)}`
          : "Сверяем копию с оригиналом…",
  );
</script>

<div class="card block">
  <h2>Перенести в другую папку</h2>
  <p class="muted small">
    Всё переедет вместе: версии игры, Java, сборки с мирами, библиотека, статистика.
    {#if used !== null}Сейчас занимает {size(used)}.{/if}
  </p>
  <div class="pick">
    <input
      class="input grow"
      bind:value={path}
      oninput={typed}
      placeholder="Например, E:\MajorLauncherData"
      spellcheck="false"
      aria-label="Новая папка данных"
    />
    <button class="btn" onclick={choose}><Icon name="folder" size={18} />Выбрать…</button>
  </div>
  {#if checking}
    <p class="muted small">Проверяем…</p>
  {:else if plan}
    {#if plan.to !== path.trim()}<p class="muted small">Данные окажутся в <code>{plan.to}</code></p>{/if}
    {#if plan.error}
      <p class="warn"><Icon name="warning" size={18} />{plan.error[0].toUpperCase() + plan.error.slice(1)}</p>
    {:else if plan.same_disk}
      <p class="muted small">Тот же диск: папка просто переименуется — это мгновенно.</p>
    {:else}
      <p class="muted small">
        Другой диск: данные скопируются{used !== null ? ` (${size(used)})` : ""}{plan.free !== null
          ? `, свободно ${size(plan.free)}`
          : ""}. Копия сверяется с оригиналом, старая папка остаётся, пока вы её не удалите.
      </p>
    {/if}
  {/if}
  {#if error}<p class="warn"><Icon name="error" size={18} />{error[0].toUpperCase() + error.slice(1)}</p>{/if}
  <p class="muted small">Пока идёт перенос, играть и менять сборки нельзя.</p>
  <div>
    <button class="btn btn-primary" disabled={!plan || !!plan.error || checking || moving} onclick={start}>
      Перенести
    </button>
  </div>
</div>

{#if moving}
  <Modal title="Перенос папки данных" width={480} closable={false} onclose={() => {}}>
    <div class="stack">
      <p class="small">{stageText}</p>
      <span class="track"><span style:width="{bar}%"></span></span>
      <p class="muted small">Если отменить, всё останется в старой папке как было.</p>
    </div>
    {#snippet footer()}
      <button
        class="btn"
        disabled={cancelling}
        onclick={() => {
          cancelling = true;
          api.dataMoveCancel();
        }}>{cancelling ? "Отменяем…" : "Отменить"}</button
      >
    {/snippet}
  </Modal>
{/if}

{#if done}
  <Modal title="Данные перенесены" width={520} onclose={() => (done = null)}>
    <div class="stack">
      <p>Лаунчер уже работает с новой папкой. Копия сверена с оригиналом.</p>
      <p class="muted small">
        Старая папка <code>{done.old}</code> больше не нужна{done.old_size ? ` и занимает ${size(done.old_size)}` : ""}.
        Удалить её сейчас? Можно оставить и удалить потом вручную.
      </p>
    </div>
    {#snippet footer()}
      <button class="btn btn-quiet" disabled={removing} onclick={() => (done = null)}>Оставить</button>
      <button class="btn danger-fill" disabled={removing} onclick={removeOld}>
        {removing ? "Удаляем…" : "Удалить старую папку"}
      </button>
    {/snippet}
  </Modal>
{/if}

<style>
  h2 {
    margin: 0;
    font-size: var(--fs-20);
  }
  .pick {
    display: flex;
    gap: var(--s2);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  code {
    font-size: var(--fs-14);
    word-break: break-all;
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
  }
  .track {
    display: block;
    height: 6px;
    border-radius: 3px;
    background: var(--border);
    overflow: hidden;
  }
  .track span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.1s linear;
  }
  .danger-fill {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
    padding: var(--s5);
  }
  p {
    margin: 0;
  }
</style>
