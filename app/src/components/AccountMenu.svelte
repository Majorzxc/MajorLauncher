<script lang="ts">
  // Аккаунт в шапке: по клику — список аккаунтов, добавить ник, войти через
  // Ely.by, удалить. Выбранный аккаунт запускает игру.
  import { fly } from "svelte/transition";
  import { api, type Account } from "../lib/api";
  import { KIND, accounts, currentAccount, loadAccounts, setAccounts } from "../lib/accounts.svelte";
  import { ms } from "../lib/motion";
  import { attempt, toast } from "../lib/toast.svelte";
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

  // Ник: 3–16 латинских букв, цифр и «_» — как требует игра.
  let nick = $state("");
  const nickOk = $derived(/^[A-Za-z0-9_]{3,16}$/.test(nick));
  async function addNick() {
    const list = await attempt("Ник", () => api.addOffline(nick));
    if (list) {
      setAccounts(list);
      toast(`Ник ${nick} добавлен и выбран`);
      dialog = null;
      nick = "";
    }
  }

  // Ely.by: пароль уходит прямо на сервер Ely.by и нигде не сохраняется.
  let login = $state("");
  let password = $state("");
  let code = $state("");
  let needCode = $state(false);
  let elyBusy = $state(false);
  let elyError = $state("");
  async function addEly() {
    elyBusy = true;
    elyError = "";
    try {
      const r = await api.addEly(login, password, needCode ? code : null);
      if (r.status === "need_code") {
        needCode = true;
      } else {
        setAccounts(r.accounts);
        toast("Вход через Ely.by выполнен");
        closeEly();
      }
    } catch (e) {
      elyError = String(e);
    } finally {
      elyBusy = false;
    }
  }
  function closeEly() {
    dialog = null;
    password = "";
    code = "";
    needCode = false;
    elyError = "";
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

<div class="wrap" bind:this={root}>
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
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        if (nickOk) addNick();
      }}
    >
      <label class="field">
        <span>Ник</span>
        <input class="input" bind:value={nick} maxlength="16" autocomplete="off" spellcheck="false" />
      </label>
      <p class="muted small">
        3–16 латинских букв, цифр или «_». Офлайн-ник работает в одиночной игре и на серверах без проверки
        лицензии. Скин видят только игроки с модом или на серверах со своими скинами.
      </p>
      <div class="buttons">
        <button type="button" class="btn btn-quiet" onclick={() => (dialog = null)}>Отмена</button>
        <button class="btn btn-primary" disabled={!nickOk}>Добавить</button>
      </div>
    </form>
  </Modal>
{/if}

{#if dialog === "ely"}
  <Modal title="Вход через Ely.by" width={460} onclose={closeEly}>
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        if (login && password) addEly();
      }}
    >
      <label class="field">
        <span>Ник или почта</span>
        <input class="input" bind:value={login} autocomplete="username" spellcheck="false" disabled={needCode} />
      </label>
      <label class="field">
        <span>Пароль</span>
        <input class="input" type="password" bind:value={password} autocomplete="current-password" disabled={needCode} />
      </label>
      {#if needCode}
        <label class="field">
          <span>Код из приложения-аутентификатора</span>
          <input class="input" bind:value={code} inputmode="numeric" maxlength="8" autocomplete="one-time-code" />
        </label>
      {/if}
      <p class="muted small">
        Пароль уходит прямо на ely.by и нигде не сохраняется. Лаунчер хранит только выданный ключ входа —
        в защищённом хранилище Windows.
      </p>
      {#if elyError}<p class="warn"><Icon name="error" size={18} />{elyError}</p>{/if}
      <div class="buttons">
        <button type="button" class="btn btn-quiet" onclick={closeEly}>Отмена</button>
        <button class="btn btn-primary" disabled={elyBusy || !login || !password || (needCode && !code)}>
          {elyBusy ? "Вход…" : "Войти"}
        </button>
      </div>
    </form>
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
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--s4);
  }
  .form p {
    margin: 0;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: var(--s2);
  }
</style>
