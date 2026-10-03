<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import type { Section } from "../lib/sections";

  let { section }: { section: Section } = $props();
</script>

<div class="empty">
  <span class="icon"><Icon name={section.icon} size={32} /></span>
  <h1>{section.title}</h1>
  <p class="muted">{section.about}</p>
  <p class="stage">Появится на этапе {section.stage}</p>
</div>

<style>
  .empty {
    height: 100%;
    display: grid;
    place-content: center;
    justify-items: center;
    text-align: center;
    gap: var(--s2);
  }
  .icon {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    margin-bottom: var(--s2);
    border: 1px solid var(--border);
    border-radius: var(--r-card);
    background: var(--surface);
    color: var(--text-muted);
  }
  h1 {
    margin: 0;
    font-size: var(--fs-24);
    font-weight: 600;
  }
  p {
    margin: 0;
    max-width: 420px;
  }
  .stage {
    margin-top: var(--s2);
    padding: 2px var(--s3);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: var(--fs-14);
    color: var(--text-muted);
  }
</style>
