<script lang="ts">
  // «Оптимизировать»: в первый раз — что сделает кнопка и где её настроить;
  // дальше — работа и отчёт по шагам с «Откатить» (снапшот перед оптимизацией).
  import { api, type OptimizeReport, type OptimizeSettings } from "../lib/api";
  import { refresh } from "../lib/builds.svelte";
  import { openBuild, openSettings, go } from "../lib/nav.svelte";
  import { attempt, toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let { id, name, onclose }: { id: string; name: string; onclose: () => void } = $props();

  let opts = $state<OptimizeSettings | null>(null);
  let phase = $state<"intro" | "running" | "done">("running");
  let report = $state<OptimizeReport | null>(null);
  let error = $state("");

  const LABELS: Record<string, string> = {
    hardware: "определит железо",
    mods: "поставит совместимые оптимизаторы",
    memory: "подберёт память под моды",
    gpu: "на ноутбуке переведёт игру на отдельную видеокарту",
    system: "проверит режим питания и драйвер",
    graphics: "настроит графику под железо",
  };
  const enabled = $derived(opts ? Object.keys(LABELS).filter((k) => opts![k as keyof OptimizeSettings]) : []);

  api.config().then((c) => {
    opts = c.optimize ?? null;
    if (opts && !opts.intro_seen) phase = "intro";
    else run();
  });

  async function run() {
    phase = "running";
    error = "";
    if (opts && !opts.intro_seen) {
      await api.setOptimize({ ...opts, intro_seen: true }).catch(() => {});
    }
    try {
      report = await api.optimizeBuild(id);
      phase = "done";
      refresh();
    } catch (e) {
      error = String(e);
      phase = "done";
    }
  }

  async function rollback() {
    const snap = report!.snapshot;
    const ok = await attempt("Откат", () => api.snapshotRestore(id, snap).then(() => true));
    if (ok) {
      toast("Оптимизация отменена — сборка как была");
      refresh();
      onclose();
    }
  }

  function settings() {
    onclose();
    go("settings");
    openSettings("optimize");
  }

  const ICON = { done: "check", skipped: "minus", info: "info", failed: "error" } as const;
  const changed = $derived(report?.steps.some((s) => s.state === "done") ?? false);
</script>

<Modal title="Оптимизация «{name}»" width={640} closable={phase !== "running"} {onclose}>
  {#if phase === "intro"}
    <p>Кнопка сделает сборку быстрее в один клик. По вашим настройкам лаунчер:</p>
    <ul class="what">
      {#each enabled as k (k)}<li>{LABELS[k]}</li>{/each}
    </ul>
    <p class="muted small">
      Перед этим всегда сохраняется снапшот — всё можно откатить одним нажатием. Что именно делает кнопка, меняется в
      «Настройки → Оптимизация». Это окно больше не появится.
    </p>
  {:else if phase === "running"}
    <p class="muted"><span class="spin"></span>Оптимизирую… Подробности — в «Загрузках».</p>
  {:else}
    {#if error}<p class="bad"><Icon name="error" size={18} />{error}</p>{/if}
    {#if report}
      <ul class="steps">
        {#each report.steps as s, i (i)}
          <li class={s.state}>
            <Icon name={ICON[s.state]} size={16} />
            <span><b>{s.title}</b> — {s.text}</span>
          </li>
        {/each}
      </ul>
      <p class="muted small">
        {changed
          ? "Изменения применятся при следующем запуске. Не понравилось — «Откатить»: сборка вернётся к снапшоту «перед оптимизацией»."
          : "Менять было нечего — сборка уже оптимизирована."}
      </p>
    {/if}
  {/if}

  {#snippet footer()}
    {#if phase === "intro"}
      <button class="btn btn-quiet back" onclick={settings}>Настроить…</button>
      <button class="btn btn-quiet" onclick={onclose}>Отмена</button>
      <button class="btn btn-primary" onclick={run}>Оптимизировать</button>
    {:else if phase === "done"}
      {#if report && changed}
        <button class="btn back" onclick={rollback}><Icon name="restore" size={18} />Откатить</button>
      {/if}
      {#if report?.steps.some((s) => s.title === "Память" && s.state === "done")}
        <button class="btn btn-quiet" onclick={() => (onclose(), openBuild(id, "settings"))}>Настройки сборки</button>
      {/if}
      <button class="btn btn-primary" onclick={onclose}>Готово</button>
    {/if}
  {/snippet}
</Modal>

<style>
  p {
    margin: 0 0 var(--s3);
  }
  .what {
    margin: 0 0 var(--s3);
    padding-left: var(--s5);
  }
  .steps {
    display: flex;
    flex-direction: column;
    gap: var(--s2);
    margin: 0 0 var(--s4);
    padding: 0;
    list-style: none;
  }
  .steps li {
    display: flex;
    align-items: flex-start;
    gap: var(--s2);
    line-height: 1.45;
  }
  .steps li :global(svg) {
    flex: none;
    margin-top: 3px;
  }
  .done :global(svg) {
    color: var(--accent);
  }
  .skipped {
    color: var(--text-muted);
  }
  .info :global(svg) {
    color: var(--warning);
  }
  .failed {
    color: var(--danger);
  }
  .bad {
    display: flex;
    gap: 6px;
    color: var(--danger);
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
