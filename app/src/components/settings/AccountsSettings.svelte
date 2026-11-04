<script lang="ts">
  // «Настройки → Аккаунты» (раздел 5.16 ТЗ): список, аккаунт по умолчанию,
  // добавить и удалить. То же, что в меню аккаунта в шапке, — на одном экране.
  import { api, type Account } from "../../lib/api";
  import { KIND, accounts, currentAccount, loadAccounts, setAccounts } from "../../lib/accounts.svelte";
  import { attempt, toast } from "../../lib/toast.svelte";
  import ElyForm from "../accounts/ElyForm.svelte";
  import NickForm from "../accounts/NickForm.svelte";
  import Icon from "../Icon.svelte";
  import Modal from "../Modal.svelte";

  loadAccounts();
  const current = $derived(currentAccount());
  let adding = $state<"nick" | "ely" | null>(null);
  let removing = $state<Account | null>(null);

  async function choose(a: Account) {
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

<div class="card block">
  {#if !accounts.loaded}
    <p class="muted">Загружаю…</p>
  {:else if !accounts.list.length}
    <p class="muted">Аккаунтов пока нет. Добавьте офлайн-ник или войдите через Ely.by — без аккаунта игра не запустится.</p>
  {:else}
    <ul class="list" role="radiogroup" aria-label="Аккаунт по умолчанию">
      {#each accounts.list as a (a.id)}
        <li>
          <label class="pick">
            <input type="radio" name="default-account" checked={a.id === current?.id} onchange={() => choose(a)} />
            <span class="avatar">{a.name[0].toUpperCase()}</span>
            <span class="grow"><b>{a.name}</b> <span class="muted small">· {KIND[a.kind]}</span></span>
            {#if a.id === current?.id}<span class="muted small">играет по умолчанию</span>{/if}
          </label>
          <button class="btn btn-sm btn-quiet" aria-label="Удалить аккаунт {a.name}" title="Удалить" onclick={() => (removing = a)}>
            <Icon name="trash" size={16} />
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  <div class="row-buttons">
    <button class="btn" onclick={() => (adding = "nick")}><Icon name="plus" size={18} />Добавить ник (офлайн)</button>
    <button class="btn" onclick={() => (adding = "ely")}><Icon name="user" size={18} />Войти через Ely.by</button>
  </div>
  <p class="muted small">
    Пароль Ely.by не сохраняется: лаунчер хранит только выданный ключ входа — в защищённом хранилище Windows. Вход
    через Microsoft появится после 1.0.
  </p>
</div>

{#if adding === "nick"}
  <Modal title="Добавить ник" width={440} onclose={() => (adding = null)}>
    <NickForm ondone={() => (adding = null)} oncancel={() => (adding = null)} />
  </Modal>
{:else if adding === "ely"}
  <Modal title="Вход через Ely.by" width={460} onclose={() => (adding = null)}>
    <ElyForm ondone={() => (adding = null)} oncancel={() => (adding = null)} />
  </Modal>
{/if}

{#if removing}
  {@const a = removing}
  <Modal title="Удалить аккаунт?" width={440} onclose={() => (removing = null)}>
    <p>
      Аккаунт <b>{a.name}</b> ({KIND[a.kind]}) пропадёт из лаунчера.
      {#if a.kind === "ely"}Ключ входа будет отозван на сервере Ely.by — сам аккаунт Ely.by не удаляется.{:else}Миры и
        сборки останутся.{/if}
    </p>
    {#snippet footer()}
      <button class="btn btn-quiet" onclick={() => (removing = null)}>Отмена</button>
      <button class="btn btn-primary" onclick={() => remove(a)}>Удалить</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .list {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: var(--s2);
    padding: var(--s2) 0;
    border-top: 1px solid var(--border);
  }
  .list li:first-child {
    border-top: 0;
    padding-top: 0;
  }
  .pick {
    display: flex;
    align-items: center;
    gap: var(--s3);
    flex: 1;
    min-width: 0;
    cursor: pointer;
  }
  .avatar {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--r-sm);
    background: var(--surface-raised);
    font-weight: 600;
  }
</style>
