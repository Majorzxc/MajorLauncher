<script lang="ts">
  import { builds } from "../lib/builds.svelte";

  let { value = $bindable() }: { value: string | null } = $props();

  const NEW = "\u0000new";
  let typing = $state(false);
  let draft = $state("");
</script>

{#if typing}
  <div class="new">
    <input
      placeholder="Название новой группы"
      maxlength="32"
      bind:value={draft}
      onkeydown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          value = draft.trim() || null;
          typing = false;
        }
        if (e.key === "Escape") {
          e.stopPropagation();
          typing = false;
        }
      }}
    />
    <button
      class="btn btn-sm"
      onclick={() => {
        value = draft.trim() || null;
        typing = false;
      }}>Готово</button
    >
  </div>
{:else}
  <select
    value={value ?? ""}
    onchange={(e) => {
      const v = e.currentTarget.value;
      if (v === NEW) {
        draft = "";
        typing = true;
        e.currentTarget.value = value ?? "";
      } else value = v || null;
    }}
  >
    <option value="">Без группы</option>
    {#each builds.groups as g (g)}<option value={g}>{g}</option>{/each}
    {#if value && !builds.groups.includes(value)}<option value={value}>{value}</option>{/if}
    <option value={NEW}>+ Новая группа…</option>
  </select>
{/if}

<style>
  select,
  input {
    width: 100%;
    height: var(--h-md);
    padding: 0 var(--s3);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: var(--bg);
    outline: none;
    user-select: text;
  }
  select:focus,
  input:focus {
    border-color: var(--accent);
  }
  option {
    background: var(--surface);
  }
  .new {
    display: flex;
    gap: var(--s2);
  }
</style>
