<script lang="ts">
  // Память для игры: «как обычно» (авто или общие настройки) или своя.
  let {
    value,
    inherit,
    total,
    onchange,
  }: {
    /// null — не задавать своё значение.
    value: number | null;
    /// Подпись первого варианта: «Авто — 2048 МБ».
    inherit: string;
    /// Объём ОЗУ, МБ.
    total: number | null;
    onchange: (v: number | null) => void;
  } = $props();

  // Своей памяти больше, чем ОЗУ без 1 ГБ на систему, не дать; максимум 16 ГБ.
  const max = $derived(Math.max(1024, Math.min(16384, Math.floor(((total ?? 8192) - 1024) / 512) * 512)));
  let draft = $state(0);
  $effect(() => {
    draft = value ?? 2048;
  });
</script>

<div class="memory">
  <label class="opt">
    <input type="radio" checked={value === null} onchange={() => onchange(null)} />
    {inherit}
  </label>
  <label class="opt">
    <input type="radio" checked={value !== null} onchange={() => onchange(Math.min(draft, max))} />
    Своя
  </label>
  {#if value !== null}
    <div class="custom">
      <input
        type="range"
        min="1024"
        {max}
        step="512"
        bind:value={draft}
        onchange={() => onchange(draft)}
        aria-label="Память, МБ"
      />
      <b>{(draft / 1024).toLocaleString("ru-RU", { maximumFractionDigits: 1 })} ГБ</b>
    </div>
    {#if total}<p class="muted small">ОЗУ в компьютере: {(total / 1024).toFixed(1).replace(".", ",")} ГБ. Игре не стоит давать больше половины.</p>{/if}
  {/if}
</div>

<style>
  .memory {
    display: flex;
    flex-direction: column;
    gap: var(--s2);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--s2);
    cursor: pointer;
  }
  .custom {
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding-left: 26px;
  }
  input[type="range"] {
    flex: 1;
    max-width: 360px;
    accent-color: var(--accent);
  }
  b {
    width: 64px;
    font-variant-numeric: tabular-nums;
  }
  p {
    margin: 0 0 0 26px;
  }
</style>
