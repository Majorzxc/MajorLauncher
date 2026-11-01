<script lang="ts">
  // «Настройки → ИИ-разбор» (раздел 5.8 ТЗ): сервис, ключ, модель, показ
  // перед отправкой, разбор сам после игры и лимит в день. Выключено, пока
  // игрок не включит: лог уходит в чужой сервис.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type AiProvider, type AiSettings } from "../lib/api";
  import { ai } from "../lib/ai.svelte";
  import { inTauri } from "../lib/tauri";
  import { attempt, toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";

  const PROVIDERS: { value: AiProvider; label: string; url: string; key?: string; about: string }[] = [
    {
      value: "anthropic",
      label: "Claude (Anthropic)",
      url: "https://api.anthropic.com/v1",
      key: "https://console.anthropic.com/settings/keys",
      about: "Ключ — в консоли Anthropic. По умолчанию модель claude-opus-5-5.",
    },
    {
      value: "openai",
      label: "ChatGPT (OpenAI)",
      url: "https://api.openai.com/v1",
      key: "https://platform.openai.com/api-keys",
      about: "Ключ — на platform.openai.com. Подписка ChatGPT Plus сюда не подходит: нужен ключ API.",
    },
    {
      value: "gemini",
      label: "Gemini (Google)",
      url: "https://generativelanguage.googleapis.com/v1beta/openai",
      key: "https://aistudio.google.com/apikey",
      about: "Ключ — в Google AI Studio; у Google есть бесплатный лимит.",
    },
    {
      value: "openrouter",
      label: "OpenRouter",
      url: "https://openrouter.ai/api/v1",
      key: "https://openrouter.ai/keys",
      about: "Один ключ — сотни моделей разных компаний, есть бесплатные.",
    },
    {
      value: "ollama",
      label: "Ollama (на этом ПК)",
      url: "http://localhost:11434/v1",
      about: "Модель работает на вашем компьютере, лог никуда не уходит. Ollama должна быть запущена, модель — скачана (ollama pull).",
    },
    {
      value: "lmstudio",
      label: "LM Studio (на этом ПК)",
      url: "http://localhost:1234/v1",
      about: "Модель работает на вашем компьютере. В LM Studio включите сервер (вкладка Developer) и загрузите модель.",
    },
    {
      value: "custom",
      label: "Другой (OpenAI-совместимый)",
      url: "",
      about: "DeepSeek, Groq, Mistral и любой сервис с OpenAI-совместимым API: адрес вида https://…/v1, ключ и модель.",
    },
  ];

  let opts = $state<AiSettings | null>(null);
  let keys = $state<AiProvider[]>([]);
  let keyInput = $state("");
  let models = $state<string[]>([]);
  let loadingModels = $state(false);
  let used = $state(0);

  const DEFAULTS: AiSettings = {
    enabled: false,
    provider: "anthropic",
    model: "",
    url: "",
    preview: true,
    auto_unknown: false,
    auto_critical: false,
    auto_warnings: false,
    daily_limit: 5,
  };
  api.config().then((c) => (opts = c.ai ?? DEFAULTS));
  api.aiKeys().then((k) => (keys = k)).catch(() => {});
  api.aiUsedToday().then((n) => (used = n)).catch(() => {});

  const info = $derived(PROVIDERS.find((p) => p.value === opts?.provider) ?? PROVIDERS[0]);
  const local = $derived(opts?.provider === "ollama" || opts?.provider === "lmstudio");
  const hasKey = $derived(opts ? keys.includes(opts.provider) : false);

  async function save(patch: Partial<AiSettings>) {
    if (!opts) return;
    const next = { ...opts, ...patch };
    opts = next;
    const saved = await attempt("ИИ-разбор", () => api.setAi(next));
    if (saved?.ai) {
      opts = saved.ai;
      ai.settings = saved.ai;
    }
  }

  function provider(p: AiProvider) {
    models = [];
    keyInput = "";
    save({ provider: p, model: "", url: "" });
  }

  async function saveKey() {
    if (!opts) return;
    const p = opts.provider;
    const done = await attempt("Ключ", () => api.aiSetKey(p, keyInput).then(() => true));
    if (!done) return;
    keyInput = "";
    keys = await api.aiKeys();
    toast("Ключ сохранён в «Диспетчере учётных данных» Windows");
  }

  async function removeKey() {
    if (!opts) return;
    const p = opts.provider;
    await attempt("Ключ", () => api.aiSetKey(p, ""));
    keys = await api.aiKeys();
  }

  async function loadModels() {
    if (!opts) return;
    loadingModels = true;
    const found = await attempt("Модели", () => api.aiModels(opts!));
    loadingModels = false;
    if (!found) return;
    models = found.list;
    // API нашёлся по другому адресу (с /v1 на конце) — запомнить его.
    const fixed = opts.provider === "custom" && found.url.replace(/\/$/, "") !== opts.url.replace(/\/$/, "");
    if (fixed) await save({ url: found.url });
    toast(
      (fixed ? `Адрес API исправлен на ${found.url}. ` : "") +
        (found.list.length ? `Сервис отвечает, моделей: ${found.list.length}` : "Сервис отвечает, но моделей не назвал"),
    );
  }

  function link(url: string) {
    if (inTauri) openUrl(url).catch(() => {});
    else window.open(url);
  }
</script>

{#if opts}
  <div class="card block">
    <p class="muted">
      Когда правила лаунчера не узнали ошибку (или вы хотите второе мнение), лог можно отправить ИИ. Своих серверов у
      лаунчера нет: запрос уходит прямо в выбранный сервис вашим ключом, платите вы по его тарифу — один разбор обычно
      10–20 тысяч токенов. Перед отправкой из лога убираются имя пользователя Windows, имя ПК, ник и UUID игрока, IP и
      адреса серверов, токены, пароли и почта.
    </p>
    <label class="opt">
      <input type="checkbox" checked={opts.enabled} onchange={(e) => save({ enabled: e.currentTarget.checked })} />
      <span><b>Включить ИИ-разбор</b></span>
    </label>
  </div>

  <div class="card block" class:off={!opts.enabled}>
    <h2>Сервис</h2>
    <div class="row">
      <Select label="Сервис" value={opts.provider} options={PROVIDERS} onchange={provider} />
      {#if info.key}
        <button class="btn btn-sm btn-quiet" onclick={() => link(info.key!)}><Icon name="link" size={16} />Где взять ключ</button>
      {/if}
    </div>
    <p class="muted small">{info.about}</p>

    {#if opts.provider === "custom" || local}
      <label class="field">
        <span>Адрес API</span>
        <input
          class="input"
          placeholder={info.url || "https://api.example.com/v1"}
          value={opts.url}
          onchange={(e) => save({ url: e.currentTarget.value.trim() })}
        />
      </label>
    {/if}

    {#if !local}
      <div class="field">
        <span>Ключ API</span>
        {#if hasKey}
          <div class="row">
            <span class="muted small grow"><Icon name="lock" size={16} /> Ключ сохранён в «Диспетчере учётных данных» Windows</span>
            <button class="btn btn-sm btn-quiet" onclick={removeKey}>Удалить</button>
          </div>
        {/if}
        <div class="row">
          <input
            class="input grow"
            type="password"
            autocomplete="off"
            placeholder={hasKey ? "Заменить ключ…" : "Вставьте ключ"}
            bind:value={keyInput}
          />
          <button class="btn btn-sm" disabled={!keyInput.trim()} onclick={saveKey}>Сохранить</button>
        </div>
      </div>
    {/if}

    <label class="field">
      <span>Модель</span>
      <div class="row">
        <input
          class="input grow"
          list="ai-models"
          placeholder={opts.provider === "anthropic" ? "claude-opus-5-5" : "Выберите из списка или впишите"}
          value={opts.model}
          onchange={(e) => save({ model: e.currentTarget.value.trim() })}
        />
        <button class="btn btn-sm" disabled={loadingModels} onclick={loadModels}>
          <Icon name="refresh" size={16} />{loadingModels ? "Загружаю…" : "Загрузить список"}
        </button>
      </div>
      <datalist id="ai-models">
        {#each models as m (m)}<option value={m}></option>{/each}
      </datalist>
      <span class="muted small">«Загрузить список» заодно проверяет адрес и ключ — это бесплатно.</span>
    </label>
  </div>

  <div class="card block" class:off={!opts.enabled}>
    <h2>Когда спрашивать</h2>
    <label class="opt">
      <input type="checkbox" checked={opts.preview} onchange={(e) => save({ preview: e.currentTarget.checked })} />
      <span>
        <b>Показывать перед отправкой, что уходит</b>
        <span class="muted small">По кнопке «Разобрать с ИИ» — сначала текст запроса и его размер, отправка — после вашего «Отправить».</span>
      </span>
    </label>
    <p class="muted small">
      Сам после игры, без кнопки (тогда текст перед отправкой не показывается, но очищается так же):
    </p>
    <label class="opt">
      <input type="checkbox" checked={opts.auto_unknown} onchange={(e) => save({ auto_unknown: e.currentTarget.checked })} />
      <span><b>Игра упала, а правила лаунчера причину не узнали</b></span>
    </label>
    <label class="opt">
      <input type="checkbox" checked={opts.auto_critical} onchange={(e) => save({ auto_critical: e.currentTarget.checked })} />
      <span><b>Любая критическая ошибка</b></span>
    </label>
    <label class="opt">
      <input type="checkbox" checked={opts.auto_warnings} onchange={(e) => save({ auto_warnings: e.currentTarget.checked })} />
      <span><b>Предупреждения за игру</b><span class="muted small">Бывают почти в каждой сборке с модами — расход будет заметный.</span></span>
    </label>
    <label class="field limit">
      <span>Не больше раз в день</span>
      <input
        class="input"
        type="number"
        min="1"
        max="100"
        value={opts.daily_limit}
        onchange={(e) => save({ daily_limit: Math.max(1, Math.min(100, Number(e.currentTarget.value) || 5)) })}
      />
      <span class="muted small">Сегодня лаунчер спросил сам: {used}. Кнопка «Разобрать с ИИ» в лимит не входит.</span>
    </label>
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
  .block p,
  h2 {
    margin: 0;
  }
  h2 {
    font-size: var(--fs-16);
  }
  .off {
    opacity: 0.55;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
  .grow {
    flex: 1;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
  }
  .field > span:first-child {
    font-weight: 600;
  }
  .limit input {
    width: 100px;
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
</style>
