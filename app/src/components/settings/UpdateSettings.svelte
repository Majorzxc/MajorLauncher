<script lang="ts">
  // «Настройки → Обновления» (раздел 5.13 ТЗ): сам лаунчер и что он обновляет в фоне.
  import { api, type UpdateSettings } from "../../lib/api";
  import { ago, size } from "../../lib/format";
  import { interceptLinks, openExternal } from "../../lib/links";
  import { renderMarkdown } from "../../lib/markdown";
  import Logo from "../Logo.svelte";
  import { openBuild } from "../../lib/nav.svelte";
  import { attempt } from "../../lib/toast.svelte";
  import { checkLauncher, checkNow, launcher, updates } from "../../lib/updates.svelte";
  import Icon from "../Icon.svelte";

  // «в 14:05», вчера и раньше — «вчера», «3 дня назад».
  function checkedAt(unix: number): string {
    const d = new Date(unix * 1000);
    const today = new Date().toDateString() === d.toDateString();
    return today ? `в ${d.toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" })}` : ago(unix);
  }

  let opts = $state<UpdateSettings | null>(null);
  let rules = $state<[number, boolean] | null>(null);
  api.rulesInfo().then((r) => (rules = r)).catch(() => {});
  api.config().then(
    (c) =>
      (opts = c.updates ?? {
        check: true,
        auto_mods: false,
        auto_packs: false,
        launcher: true,
        launcher_download: true,
        channel: "stable",
      }),
  );

  async function set<K extends keyof UpdateSettings>(key: K, value: UpdateSettings[K]) {
    if (!opts) return;
    const next = { ...opts, [key]: value };
    const saved = await attempt("Обновления", () => api.setUpdates(next));
    if (saved) opts = saved.updates ?? next;
    // Сменили канал — сразу посмотреть, что в нём есть.
    if (saved && key === "channel") checkLauncher();
  }

  let installing = $state(false);
  async function install() {
    installing = true;
    // Получилось — лаунчер закрывается и открывается уже новым.
    await attempt("Обновление лаунчера", () => api.launcherUpdateInstall());
    installing = false;
  }

  const published = (iso: string | null) => (iso ? new Date(iso).toLocaleDateString("ru-RU", { day: "numeric", month: "long" }) : "");
</script>

{#if opts}
  {@const lu = launcher.value}
  <div class="card block">
    <div class="row">
      <Logo size={40} />
      <span class="status">
        <b>MajorLauncher {lu?.current ?? ""}</b>
        <span class="muted small">
          {lu?.busy
            ? "Проверяю GitHub…"
            : lu?.available
              ? `Вышла версия ${lu.available.version}`
              : lu?.checked
                ? `Это последняя версия · проверено ${checkedAt(lu.checked)}`
                : "Новые версии ещё не проверялись с запуска лаунчера"}
        </span>
      </span>
      <button class="btn btn-sm" disabled={!lu || lu.busy} onclick={() => attempt("Проверка версии", checkLauncher)}>
        <Icon name="refresh" size={16} />Проверить
      </button>
    </div>
    {#if lu?.available}
      {@const a = lu.available}
      <div class="release">
        <p>
          <b>Версия {a.version}</b>{#if a.beta}<span class="beta small">бета</span>{/if}
          <span class="muted small">{[published(a.published), size(a.size)].filter(Boolean).join(" · ")}</span>
        </p>
        {#if a.notes.trim()}
          <!-- Текст выпуска прошёл DOMPurify (lib/markdown.ts), ссылки — в браузер (клавиатура — через Enter на ссылке). -->
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
          <article class="md notes" onclick={interceptLinks}>{@html renderMarkdown(a.notes)}</article>
        {/if}
        <div class="row">
          <button class="btn btn-primary" disabled={installing || lu.busy} onclick={install}>
            <Icon name="download" size={18} />{installing
              ? "Запускаю установку…"
              : lu.ready
                ? "Установить и перезапустить"
                : "Скачать и установить"}
          </button>
          <button class="btn btn-quiet" onclick={() => openExternal(a.page).catch(() => {})}>
            <Icon name="link" size={18} />Страница выпуска
          </button>
        </div>
        <p class="muted small">
          Подпись выпуска, версия и хеш установщика проверены. Установка занимает несколько секунд, потом лаунчер
          откроется сам. Сборки, миры и настройки не трогаются. Во время игры обновить нельзя — установщик закрывает
          лаунчер.
        </p>
      </div>
    {/if}
    {#if lu?.error}<p class="warn"><Icon name="error" size={18} />{lu.error}</p>{/if}
    <label class="opt">
      <input type="checkbox" checked={opts.launcher} onchange={(e) => set("launcher", e.currentTarget.checked)} />
      <span>
        <b>Проверять новые версии лаунчера</b>
        <span class="muted small"
          >Вместе с фоновой проверкой: лаунчер спрашивает у GitHub список выпусков. О вас ничего не передаётся.</span
        >
      </span>
    </label>
    <label class="opt" class:off={!opts.launcher}>
      <input
        type="checkbox"
        checked={opts.launcher_download}
        disabled={!opts.launcher}
        onchange={(e) => set("launcher_download", e.currentTarget.checked)}
      />
      <span>
        <b>Скачивать новую версию самому</b>
        <span class="muted small">Около 5 МБ. Ставится она только по кнопке — не посреди игры.</span>
      </span>
    </label>
    <div class="row">
      <span class="status">
        <b>Канал</b>
        <span class="muted small">Бета — новые версии раньше остальных, но в них чаще бывают ошибки.</span>
      </span>
      <div class="chips" role="radiogroup" aria-label="Канал обновлений">
        {#each [["stable", "Стабильный"], ["beta", "Бета"]] as const as [id, title] (id)}
          <button
            class="chip"
            role="radio"
            aria-checked={opts.channel === id}
            class:on={opts.channel === id}
            onclick={() => opts!.channel !== id && set("channel", id)}>{title}</button
          >
        {/each}
      </div>
    </div>
  </div>

  <div class="card block">
    <label class="opt">
      <input type="checkbox" checked={opts.check} onchange={(e) => set("check", e.currentTarget.checked)} />
      <span>
        <b>Проверять обновления в фоне</b>
        <span class="muted small"
          >Через минуту после запуска лаунчера и дальше раз в 6 часов: моды сборок и сами сборки из каталога. Только
          проверка у Modrinth — ничего не меняется, о новом лаунчер скажет. Заодно скачиваются правила разбора ошибок
          с GitHub — они подписаны, чужой файл лаунчер не примет.</span
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
      <!-- Две строки друг под другом: в одну они не помещались и обрезались. -->
      <span class="status muted small">
        <span>
          {updates.running
            ? "Проверяю…"
            : updates.checked
              ? `Последняя проверка — ${checkedAt(updates.checked)}`
              : "Ещё не проверялось с запуска лаунчера"}
        </span>
        {#if rules}<span>Правила разбора ошибок: версия {rules[0]}{rules[1] ? " с GitHub" : ", встроенные"}</span>{/if}
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
  .grow {
    flex: 1;
    min-width: 0;
  }
  .status {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
  }
  .release {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
    padding: var(--s4);
    border: 1px solid var(--accent);
    border-radius: var(--r-card);
  }
  .release p {
    display: flex;
    align-items: baseline;
    gap: var(--s2);
  }
  .beta {
    padding: 0 6px;
    border: 1px solid var(--warning);
    border-radius: var(--r-sm);
    color: var(--warning);
  }
  .notes {
    max-height: 220px;
    overflow: auto;
    font-size: var(--fs-14);
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
