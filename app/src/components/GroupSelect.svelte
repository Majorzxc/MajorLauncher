<script lang="ts">
  // Группа сборки: общий выпадающий список лаунчера, последний пункт —
  // «Новая группа…», он превращается в поле ввода.
  import { builds } from "../lib/builds.svelte";
  import Select from "./Select.svelte";

  let { value = $bindable() }: { value: string | null } = $props();

  const NEW = "\u0000new";
  let typing = $state(false);
  let draft = $state("");

  const options = $derived([
    { value: "", label: "Без группы" },
    ...builds.groups.map((g) => ({ value: g, label: g })),
    ...(value && !builds.groups.includes(value) ? [{ value, label: value }] : []),
    { value: NEW, label: "+ Новая группа…" },
  ]);

  function done() {
    value = draft.trim() || value;
    typing = false;
  }
</script>

{#if typing}
  <div class="new">
    <!-- svelte-ignore a11y_autofocus -->
    <input
      class="input"
      type="search"
      placeholder="Название новой группы"
      maxlength="32"
      autofocus
      bind:value={draft}
      onkeydown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          done();
        }
        if (e.key === "Escape") {
          e.stopPropagation();
          typing = false;
        }
      }}
    />
    <button class="btn" onclick={done}>Готово</button>
  </div>
{:else}
  <Select
    label="Группа"
    value={value ?? ""}
    {options}
    onchange={(v) => {
      if (v === NEW) {
        draft = "";
        typing = true;
      } else value = v || null;
    }}
  />
{/if}

<style>
  .new {
    display: flex;
    gap: var(--s2);
  }
  .new input {
    flex: 1;
  }
</style>
