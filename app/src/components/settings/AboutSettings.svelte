<script lang="ts">
  // «Настройки → О программе» (раздел 5.16 ТЗ): версия, лицензия, ссылки,
  // «Собрать логи» для сообщения об ошибке, обучение заново.
  import { save as saveFile } from "@tauri-apps/plugin-dialog";
  import { api } from "../../lib/api";
  import { size } from "../../lib/format";
  import { openExternal } from "../../lib/links";
  import { call, inTauri } from "../../lib/tauri";
  import { restartTutorial, stopTutorial, tutorial } from "../../lib/tutorial.svelte";
  import Icon from "../Icon.svelte";
  import Logo from "../Logo.svelte";

  const REPO = "https://github.com/Majorzxc/MajorLauncher";
  const version = call<string>("core_version");

  let error = $state("");
  async function open(url: string) {
    error = "";
    try {
      await openExternal(url);
    } catch (e) {
      error = `Не удалось открыть ссылку: ${e}`;
    }
  }

  // «Собрать логи»: архив для сообщения об ошибке — куда скажет игрок.
  let collecting = $state(false);
  let collected = $state("");
  async function collectLogs() {
    if (!inTauri) return;
    const stamp = new Date().toISOString().slice(0, 16).replace(/[-:T]/g, "");
    const path = await saveFile({
      title: "Архив логов для сообщения об ошибке",
      defaultPath: `MajorLauncher-logs-${stamp}.zip`,
      filters: [{ name: "Архив", extensions: ["zip"] }],
    });
    if (!path) return;
    collecting = true;
    collected = "";
    error = "";
    try {
      const r = await api.launcherReport(path);
      collected = `Готово: ${r.files.join(", ")} — ${size(r.size)}. Архив открыт в проводнике.`;
    } catch (e) {
      error = String(e);
    } finally {
      collecting = false;
    }
  }
</script>

<div class="card block about">
  <Logo size={64} />
  <div>
    <h2>MajorLauncher</h2>
    {#await version then v}<p class="muted">Версия {v}</p>{/await}
    <p class="muted">Открытый код, лицензия GPL-3.0. Без рекламы и телеметрии.</p>
  </div>
</div>
<div class="card block">
  <div class="row-buttons">
    <button class="btn" onclick={() => open(REPO)}><Icon name="link" size={18} /> Исходный код на GitHub</button>
    <button class="btn" onclick={() => open(`${REPO}/releases`)}><Icon name="download" size={18} /> Все выпуски</button>
  </div>
  <p class="muted small">Иконки — Lucide (ISC). Шрифт — Onest (SIL Open Font License).</p>
</div>
<div class="card block">
  <h2>Сообщить об ошибке</h2>
  <p class="muted">
    Лаунчер соберёт в один архив свой журнал, сведения о ПК (Windows, процессор, память, видеокарты), список сборок
    с версиями, настройки и последний лог игры. Ник, UUID, имя ПК и пользователя, токены и адреса серверов из текста
    убираются, ключи входа и ИИ в архив не попадают. Архив никуда не отправляется — приложите его к сообщению сами.
  </p>
  <div class="row-buttons">
    <button class="btn" disabled={collecting} onclick={collectLogs}>
      <Icon name="log" size={18} />{collecting ? "Собираю…" : "Собрать логи…"}
    </button>
    <button class="btn btn-quiet" onclick={() => open(`${REPO}/issues`)}><Icon name="link" size={18} /> Ошибки на GitHub</button>
  </div>
  {#if collected}<p class="muted small">{collected}</p>{/if}
</div>
<div class="card block">
  <h2>Обучение</h2>
  <p class="muted">
    Короткая экскурсия по главному экрану, а в каждом разделе — пара подсказок при первом открытии.
    {tutorial.state.enabled ? "Сейчас подсказки разделов включены." : "Сейчас подсказки разделов выключены."}
  </p>
  <div class="row-buttons">
    <button class="btn" onclick={restartTutorial}><Icon name="info" size={18} />Пройти обучение заново</button>
    {#if tutorial.state.enabled}
      <button class="btn btn-quiet" onclick={stopTutorial}>Выключить подсказки</button>
    {/if}
  </div>
</div>
{#if error}<p class="warn"><Icon name="error" size={18} />{error}</p>{/if}

<style>
  .card.block.about {
    flex-direction: row;
    align-items: center;
  }
  .about p {
    margin: 0;
  }
</style>
