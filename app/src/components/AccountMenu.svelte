<script lang="ts">
  // Аккаунт в шапке: по клику — список аккаунтов, добавить ник, войти через
  // Ely.by, удалить. Выбранный аккаунт запускает игру.
  import { fly } from "svelte/transition";
  import { api, type Account } from "../lib/api";
  import { KIND, accounts, currentAccount, loadAccounts, setAccounts } from "../lib/accounts.svelte";
  import { ms } from "../lib/motion";
  import { attempt, toast } from "../lib/toast.svelte";
  import ElyForm from "./accounts/ElyForm.svelte";
  import NickForm from "./accounts/NickForm.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let open = $state(false);
  let root: HTMLDivElement;
  let dialog: "nick" | "ely" | null = $state(null);
  let removing: Account | null = $state(null);

  const current = $derived(currentAccount());
  loadAccounts();

  async function choose(a: Account) {
    open = false;
    const list = await attempt("Аккаунт", () => api.setDefaultAccount(a.id));
    if (list) setAccounts(list);
  }

  async function remove(a: Account) {
    removing = null;
    const list = await attempt("Удаление аккаунта", () => api.removeAccount(a.id));
    if (list) {
      setAccounts(list);
      toast(`Аккаунт ${a.name} удалён`);
    }
  }
</script>

<svelte:window
  onmousedown={(e) => open && !root.contains(e.target as Node) && (open = false)}
  onkeydown={(e) => e.key === "Escape" && (open = false)}
/>

<div class="wrap" bind:this={root} data-tour="account">
  <button class="acct" aria-haspopup="menu" aria-expanded={open} onclick={() => (open = !open)}>
    {#if current}
      <span class="avatar">{current.name[0].toUpperCase()}</span>
      <span class="who"><b>{current.name}</b><span class="muted">· {KIND[current.kind]}</span></span>
    {:else}
      <span class="avatar"><Icon name="user" size={18} /></span>
      <span class="muted">{accounts.loaded ? "Добавьте аккаунт" : ""}</span>
    {/if}
    <Icon name="chevron-down" size={16} />
  </button>

  {#if open}
    <div class="menu" role="menu" in:fly={{ y: -4, duration: ms(120) }} out:fly={{ y: -4, duration: ms(80) }}>
      {#if accounts.list.length}
        <p class="caption">Играть под аккаунтом</p>
        {#each accounts.list as a (a.id)}
          <div class="item" class:on={a.id === current?.id}>
            <button class="pick" role="menuitemradio" aria-checked={a.id === current?.id} onclick={() => choose(a)}>
              <span class="avatar sm">{a.name[0].toUpperCase()}</span>
              <span class="grow"><b>{a.name}</b> <span class="muted small">{KIND[a.kind]}</span></span>
              {#if a.id === current?.id}<Icon name="check" size={18} />{/if}
            </button>
            <button
              class="remove"
              aria-label="Удалить аккаунт {a.name}"
              title="Удалить аккаунт"
              onclick={() => ((removing = a), (open = false))}><Icon name="close" size={16} /></button
            >
          </div>
        {/each}
        <hr />
      {/if}
      <button class="action" role="menuitem" onclick={() => ((dialog = "nick"), (open = false))}>
        <Icon name="plus" size={18} />Добавить ник (офлайн)
      </button>
      <button class="action" role="menuitem" onclick={() => ((dialog = "ely"), (open = false))}>
        <Icon name="user" size={18} />Войти через Ely.by
      </button>
    </div>
  {/if}
</div>

{#if dialog === "nick"}
  <Modal title="Добавить ник" width={440} onclose={() => (dialog = null)}>
    <NickForm ondone={() => (dialog = null)} oncancel={() => (dialog = null)} />
  </Modal>
{/if}

{#if dialog === "ely"}
  <Modal title="Вход через Ely.by" width={460} onclose={() => (dialog = null)}>
    <ElyForm ondone={() => (dialog = null)} oncancel={() => (dialog = null)} />
  </Modal>
{/if}

{#if removing}
  {@const a = removing}
  <Modal title="Удалить аккаунт?" width={440} onclose={() => (removing = null)}>
    <p>
      Аккаунт <b>{a.name}</b> ({KIND[a.kind]}) пропадёт из лаунчера.
      {#if a.kind === "ely"}Ключ входа будет отозван на сервере Ely.by — сам аккаунт Ely.by не удаляется.{:else}Миры
        и сборки останутся.{/if}
    </p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (removing = null)}>Отмена</button>
      <button class="btn btn-primary" onclick={() => remove(a)}>Удалить</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .wrap {
    position: relative;
  }
  .acct {
    display: inline-flex;
    align-items: center;
    gap: var(--s3);
    height: 48px;
    padding: 0 var(--s3) 0 var(--s2);
    border: 1px solid var(--border);
    border-radius: var(--r);
    background: none;
    white-space: nowrap;
    cursor: pointer;
  }
  .acct:hover,
  .acct[aria-expanded="true"] {
    border-color: var(--border-strong);
    background: var(--surface-raised);
  }
  .avatar {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--r-sm);
    background: var(--surface-raised);
    color: var(--text);
    font-weight: 600;
  }
  .avatar.sm {
    width: 28px;
    height: 28px;
    font-size: var(--fs-14);
  }
  .who {
    display: inline-flex;
    gap: 6px;
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    z-index: 60;
    width: 300px;
    padding: var(--s1);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--r);
  }
  .caption {
    margin: var(--s2) var(--s3) var(--s1);
    font-size: var(--fs-12);
    color: var(--text-muted);
  }
  .item {
    display: flex;
    align-items: center;
    border-radius: var(--r-sm);
  }
  .item:hover {
    background: var(--surface-raised);
  }
  .pick,
  .action {
    display: flex;
    align-items: center;
    gap: var(--s3);
    flex: 1;
    min-width: 0;
    height: 44px;
    padding: 0 var(--s3);
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    text-align: left;
    font-size: var(--fs-14);
    cursor: pointer;
  }
  .action {
    width: 100%;
    height: 36px;
  }
  .action:hover {
    background: var(--surface-raised);
  }
  .remove {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    margin-right: var(--s1);
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0;
  }
  .item:hover .remove,
  .remove:focus-visible {
    opacity: 1;
  }
  .remove:hover {
    color: var(--danger);
  }
  hr {
    margin: var(--s1) 0;
    border: 0;
    border-top: 1px solid var(--border);
  }
</style>
