<script lang="ts">
  // «Настройки → Моды и каталог» (раздел 5.16 ТЗ): перевод описаний, зависимости,
  // какие предупреждения показывать.
  import { catalog, setAutoDeps, setTips, setTranslate } from "../../lib/catalog.svelte";
</script>

<div class="card block">
  <label class="opt">
    <input type="checkbox" checked={catalog.translate} onchange={(e) => setTranslate(e.currentTarget.checked)} />
    <span>
      <b>Переводить описания модов на русский</b>
      <span class="muted small">
        Описания на странице мода и краткие описания в выдаче будут показываться по-русски, рядом — «Показать оригинал».
      </span>
    </span>
  </label>
  <p class="muted small">
    Перевод делает Google: текст описания мода отправляется на translate.googleapis.com. Ваши данные — ник, аккаунты,
    сборки, файлы — не отправляются, только текст описания. Это единственное место, где лаунчер что-то передаёт чужому
    сервису, поэтому по умолчанию перевод выключен. Готовые переводы хранятся на компьютере и повторно не запрашиваются.
  </p>
</div>
<div class="card block">
  <label class="opt">
    <input type="checkbox" checked={catalog.autoDeps} onchange={(e) => setAutoDeps(e.currentTarget.checked)} />
    <span>
      <b>Ставить нужные зависимости без окна подтверждения</b>
      <span class="muted small">
        Окно всё равно появится, если чего-то не хватает или есть несовместимость с уже стоящими модами.
      </span>
    </span>
  </label>
  <label class="opt">
    <input type="checkbox" checked={catalog.tips} onchange={(e) => setTips(e.currentTarget.checked)} />
    <span>
      <b>Показывать советы 💡 среди предупреждений о модах</b>
      <span class="muted small">
        Например, «есть обновление мода», «старый драйвер видеокарты», «включена экономия энергии». Ошибки ⛔ и
        предупреждения ⚠ видны всегда.
      </span>
    </span>
  </label>
</div>
