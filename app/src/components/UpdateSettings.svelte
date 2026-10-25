<script lang="ts">
  // «Настройки → Обновления» (раздел 5.13 ТЗ): что лаунчер обновляет в фоне.
  import { api, type UpdateSettings } from "../lib/api";
  import { ago } from "../lib/format";

  // «в 14:05», вчера и раньше — «вчера», «3 дня назад».
  function checkedAt(unix: number): string {
    const d = new Date(unix * 1000);
    const today = new Date().toDateString() === d.toDateString();
    return today ? `в ${d.toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" })}` : ago(unix);
  }
  import { openBuild } from "../lib/nav.svelte";
  import { attempt } from "../lib/toast.svelte";
  import { checkNow, updates } from "../lib/updates.svelte";
  import Icon from "./Icon.svelte";

  let opts = $state<UpdateSettings | null>(null);
  api.config().then((c) => (opts = c.updates ?? { check: true, auto_mods: false, auto_packs: false }));

  async function set(key: keyof UpdateSettings, value: boolean) {
    if (!opts) return;
    const next = { ...opts, [key]: value };
    const saved = await attempt("Обновления", () => api.setUpdates(next));
    if (saved) opts = saved.updates ?? next;
  }
</script>

{#if opts}
  <div class="card block">
    <label class="opt">
      <input type="checkbox" checked={opts.check} onchange={(e) => set("check", e.currentTarget.checked)} />
      <span>
        <b>Проверять обновления в фоне</b>
        <span class="muted small"
          >Через минуту после запуска лаунчера и дальше раз в 6 часов: моды сборок и сами сборки из каталога. Только
          проверка у Modrinth — ничего не меняется, о новом лаунчер скажет.</span
        >
      </span>
    </label>
    <label class="opt" class:off={!opts.check}>
      <input type="checkbox" checked={opts.auto_mods} disabled={!opts.check} onchange={(e) => set("auto_mods", e.currentTarget.checked)} />
      <span>
        <b>Обновлять моды самому</b>
        <span class="muted small"
          >Перед обновлением — снапшот сборки. Закреплённые моды не трогаются, запущенная сборка — тоже. Бывает, что новая
          версия мода ломает сборку, поэтому по умолчанию выключено.</span
        >
      </span>
    </label>
    <label class="opt" class:off={!opts.check}>
      <input type="checkbox" checked={opts.auto_packs} disabled={!opts.check} onchange={(e) => set("auto_packs", e.currentTarget.checked)} />
      <span>
        <b>Обновлять сборки из каталога самому</b>
        <span class="muted small"
          >Когда автор сборки на Modrinth выпускает новую версию. Добавленные вами моды остаются, выключенные — остаются
          выключенными; настройки модов из сборки ставятся заново (прежние — в снапшоте).</span
        >
      </span>
    </label>
  </div>

  <div class="card block">
    <div class="row">
      <span class="grow muted small">
        {updates.running
          ? "Проверяю…"
          : updates.checked
            ? `Последняя проверка — ${checkedAt(updates.checked)}`
            : "Ещё не проверялось с запуска лаунчера"}
      </span>
      <button class="btn btn-sm" disabled={updates.running} onclick={checkNow}><Icon name="refresh" size={16} />Проверить сейчас</button>
    </div>
    {#if updates.list.length}
      <ul class="found">
        {#each updates.list as b (b.build)}
          <li>
            <span class="grow">
              <b>{b.name}</b>
              <span class="muted small">
                {[b.mods ? `модов с обновлением: ${b.mods}` : "", b.pack ? `новая версия сборки ${b.pack.version_number}` : ""]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
            </span>
            <button class="btn btn-sm" onclick={() => openBuild(b.build, b.mods ? "mods" : "settings")}>Открыть</button>
          </li>
        {/each}
      </ul>
    {:else if updates.checked && !updates.running}
      <p class="muted small">Всё свежее.</p>
    {/if}
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
  .opt {
    display: flex;
    align-items: flex-start;
    gap: var(--s3);
    cursor: pointer;
  }
  .opt input {
    margin-top: 4px;
  }
  .opt > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .off {
    opacity: 0.55;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--s3);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .found {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .found li {
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: var(--s2) 0;
    border-top: 1px solid var(--border);
  }
  .found li > .grow {
    display: flex;
    flex-direction: column;
  }
</style>
