<script lang="ts">
  // Настройки → Игра: что делать с окном во время игры, память, аргументы JVM.
  import { api, type GameSettings } from "../../lib/api";
  import { attempt } from "../../lib/toast.svelte";
  import MemoryPicker from "../MemoryPicker.svelte";

  let game: GameSettings | null = $state(null);
  let memory: { total_mb: number | null; auto_mb: number } | null = $state(null);
  let jvm = $state("");
  api.config().then((c) => ((game = c.game), (jvm = c.game.jvm_args)));
  api.systemMemory().then((m) => (memory = m));

  const ON_LAUNCH: { id: GameSettings["on_launch"]; title: string; hint: string }[] = [
    { id: "close", title: "Закрывать", hint: "Остаётся только фоновый процесс (около 10 МБ), окно вернётся после игры" },
    { id: "minimize", title: "Сворачивать", hint: "Окно в панели задач, но занимает память (около 100 МБ)" },
    { id: "keep", title: "Оставлять открытым", hint: "Удобно для отладки сборок" },
  ];

  async function save(patch: Partial<GameSettings>) {
    if (!game) return;
    const next = { ...game, ...patch };
    const c = await attempt("Настройки игры", () => api.setGame(next));
    if (c) game = c.game;
  }
</script>

{#if game && memory}
  <div class="card block">
    <h2>Окно лаунчера во время игры</h2>
    {#each ON_LAUNCH as o (o.id)}
      <label class="opt">
        <input type="radio" name="on-launch" checked={game.on_launch === o.id} onchange={() => save({ on_launch: o.id })} />
        <span><b>{o.title}</b><span class="muted small">{o.hint}</span></span>
      </label>
    {/each}
  </div>

  <div class="card block">
    <h2>Память для игры</h2>
    <MemoryPicker
      value={game.memory_mb}
      inherit="Авто — {memory.auto_mb} МБ (четверть ОЗУ, от 1 до 4 ГБ)"
      total={memory.total_mb}
      onchange={(v) => save({ memory_mb: v })}
    />
    <p class="muted small">В настройках сборки можно задать свою память для конкретной сборки.</p>
  </div>

  <div class="card block">
    <h2>Аргументы JVM</h2>
    <input
      class="input mono"
      bind:value={jvm}
      onchange={() => save({ jvm_args: jvm })}
      placeholder="Например: -XX:+UseG1GC"
      spellcheck="false"
    />
    <p class="muted small">Для всех сборок, через пробел. Без нужды лучше не трогать.</p>
  </div>
  <div class="card block">
    <h2>Для опытных</h2>
    <label class="opt">
      <input type="checkbox" checked={!!game.log_window} onchange={(e) => save({ log_window: e.currentTarget.checked })} />
      <span>
        <b>Показывать лог игры, пока она идёт</b>
        <span class="muted small"
          >Окно лаунчера при запуске не закрывается (выбор «Окно лаунчера во время игры» на это время не действует), внизу —
          живой вывод игры: загрузка модов, предупреждения, ошибки. Открытое окно занимает около 100 МБ памяти.</span
        >
      </span>
    </label>
  </div>
{/if}

<style>
  /* Те же отступы, что у карточек в Settings.svelte. */
  .mono {
    font-family: ui-monospace, Consolas, monospace;
  }
  p {
    margin: 0;
  }
</style>
