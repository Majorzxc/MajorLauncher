<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  // Окно ничего не знает само: даже версию спрашивает у ядра.
  const version = invoke<string>("core_version");
</script>

<main>
  <h1>MajorLauncher</h1>
  {#await version}
    <p class="muted">Подключение к ядру…</p>
  {:then v}
    <p class="muted">Ядро {v} · каркас этапа 0</p>
  {:catch}
    <p class="muted">Ядро недоступно: окно открыто не из приложения</p>
  {/await}
</main>

<style>
  main {
    height: 100%;
    display: grid;
    place-content: center;
    text-align: center;
  }
  h1 {
    margin: 0 0 8px;
    font-size: 32px;
    font-weight: 600;
  }
  .muted {
    margin: 0;
    color: var(--text-muted);
  }
</style>
