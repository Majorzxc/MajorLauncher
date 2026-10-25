<script lang="ts">
  // Перевод сборки на другую версию игры (раздел 5.5 ТЗ). Создаётся новая
  // сборка, старая остаётся как есть. Шаги: версия → моды → миры → создать.
  import { api, type Entry, type UpgradePlan, type VersionInfo } from "../lib/api";
  import { refresh } from "../lib/builds.svelte";
  import { count, plural, size } from "../lib/format";
  import { loaderLabel } from "../lib/loaders";
  import { openBuild } from "../lib/nav.svelte";
  import { toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import Select from "./Select.svelte";

  let { entry, onclose }: { entry: Entry; onclose: () => void } = $props();

  type Step = "version" | "mods" | "worlds" | "finish";
  let step = $state<Step>("version");

  // Шаг 1: версия.
  let snapshots = $state(false);
  let versions = $state<VersionInfo[]>([]);
  let version = $state<string | null>(null);
  let listError = $state("");
  $effect(() => {
    api
      .versions(snapshots)
      .then((v) => {
        versions = v;
        listError = "";
        // По умолчанию — последний релиз, если он новее сборки.
        const newest = v.find((x) => x.kind === "release");
        if (!version && newest && newest.id !== entry.version) version = newest.id;
      })
      .catch((e) => (listError = String(e)));
  });
  const options = $derived([
    { value: null as string | null, label: "Выберите версию…" },
    ...versions
      .filter((v) => v.id !== entry.version)
      .map((v) => ({ value: v.id as string | null, label: v.kind === "snapshot" ? `${v.id} · снапшот` : v.id })),
  ]);

  // Шаг 2: план — что станет с модами.
  let plan = $state<UpgradePlan | null>(null);
  let planning = $state(false);
  let planError = $state("");
  let keep = $state<Record<string, boolean>>({});
  let showAll = $state(false);
  async function makePlan() {
    if (!version) return;
    planning = true;
    planError = "";
    try {
      plan = await api.upgradePlan(entry.id, version);
      keep = {};
      name = plan.name;
      worldMode = plan.worlds.length ? "all" : "none";
      picked = Object.fromEntries(plan.worlds.map((w) => [w.folder, true]));
      step = "mods";
    } catch (e) {
      planError = String(e);
    } finally {
      planning = false;
    }
  }
  const withVersion = $derived(plan?.mods.filter((m) => m.to) ?? []);
  const without = $derived(plan?.mods.filter((m) => !m.to) ?? []);
  // Без новой версии: выключенные в сборке и так останутся выключенными.
  const askable = $derived(without.filter((m) => m.enabled));

  // Шаг 3: миры.
  let worldMode = $state<"all" | "pick" | "none">("all");
  let picked = $state<Record<string, boolean>>({});
  const chosenWorlds = $derived(
    !plan ? [] : worldMode === "all" ? plan.worlds : worldMode === "pick" ? plan.worlds.filter((w) => picked[w.folder]) : [],
  );
  const worldsSize = $derived(chosenWorlds.reduce((s, w) => s + w.size, 0));
  const allWorldsSize = $derived(plan?.worlds.reduce((s, w) => s + w.size, 0) ?? 0);

  // Шаг 4: создать.
  let name = $state("");
  let running = $state(false);
  let runError = $state("");
  async function run() {
    if (!plan) return;
    running = true;
    runError = "";
    const p = plan;
    try {
      const created = await api.upgradeRun(entry.id, {
        version: p.version,
        loader_version: p.loader_version,
        name: name.trim() || p.name,
        versions: [...p.mods.filter((m) => m.to).map((m) => m.to!.version_id), ...p.deps.map((d) => d.version_id)],
        keep_enabled: askable.filter((m) => keep[m.file]).map((m) => m.file),
        worlds: chosenWorlds.map((w) => w.folder),
      });
      toast(`Сборка «${created.name}» на ${created.version} готова, «${entry.name}» осталась как была`);
      await refresh();
      onclose();
      openBuild(created.id);
    } catch (e) {
      runError = String(e);
    } finally {
      running = false;
    }
  }

  const mods = (n: number) => `${count(n)} ${plural(n, ["мод", "мода", "модов"])}`;
  const STEPS: { id: Step; title: string }[] = [
    { id: "version", title: "Версия" },
    { id: "mods", title: "Моды" },
    { id: "worlds", title: "Миры" },
    { id: "finish", title: "Готово" },
  ];
  const index = $derived(STEPS.findIndex((s) => s.id === step));
</script>

<Modal title="Перевести «{entry.name}» на другую версию" width={720} closable={!running} {onclose}>
  <ol class="steps">
    {#each STEPS as s, i (s.id)}
      <li class:on={s.id === step} class:done={i < index}>{i + 1}. {s.title}</li>
    {/each}
  </ol>

  {#if step === "version"}
    <p>
      Лаунчер создаст <b>новую сборку</b> на выбранной версии: подберёт версии модов под неё, а моды без такой версии
      перенесёт выключенными. «{entry.name}» на {entry.version} останется как есть.
    </p>
    <div class="row">
      <div class="grow-select">
        <Select label="Версия игры" value={version} {options} onchange={(v) => (version = v)} />
      </div>
      <label class="snap muted small"><input type="checkbox" bind:checked={snapshots} />Снапшоты</label>
    </div>
    {#if listError}<p class="bad"><Icon name="error" size={18} />Список версий не загрузился: {listError}</p>{/if}
    {#if planning}
      <p class="muted small"><span class="spin"></span>Ищу версии модов под {version}… В первый раз может занять минуту.</p>
    {/if}
    {#if planError}<p class="bad"><Icon name="error" size={18} />{planError}</p>{/if}
  {:else if step === "mods" && plan}
    <p>
      {plan.from} → <b>{plan.version}</b>{#if plan.loader !== "vanilla"}{" · "}{loaderLabel({
          loader: plan.loader,
          loader_version: plan.loader_version,
        })}{/if}
    </p>
    {#if plan.mods.length === 0}
      <p class="muted">В сборке нет модов — перенесутся настройки и выбранные миры.</p>
    {:else}
      <p class="summary">
        <span class="ok"><Icon name="check" size={16} />{mods(withVersion.length)} с версией под {plan.version}</span>
        {#if without.length}<span class="warn-text"><Icon name="warning" size={16} />{mods(without.length)} без неё</span>{/if}
        {#if plan.download > 0}<span class="muted">скачать ~{size(plan.download)}</span>{/if}
      </p>
      {#if askable.length}
        <p class="muted small">
          Моды без версии под {plan.version} перенесутся выключенными, чтобы не уронить игру. Если уверены, что мод
          подойдёт (например, он не зависит от версии игры), — оставьте его включённым.
        </p>
        <ul class="list">
          {#each askable as m (m.file)}
            <li>
              <label class="keep">
                <input type="checkbox" bind:checked={keep[m.file]} />
                <span class="grow">{m.name} <span class="muted small">{m.from}</span></span>
                <span class="muted small why">{m.note}</span>
              </label>
            </li>
          {/each}
        </ul>
      {/if}
      {#if plan.deps.length}
        <p class="small">
          Новым версиям нужны ещё: {plan.deps.map((d) => d.title).join(", ")} — поставятся сами.
        </p>
      {/if}
      {#if plan.missing_deps.length}
        <p class="warn-text small">
          <Icon name="warning" size={16} />Нужны, но под {plan.version} их нет: {plan.missing_deps.join(", ")}. Моды,
          которым они нужны, могут не запуститься — лаунчер предупредит на вкладке «Моды».
        </p>
      {/if}
      <button class="link small" onclick={() => (showAll = !showAll)}>
        {showAll ? "Скрыть список модов" : "Все моды: что во что обновится"}
      </button>
      {#if showAll}
        <ul class="list all small">
          {#each plan.mods as m (m.file)}
            <li class:off={!m.enabled}>
              <span class="grow">{m.name}</span>
              <span class="muted">{m.from}</span>
              {#if m.to}
                <span>→ {m.to.same ? "подходит как есть" : m.to.version_number}</span>
              {:else}
                <span class="warn-text">→ выключится</span>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  {:else if step === "worlds" && plan}
    {#if plan.worlds.length === 0}
      <p class="muted">В сборке нет миров — переносить нечего.</p>
    {:else}
      <p>
        Мир, открытый в {plan.version}, меняется навсегда: вернуть его на {plan.from} будет нельзя. Поэтому миры не
        переносятся, а <b>копируются</b> — оригиналы остаются в «{entry.name}».
      </p>
      {#if plan.older}
        <p class="warn-text small">
          <Icon name="warning" size={16} />{plan.version} старше {plan.from}: мир из новой версии в старой может
          испортиться. Копия защитит оригинал, но копия может не открыться.
        </p>
      {/if}
      <div class="modes">
        <label class="opt">
          <input type="radio" bind:group={worldMode} value="all" />
          <span><b>Скопировать все</b> <span class="muted small">— {count(plan.worlds.length)} {plural(plan.worlds.length, ["мир", "мира", "миров"])}, займут {size(allWorldsSize)}</span></span>
        </label>
        <label class="opt">
          <input type="radio" bind:group={worldMode} value="pick" />
          <span><b>Выбрать миры</b></span>
        </label>
        {#if worldMode === "pick"}
          <ul class="list worlds">
            {#each plan.worlds as w (w.folder)}
              <li>
                <label class="keep">
                  <input type="checkbox" bind:checked={picked[w.folder]} />
                  <span class="grow">{w.name}{#if w.version}<span class="muted small">{" · "}{w.version}</span>{/if}</span>
                  <span class="muted small">{size(w.size)}</span>
                </label>
              </li>
            {/each}
          </ul>
        {/if}
        <label class="opt">
          <input type="radio" bind:group={worldMode} value="none" />
          <span><b>Не копировать</b> <span class="muted small">— новая сборка начнётся без миров</span></span>
        </label>
      </div>
    {/if}
  {:else if step === "finish" && plan}
    <label class="field">
      <span>Название новой сборки</span>
      <input class="input" bind:value={name} maxlength="64" />
    </label>
    <ul class="what">
      <li>{plan.version}{#if plan.loader !== "vanilla"}{" · "}{loaderLabel({ loader: plan.loader, loader_version: plan.loader_version })}{/if}</li>
      {#if plan.mods.length}
        <li>
          {mods(withVersion.length + plan.deps.length)} под {plan.version}{#if without.length}, {mods(without.length)} без версии — {askable.some(
                (m) => keep[m.file],
              )
              ? "часть оставлена включённой"
              : "выключены"}{/if}
        </li>
      {/if}
      <li>
        {chosenWorlds.length ? `миры: ${chosenWorlds.map((w) => w.name).join(", ")} (${size(worldsSize)})` : "без миров"}
      </li>
      <li>настройки модов, игры и сборки — копией</li>
    </ul>
    {#if plan.download > 0}<p class="muted small">Скачается около {size(plan.download)}. Прогресс — в «Загрузках».</p>{/if}
    {#if running}<p class="muted small"><span class="spin"></span>Создаю сборку… Это может занять несколько минут.</p>{/if}
    {#if runError}<p class="bad"><Icon name="error" size={18} />{runError}</p>{/if}
  {/if}

  {#snippet footer()}
    {#if step !== "version"}
      <button class="btn btn-quiet back" disabled={running} onclick={() => (step = STEPS[index - 1].id)}>Назад</button>
    {/if}
    <button class="btn btn-quiet" disabled={running} onclick={onclose}>Отмена</button>
    {#if step === "version"}
      <button class="btn btn-primary" disabled={!version || planning} onclick={makePlan}>
        {planning ? "Ищу моды…" : "Дальше"}<Icon name="chevron-right" size={18} />
      </button>
    {:else if step === "mods"}
      <button class="btn btn-primary" onclick={() => (step = "worlds")}>Дальше<Icon name="chevron-right" size={18} /></button>
    {:else if step === "worlds"}
      <button class="btn btn-primary" disabled={worldMode === "pick" && chosenWorlds.length === 0} onclick={() => (step = "finish")}>
        Дальше<Icon name="chevron-right" size={18} />
      </button>
    {:else}
      <button class="btn btn-primary" disabled={running || !name.trim()} onclick={run}>Создать сборку</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .steps {
    display: flex;
    gap: var(--s4);
    margin: 0 0 var(--s4);
    padding: 0;
    list-style: none;
    font-size: var(--fs-14);
    color: var(--text-muted);
  }
  .steps .on {
    color: var(--text);
    font-weight: 600;
  }
  .steps .done {
    color: var(--accent);
  }
  p {
    margin: 0 0 var(--s3);
  }
  .row {
    display: flex;
    align-items: flex-end;
    gap: var(--s4);
    margin-bottom: var(--s3);
  }
  .grow-select {
    flex: 1;
  }
  .snap,
  .opt,
  .keep {
    display: flex;
    align-items: center;
    gap: var(--s2);
    cursor: pointer;
  }
  .snap {
    padding-bottom: 10px;
  }
  .summary {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s4);
  }
  .summary span,
  .warn-text,
  .bad {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .warn-text {
    color: var(--warning);
  }
  .bad {
    color: var(--danger);
  }
  .list {
    max-height: 220px;
    overflow: auto;
    margin: 0 0 var(--s3);
    padding: var(--s2) var(--s3);
    list-style: none;
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--bg);
  }
  .list li {
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: 3px 0;
  }
  .list.all {
    max-height: 260px;
  }
  .list .keep {
    flex: 1;
    min-width: 0;
  }
  .list .grow {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .why {
    flex: none;
  }
  .off {
    opacity: 0.55;
  }
  .modes {
    display: flex;
    flex-direction: column;
    gap: var(--s2);
  }
  .worlds {
    margin-left: 26px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: var(--s3);
    font-size: var(--fs-14);
    font-weight: 600;
  }
  .what {
    margin: 0 0 var(--s3);
    padding-left: var(--s5);
  }
  .link {
    margin-bottom: var(--s3);
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    cursor: pointer;
  }
  .back {
    margin-right: auto;
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
