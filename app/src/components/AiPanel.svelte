<script lang="ts">
  // «Разобрать с ИИ» (раздел 5.8 ТЗ): по кнопке — сначала показать, что
  // именно уйдёт в сервис (если так в настройках), потом ответ ИИ. Выключенный
  // ИИ-разбор объясняет, почему он выключен, и ведёт в настройки.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { untrack } from "svelte";
  import { api, type AiAnswer, type AiPrepared } from "../lib/api";
  import { ai } from "../lib/ai.svelte";
  import { ago, size } from "../lib/format";
  import { renderMarkdown } from "../lib/markdown";
  import { openSettings } from "../lib/nav.svelte";
  import { inTauri } from "../lib/tauri";
  import Icon from "./Icon.svelte";

  let {
    build,
    file = null,
    answer = null,
    pending = false,
    onleave,
  }: {
    build: string;
    /// Лог или отчёт из вкладки «Логи»; `null` — последняя игра.
    file?: string | null;
    /// Уже полученный ответ (по последней игре).
    answer?: AiAnswer | null;
    /// Лаунчер сам спросил ИИ после игры — ждём ответ.
    pending?: boolean;
    /// Перед переходом в настройки (закрыть окно).
    onleave?: () => void;
  } = $props();

  let mine = $state<AiAnswer | null>(null);
  const shown = $derived(mine ?? answer);
  let autoDone = $state(false);
  const waitingAuto = $derived(pending && !autoDone && !mine);
  let preview = $state<AiPrepared | null>(null);
  let busy = $state<"preview" | "ask" | null>(null);
  let error = $state("");
  let hint = $state(false);
  const enabled = $derived(ai.settings?.enabled ?? false);

  // Ждём ответ разбора, который лаунчер запустил сам: событием, а если окно
  // открылось позже события — из сохранённого разбора последней игры.
  const since = Date.now() / 1000 - 120;
  $effect(() => {
    if (!pending) return;
    untrack(() => (ai.waiting[build] = (ai.waiting[build] ?? 0) + 1));
    const timer = setInterval(() => {
      api
        .problemsLast(build)
        .then((s) => {
          if (s?.ai?.auto && s.ai.at >= since && !autoDone) {
            autoDone = true;
            mine = s.ai;
          }
        })
        .catch(() => {});
    }, 5000);
    const stop = setTimeout(() => clearInterval(timer), 6 * 60 * 1000);
    return () => {
      clearInterval(timer);
      clearTimeout(stop);
      untrack(() => (ai.waiting[build] = Math.max(0, (ai.waiting[build] ?? 1) - 1)));
    };
  });
  $effect(() => {
    const ev = ai.last;
    if (!pending || !ev || ev.build !== build || untrack(() => autoDone)) return;
    autoDone = true;
    if (ev.answer) mine = ev.answer;
    else error = ev.error ?? "";
  });

  async function start() {
    error = "";
    if (!enabled) {
      hint = !hint;
      return;
    }
    if (ai.settings?.preview) {
      busy = "preview";
      try {
        preview = await api.aiPreview(build, file);
      } catch (e) {
        error = String(e);
      } finally {
        busy = null;
      }
    } else {
      await send(null);
    }
  }

  async function send(text: string | null) {
    busy = "ask";
    error = "";
    try {
      mine = await api.aiAsk(build, file, text);
      preview = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  function settings() {
    onleave?.();
    openSettings("ai");
  }

  // Ссылки из ответа — во внешнем браузере.
  function intercept(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    const href = a?.getAttribute("href");
    if (!href) return;
    e.preventDefault();
    if (inTauri) openUrl(href).catch(() => {});
    else window.open(href);
  }
</script>

<section class="ai">
  {#if shown}
    <div class="answer">
      <div class="head">
        <Icon name="star" size={16} />
        <b>Разбор ИИ</b>
        <span class="muted small grow"
          >{shown.by} · {ago(shown.at)}{shown.auto ? " · лаунчер спросил сам после игры" : ""}</span
        >
      </div>
      <!-- Ответ прошёл DOMPurify (lib/markdown.ts). -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
      <article class="md" onclick={intercept}>{@html renderMarkdown(shown.text)}</article>
      {#if shown.truncated}<p class="muted small">Ответ оборвался на лимите длины.</p>{/if}
      <p class="muted small">ИИ может ошибаться: проверьте совет, прежде чем удалять что-то из сборки.</p>
    </div>
  {/if}

  {#if waitingAuto || busy === "ask"}
    <p class="wait small"><span class="dot"></span>ИИ разбирает лог… Обычно это до минуты.</p>
  {:else if preview}
    {@const p = preview}
    <div class="preview">
      <p class="small">
        Уйдёт в <b>{p.to}</b>: {size(p.bytes)}, примерно {p.tokens.toLocaleString("ru-RU")} токенов. Личные данные уже
        заменены метками <code>&lt;user&gt;</code>, <code>&lt;player&gt;</code>, <code>&lt;ip&gt;</code> и т. п.
      </p>
      <pre>{p.text}</pre>
      <div class="actions">
        <button class="btn btn-sm btn-primary" onclick={() => send(p.text)}>Отправить</button>
        <button class="btn btn-sm btn-quiet" onclick={() => (preview = null)}>Отмена</button>
      </div>
    </div>
  {:else}
    <button class="btn btn-sm" disabled={busy !== null} onclick={start}>
      <Icon name="star" size={16} />{busy === "preview" ? "Собираю…" : shown ? "Спросить ИИ ещё раз" : "Разобрать с ИИ"}
    </button>
  {/if}

  {#if hint && !enabled}
    <p class="hint small">
      ИИ-разбор выключен. Он отправляет лог в выбранный вами сервис — Claude, ChatGPT, Gemini, OpenRouter или
      локальную модель (Ollama, LM Studio) — вашим ключом, поэтому включается только вручную.
      <button class="link" onclick={settings}>Открыть настройки</button>
    </p>
  {/if}
  {#if error}<p class="warn"><Icon name="error" size={16} />{error}</p>{/if}
</section>

<style>
  .ai {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--s2);
    margin: var(--s3) 0;
  }
  .answer,
  .preview {
    align-self: stretch;
    padding: var(--s3) var(--s4);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--surface);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
  .grow {
    flex: 1;
  }
  .wait {
    display: flex;
    align-items: center;
    gap: var(--s2);
    margin: 0;
    color: var(--text-muted);
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.25;
    }
  }
  .hint {
    margin: 0;
    color: var(--text-muted);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-decoration: underline;
    cursor: pointer;
  }
  .actions {
    display: flex;
    gap: var(--s2);
  }
  .preview p {
    margin: 0 0 var(--s2);
  }
  pre {
    max-height: 260px;
    margin: 0 0 var(--s3);
    padding: var(--s3);
    overflow: auto;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--r);
    font: var(--fs-12) / 1.5 ui-monospace, Consolas, monospace;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }
  .md {
    line-height: 1.55;
    user-select: text;
    overflow-wrap: anywhere;
  }
  .md :global(p),
  .md :global(ol),
  .md :global(ul) {
    margin: var(--s2) 0;
  }
  .md :global(a) {
    color: var(--text);
    text-decoration: underline;
  }
  .md :global(pre),
  .md :global(code) {
    font-family: Consolas, monospace;
    font-size: var(--fs-12);
    background: var(--surface-raised);
    border-radius: var(--r-sm);
  }
  .md :global(pre) {
    padding: var(--s2) var(--s3);
    overflow: auto;
  }
  .md :global(h1),
  .md :global(h2),
  .md :global(h3) {
    margin: var(--s3) 0 var(--s1);
    font-size: var(--fs-16);
  }
</style>
