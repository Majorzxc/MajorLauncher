<script lang="ts">
  // «Настройки → Оптимизация» (раздел 5.7 ТЗ): что делает кнопка
  // «Оптимизировать». У каждого пункта — зачем он нужен.
  import { api, type Hardware, type OptimizeSettings } from "../lib/api";
  import { attempt } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";

  let opts = $state<OptimizeSettings | null>(null);
  let hw = $state<Hardware | null>(null);
  api.config().then((c) => (opts = c.optimize ?? null));
  api.hardwareInfo().then((h) => (hw = h)).catch(() => {});

  type Key = Exclude<keyof OptimizeSettings, "intro_seen">;
  const ITEMS: { key: Key; title: string; why: string; soon?: boolean }[] = [
    {
      key: "hardware",
      title: "Определить железо",
      why: "Память, процессор и видеокарты — от них зависят память для игры и настройки графики.",
    },
    {
      key: "mods",
      title: "Поставить оптимизаторы",
      why: "Sodium (Embeddium на Forge), Lithium, FerriteCore, ModernFix, ImmediatelyFast, Entity Culling, Dynamic FPS — только совместимые с этой сборкой и её модами. Уже стоящие не трогаются, с OptiFine Sodium не ставится.",
    },
    {
      key: "memory",
      title: "Подобрать память",
      why: "По числу модов и объёму памяти ПК: 2 ГБ и по 25 МБ на мод, не больше 8 ГБ и с запасом Windows. Записывается в настройки сборки.",
    },
    {
      key: "gpu",
      title: "Ноутбуки: игра на отдельной видеокарте",
      why: "Если видеокарт две, Windows часто запускает Java на встроенной — FPS в разы ниже. Лаунчер ставит Java на отдельную, как «Параметры Windows → Графика».",
    },
    {
      key: "system",
      title: "Проверить систему",
      why: "Режим питания «Экономия энергии», старый драйвер видеокарты. Это лаунчер не меняет сам — только подскажет.",
    },
    {
      key: "graphics",
      title: "Настроить графику под железо",
      why: "Дальность прорисовки, качество графики, частицы, тени. Выключено по умолчанию: меняет привычные вам настройки.",
    },
    {
      key: "fps",
      title: "Замер FPS «до и после»",
      why: "Через мод-помощник, занимает пару минут.",
      soon: true,
    },
  ];

  async function set(key: Key, value: boolean) {
    if (!opts) return;
    const next = { ...opts, [key]: value };
    const saved = await attempt("Оптимизация", () => api.setOptimize(next));
    if (saved) opts = saved.optimize ?? next;
  }

  const gb = (mb: number | null) => (mb === null ? "?" : `${(mb / 1024).toFixed(0)} ГБ`);
</script>

<div class="card block">
  <p class="muted">
    Кнопка «Оптимизировать» — на странице сборки и в её меню. Перед оптимизацией лаунчер всегда сохраняет снапшот
    сборки, поэтому всё откатывается одним нажатием. После — отчёт: что сделано, что пропущено и почему.
  </p>
  {#if hw}
    <p class="small hw">
      <Icon name="cpu" size={16} />
      <span>
        {hw.cpu ?? "Процессор"} · {hw.cores} потоков · память {gb(hw.ram_total_mb)}
        {#each hw.gpus as g (g.name)}<br />{g.name}{g.discrete ? "" : " (встроенная)"}{#if g.driver_date}<span class="muted"
              >{" · драйвер от "}{g.driver_date}</span
            >{/if}{/each}
        {#if hw.laptop}<br />Ноутбук{/if}
      </span>
    </p>
  {/if}
</div>

{#if opts}
  <div class="card block">
    {#each ITEMS as it (it.key)}
      <label class="opt" class:soon={it.soon}>
        <input type="checkbox" checked={opts[it.key]} disabled={it.soon} onchange={(e) => set(it.key, e.currentTarget.checked)} />
        <span>
          <b>{it.title}</b>{#if it.soon}<span class="muted small"> · появится позже</span>{/if}
          <span class="muted small why">{it.why}</span>
        </span>
      </label>
    {/each}
  </div>
{/if}

<style>
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--s4);
    padding: var(--s5);
    margin-bottom: var(--s4);
    max-width: 760px;
  }
  .block p {
    margin: 0;
  }
  .hw {
    display: flex;
    align-items: flex-start;
    gap: var(--s2);
    line-height: 1.6;
  }
  .opt {
    display: flex;
    align-items: flex-start;
    gap: var(--s3);
    cursor: pointer;
  }
  .opt input {
    margin-top: 4px;
  }
  .opt span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .why {
    display: block;
  }
  .soon {
    cursor: default;
    opacity: 0.6;
  }
</style>
