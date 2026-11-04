<script lang="ts">
  // Добавить офлайн-ник: в меню аккаунта и в мастере первого запуска.
  import { api } from "../../lib/api";
  import { setAccounts } from "../../lib/accounts.svelte";
  import { attempt, toast } from "../../lib/toast.svelte";

  let { ondone, oncancel }: { ondone: () => void; oncancel?: () => void } = $props();

  // Ник: 3–16 латинских букв, цифр и «_» — как требует игра.
  let nick = $state("");
  const ok = $derived(/^[A-Za-z0-9_]{3,16}$/.test(nick));
  let busy = $state(false);

  async function add() {
    busy = true;
    const list = await attempt("Ник", () => api.addOffline(nick));
    busy = false;
    if (list) {
      setAccounts(list);
      toast(`Ник ${nick} добавлен и выбран`);
      nick = "";
      ondone();
    }
  }
</script>

<form
  class="form"
  onsubmit={(e) => {
    e.preventDefault();
    if (ok && !busy) add();
  }}
>
  <label class="field">
    <span>Ник</span>
    <input class="input" bind:value={nick} maxlength="16" autocomplete="off" spellcheck="false" />
  </label>
  <p class="muted small">
    3–16 латинских букв, цифр или «_». Офлайн-ник работает в одиночной игре и на серверах без проверки лицензии.
    Скин видят только игроки с модом или на серверах со своими скинами.
  </p>
  <div class="buttons">
    {#if oncancel}<button type="button" class="btn btn-quiet" onclick={oncancel}>Отмена</button>{/if}
    <button class="btn btn-primary" disabled={!ok || busy}>Добавить</button>
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
