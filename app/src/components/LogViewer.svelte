<script lang="ts">
  // Лог или краш-отчёт прямо в лаунчере: подсветка уровней, поиск, фильтр
  // «только ошибки». Строк бывает десятки тысяч — рисуются только видимые.
  import { api, type LogFile } from "../lib/api";
  import { size } from "../lib/format";
  import { attempt, toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";

  let { id, log, onback }: { id: string; log: LogFile; onback: () => void } = $props();

  type Level = "error" | "warn" | "info" | "debug" | "";
  const ROW = 20;

  let lines = $state<string[]>([]);
  let levels = $state<Level[]>([]);
  let truncated = $state(false);
  let error = $state("");
  let loaded = $state(false);

  let filter = $state<"all" | "problems" | "errors">("all");
  let query = $state("");
  let current = $state(0);

  let box = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let height = $state(500);
  // Лог занимает всё место до низа окна.
  let fit = $state(400);
  function measure() {
    if (box) fit = Math.max(240, window.innerHeight - box.getBoundingClientRect().top - 24);
  }
  $effect(() => {
    if (!box) return;
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  });

  // [12:00:00] [Render thread/WARN]: …, [12:00:00.123] [main/ERROR] [mod/]: …,
  // старые версии: 2013-04-01 12:00:00 [SEVERE] …
  const HEAD = /^\[[^\]]*\] \[[^\]]*\/(FATAL|ERROR|WARN|INFO|DEBUG|TRACE)\]|^\d{4}-\d\d-\d\d \d\d:\d\d:\d\d \[(SEVERE|WARNING|INFO|FINE)\]/;

  function classify(all: string[], crash: boolean): Level[] {
    const out: Level[] = [];
    let last: Level = "";
    for (const line of all) {
      if (crash) {
        out.push(/Exception|Error\b|^Caused by|^\s+at |^Description:/.test(line) ? "error" : "");
        continue;
      }
      const m = HEAD.exec(line);
      if (m) {
        const l = m[1] ?? m[2];
        last =
          l === "FATAL" || l === "ERROR" || l === "SEVERE"
            ? "error"
            : l === "WARN" || l === "WARNING"
              ? "warn"
              : l === "INFO"
                ? "info"
                : "debug";
      } else if (!/^\s|^Caused by|^\w+(\.\w+)+(Exception|Error)/.test(line)) {
        // Строка без заголовка и не продолжение трассировки — обычный вывод.
        last = /Exception|Error:/.test(line) ? "error" : "";
      }
      out.push(last);
    }
    return out;
  }

  $effect(() => {
    const file = log.file;
    loaded = false;
    api
      .logText(id, file)
      .then((r) => {
        const all = r.text.replace(/\r/g, "").split("\n");
        if (all.length && all[all.length - 1] === "") all.pop();
        lines = all;
        levels = classify(all, log.kind === "crash");
        truncated = r.truncated;
        loaded = true;
        // Лог читают с конца (там краш и выход), краш-отчёт — с начала.
        requestAnimationFrame(() => {
          if (box && log.kind !== "crash") box.scrollTop = box.scrollHeight;
        });
      })
      .catch((e) => (error = String(e)));
  });

  // Строки после фильтра (номера в исходном списке).
  const shown = $derived.by(() => {
    const out: number[] = [];
    for (let i = 0; i < lines.length; i++) {
      const l = levels[i];
      if (filter === "errors" && l !== "error") continue;
      if (filter === "problems" && l !== "error" && l !== "warn") continue;
      out.push(i);
    }
    return out;
  });

  const needle = $derived(query.trim().toLowerCase());
  // Позиции совпадений в `shown`.
  const matches = $derived.by(() => {
    if (!needle) return [] as number[];
    const out: number[] = [];
    shown.forEach((line, pos) => {
      if (lines[line].toLowerCase().includes(needle)) out.push(pos);
    });
    return out;
  });

  const counts = $derived({
    errors: levels.filter((l) => l === "error").length,
    warns: levels.filter((l) => l === "warn").length,
  });

  $effect(() => {
    void needle;
    current = 0;
    if (matches.length) reveal(matches[0]);
  });

  function reveal(pos: number) {
    if (!box) return;
    const top = pos * ROW;
    if (top < box.scrollTop || top > box.scrollTop + box.clientHeight - ROW * 2) {
      box.scrollTop = Math.max(0, top - box.clientHeight / 2);
    }
  }

  function jump(d: number) {
    if (!matches.length) return;
    current = (current + d + matches.length) % matches.length;
    reveal(matches[current]);
  }

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 20));
  const last = $derived(Math.min(shown.length, Math.ceil((scrollTop + height) / ROW) + 20));
  const activeLine = $derived(matches.length ? shown[matches[current]] : -1);

  // Подсветить совпадения в строке.
  function parts(text: string): { t: string; hit: boolean }[] {
    if (!needle) return [{ t: text, hit: false }];
    const out: { t: string; hit: boolean }[] = [];
    const low = text.toLowerCase();
    let at = 0;
    for (;;) {
      const i = low.indexOf(needle, at);
      if (i < 0) break;
      if (i > at) out.push({ t: text.slice(at, i), hit: false });
      out.push({ t: text.slice(i, i + needle.length), hit: true });
      at = i + needle.length;
    }
    if (at < text.length) out.push({ t: text.slice(at), hit: false });
    return out;
  }

  async function copy() {
    const text = shown.map((i) => lines[i]).join("\n");
    try {
      await navigator.clipboard.writeText(text);
      toast(filter === "all" ? "Лог скопирован" : `Скопировано строк: ${shown.length}`);
    } catch (e) {
      toast(`Не удалось скопировать: ${e}`, { error: true });
    }
  }

  const name = $derived(log.file.split("/").pop());
</script>

<div class="tools">
  <button class="btn btn-sm btn-quiet" onclick={onback}><Icon name="arrow-left" size={16} />Логи</button>
  <span class="file">
    <b>{name}</b>
    <span class="muted small">{size(log.size)}{lines.length ? ` · ${lines.length} строк` : ""}{truncated ? " · показан конец" : ""}</span>
  </span>
  <span class="grow"></span>
  <button class="btn btn-sm btn-quiet" title="Скопировать (с учётом фильтра)" aria-label="Скопировать" onclick={copy}><Icon name="copy" size={16} /></button>
  <button
    class="btn btn-sm btn-quiet"
    title="Открыть в программе"
    aria-label="Открыть в программе"
    onclick={() => attempt("Лог", () => api.open(id, { kind: "log", file: log.file }))}><Icon name="export" size={16} /></button
  >
  <button
    class="btn btn-sm btn-quiet"
    title="Показать в папке"
    aria-label="Показать в папке"
    onclick={() => attempt("Лог", () => api.reveal(id, { kind: "log", file: log.file }))}><Icon name="folder" size={16} /></button
  >
</div>

<div class="tools">
  <label class="search">
    <Icon name="search" size={16} />
    <input
      type="search"
      bind:value={query}
      placeholder="Найти в логе"
      spellcheck="false"
      onkeydown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          jump(e.shiftKey ? -1 : 1);
        }
      }}
    />
  </label>
  {#if needle}
    <span class="muted small count">{matches.length ? `${current + 1} из ${matches.length}` : "не найдено"}</span>
    <button class="btn btn-sm btn-quiet" title="Предыдущее (Shift+Enter)" aria-label="Предыдущее" disabled={!matches.length} onclick={() => jump(-1)}>↑</button>
    <button class="btn btn-sm btn-quiet" title="Следующее (Enter)" aria-label="Следующее" disabled={!matches.length} onclick={() => jump(1)}>↓</button>
  {/if}
  <span class="grow"></span>
  <div class="chips" role="radiogroup" aria-label="Какие строки показывать">
    <button class="chip" role="radio" aria-checked={filter === "all"} class:on={filter === "all"} onclick={() => (filter = "all")}>Все</button>
    <button class="chip" role="radio" aria-checked={filter === "problems"} class:on={filter === "problems"} onclick={() => (filter = "problems")}>
      Ошибки и предупреждения <span class="n">{counts.errors + counts.warns}</span>
    </button>
    <button class="chip" role="radio" aria-checked={filter === "errors"} class:on={filter === "errors"} onclick={() => (filter = "errors")}>
      Ошибки <span class="n">{counts.errors}</span>
    </button>
  </div>
</div>

{#if error}
  <p class="warn"><Icon name="error" size={18} />{error}</p>
{:else if !loaded}
  <p class="muted">Читаю лог…</p>
{:else}
  <div
    class="log"
    bind:this={box}
    bind:clientHeight={height}
    style:height="{fit}px"
    onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  >
    <div class="inner" style:height="{shown.length * ROW}px">
      {#each shown.slice(first, last) as line, k (line)}
        <div class="row lvl-{levels[line]}" class:active={line === activeLine} style:top="{(first + k) * ROW}px">
          <span class="no">{line + 1}</span>
          <span class="text">{#each parts(lines[line]) as p}{#if p.hit}<mark>{p.t}</mark>{:else}{p.t}{/if}{/each}</span>
        </div>
      {/each}
    </div>
    {#if shown.length === 0}
      <p class="muted empty">{filter === "errors" ? "Ошибок в этом логе нет." : "Таких строк нет."}</p>
    {/if}
  </div>
{/if}

<style>
  .tools {
    display: flex;
    align-items: center;
    gap: var(--s2);
    margin-bottom: var(--s3);
    flex-wrap: wrap;
  }
  .file {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .file b {
    font-family: ui-monospace, Consolas, monospace;
    font-size: var(--fs-14);
    overflow-wrap: anywhere;
  }
  .search {
    display: flex;
    align-items: center;
    gap: var(--s2);
    height: var(--h-md, 36px);
    padding: 0 var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--bg);
    width: 320px;
    color: var(--text-muted);
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .search input {
    flex: 1;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
  }
  .count {
    min-width: 72px;
  }
  .chips {
    display: flex;
    gap: var(--s2);
    flex-wrap: wrap;
  }
  .n {
    opacity: 0.7;
    margin-left: 2px;
  }
  .log {
    position: relative;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--bg);
    font-family: ui-monospace, Consolas, monospace;
    font-size: 12.5px;
  }
  .inner {
    position: relative;
    min-width: 100%;
    width: max-content;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 20px;
    line-height: 20px;
    display: flex;
    white-space: pre;
  }
  .row:hover {
    background: var(--surface);
  }
  .no {
    position: sticky;
    left: 0;
    flex: none;
    width: 56px;
    padding-right: var(--s2);
    text-align: right;
    color: var(--text-muted);
    background: var(--bg);
    opacity: 0.7;
    user-select: none;
  }
  .text {
    padding: 0 var(--s3) 0 var(--s2);
  }
  .row.lvl-error .text {
    color: var(--danger);
  }
  .row.lvl-warn .text {
    color: var(--warning, #e0a43a);
  }
  .row.lvl-debug .text {
    color: var(--text-muted);
  }
  .row.active {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  mark {
    background: color-mix(in srgb, var(--accent) 55%, transparent);
    color: var(--text);
    border-radius: 2px;
  }
  .empty {
    padding: var(--s4);
  }
</style>
