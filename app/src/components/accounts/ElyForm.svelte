<script lang="ts">
  // Вход через Ely.by: в меню аккаунта и в мастере первого запуска. Пароль
  // уходит прямо на сервер Ely.by и нигде не сохраняется.
  import { api } from "../../lib/api";
  import { setAccounts } from "../../lib/accounts.svelte";
  import { toast } from "../../lib/toast.svelte";
  import Icon from "../Icon.svelte";

  let { ondone, oncancel }: { ondone: () => void; oncancel?: () => void } = $props();

  let login = $state("");
  let password = $state("");
  let code = $state("");
  let needCode = $state(false);
  let busy = $state(false);
  let error = $state("");

  async function add() {
    busy = true;
    error = "";
    try {
      const r = await api.addEly(login, password, needCode ? code : null);
      if (r.status === "need_code") {
        needCode = true;
      } else {
        setAccounts(r.accounts);
        toast("Вход через Ely.by выполнен");
        password = "";
        code = "";
        needCode = false;
        ondone();
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<form
  class="form"
  onsubmit={(e) => {
    e.preventDefault();
    if (login && password && !busy) add();
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
    Пароль уходит прямо на ely.by и нигде не сохраняется. Лаунчер хранит только выданный ключ входа — в защищённом
    хранилище Windows.
  </p>
  {#if error}<p class="warn"><Icon name="error" size={18} />{error}</p>{/if}
  <div class="buttons">
    {#if oncancel}<button type="button" class="btn btn-quiet" onclick={oncancel}>Отмена</button>{/if}
    <button class="btn btn-primary" disabled={busy || !login || !password || (needCode && !code)}>
      {busy ? "Вход…" : "Войти"}
    </button>
  </div>
</form>

<style>
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
