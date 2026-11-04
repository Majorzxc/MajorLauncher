<script lang="ts">
  // Добавить сервер или изменить: название, адрес и что делать с ресурспаком
  // сервера — как окно «Изменить» в игре. Проверку адреса делает ядро.
  import { untrack } from "svelte";
  import { api, type ServerRow, type ServerTextures } from "../../lib/api";
  import { pingServers, servers } from "../../lib/servers.svelte";
  import Modal from "../Modal.svelte";
  import Select from "../Select.svelte";

  let { server = null, onclose }: { server?: ServerRow | null; onclose: () => void } = $props();

  // Начальные значения — один раз при открытии окна.
  const initial = untrack(() => server);
  let name = $state(initial?.name ?? "");
  let address = $state(initial?.address ?? "");
  let textures = $state<ServerTextures>(initial?.textures ?? "ask");
  let error = $state("");
  let saving = $state(false);

  const TEXTURES: { value: ServerTextures; label: string }[] = [
    { value: "ask", label: "Спрашивать" },
    { value: "accept", label: "Принимать" },
    { value: "deny", label: "Не принимать" },
  ];

  async function save(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    error = "";
    try {
      const build = servers.build;
      if (initial) {
        servers.view = await api.serverEdit(build, initial.index, initial.address, { name, address, textures });
      } else {
        servers.view = await api.serverAdd(build, name, address);
        if (textures !== "ask") {
          const added = servers.view.servers.at(-1);
          if (added) servers.view = await api.serverEdit(build, added.index, added.address, { textures });
        }
      }
      const target = address.trim();
      onclose();
      if (!initial || initial.address.trim().toLowerCase() !== target.toLowerCase()) pingServers([target]);
    } catch (err) {
      error = String(err);
    } finally {
      saving = false;
    }
  }
</script>

<Modal title={initial ? "Изменить сервер" : "Добавить сервер"} width={460} {onclose}>
  <form id="server-form" class="form" onsubmit={save}>
    <label class="field">
      <span>Название</span>
      <input class="input" bind:value={name} maxlength="64" placeholder="Сервер Minecraft" />
    </label>
    <label class="field">
      <span>Адрес</span>
      <input
        class="input"
        bind:value={address}
        maxlength="128"
        placeholder="play.example.org или 1.2.3.4:25565"
        spellcheck="false"
        autocomplete="off"
      />
    </label>
    <div class="field">
      <span>Ресурспак сервера</span>
      <Select label="Ресурспак сервера" value={textures} options={TEXTURES} onchange={(v) => (textures = v)} />
    </div>
    {#if error}<p class="warn">{error}</p>{/if}
    <p class="muted small">
      {servers.build
        ? "Сервер появится только в этой сборке — у неё свой список."
        : "Сервер появится во всех сборках с общим списком и в самой игре."}
    </p>
  </form>
  {#snippet footer()}
    <button class="btn btn-quiet" type="button" onclick={onclose}>Отмена</button>
    <button class="btn btn-primary" type="submit" form="server-form" disabled={saving || !address.trim()}>
      {initial ? "Сохранить" : "Добавить"}
    </button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--s4);
  }
  .warn {
    margin: 0;
  }
  p.muted {
    margin: 0;
  }
</style>
