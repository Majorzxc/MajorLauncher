<script lang="ts">
  // «Настройки → Загрузки» (раздел 5.16 ТЗ): сколько файлов качать сразу,
  // ограничение скорости, запасные зеркала, прокси. Действует сразу.
  import { api, type DownloadSettings } from "../../lib/api";
  import { attempt, toast } from "../../lib/toast.svelte";
  import Select from "../Select.svelte";

  let opts = $state<DownloadSettings | null>(null);
  let proxy = $state("");
  let limit = $state("");
  api.config().then((c) => {
    opts = c.downloads ?? { parallel: 16, mirrors: true, limit_kbps: 0, proxy: "" };
    proxy = opts.proxy;
    limit = opts.limit_kbps ? String(Math.round(opts.limit_kbps / 1024 * 10) / 10) : "";
  });

  async function set(patch: Partial<DownloadSettings>): Promise<boolean> {
    if (!opts) return false;
    const saved = await attempt("Загрузки", () => api.setDownloads({ ...opts!, ...patch }));
    if (saved?.downloads) opts = saved.downloads;
    return !!saved;
  }

  const PARALLEL = [2, 4, 8, 16, 32].map((n) => ({ value: n, label: `${n}${n === 16 ? " — обычно" : ""}` }));

  // Скорость — в МБ/с, ядро хранит КБ/с. Пусто или 0 — без ограничения.
  async function setLimit() {
    const mb = Number(limit.replace(",", ".") || 0);
    if (!Number.isFinite(mb) || mb < 0) return toast("Скорость — число МБ/с, например 2 или 0,5", { error: true });
    await set({ limit_kbps: Math.round(mb * 1024) });
  }

  async function setProxy() {
    if (!(await set({ proxy: proxy.trim() }))) proxy = opts?.proxy ?? "";
  }
</script>

{#if opts}
  <div class="card block">
    <div class="line">
      <span class="what"><b>Одновременных загрузок</b><span class="muted small">Меньше — легче слабому интернету и роутеру.</span></span>
      <Select label="Одновременных загрузок" value={opts.parallel} options={PARALLEL} onchange={(v) => set({ parallel: v })} />
    </div>
    <div class="line">
      <span class="what">
        <b>Ограничение скорости, МБ/с</b>
        <span class="muted small">Пусто или 0 — без ограничения. Пригодится, чтобы загрузка не мешала игре или звонку.</span>
      </span>
      <input class="input short" bind:value={limit} onchange={setLimit} inputmode="decimal" placeholder="нет" />
    </div>
    <label class="opt">
      <input type="checkbox" checked={opts.mirrors} onchange={(e) => set({ mirrors: e.currentTarget.checked })} />
      <span>
        <b>Запасные зеркала</b>
        <span class="muted small">
          Если серверы Mojang, Forge или Fabric не отвечают, ассеты и библиотеки качаются с зеркала BMCLAPI — всегда с
          проверкой хеша из официальных данных, подменённый файл не пройдёт.
        </span>
      </span>
    </label>
  </div>
  <div class="card block">
    <div class="line">
      <span class="what">
        <b>Прокси</b>
        <span class="muted small">Адрес вида http://адрес:порт. Пусто — напрямую. Через прокси идут загрузки и запросы к Modrinth, Ely.by и GitHub.</span>
      </span>
      <input class="input mono" bind:value={proxy} onchange={setProxy} placeholder="напрямую" spellcheck="false" />
    </div>
  </div>
{/if}

<style>
  .line {
    display: flex;
    align-items: center;
    gap: var(--s4);
  }
  .what {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .short {
    width: 96px;
  }
  .mono {
    width: 260px;
    font-family: Consolas, monospace;
  }
</style>
