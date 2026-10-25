<script lang="ts">
  // «Перезапустить и вернуться» (раздел 5.4 ТЗ): новые моды подключаются только
  // при запуске игры. Лаунчер закроет игру (или дождётся выхода) и сразу
  // запустит снова — в тот же мир или на тот же сервер.
  import { api } from "../lib/api";
  import { toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let error = $state("");

  async function arm(closeNow: boolean) {
    error = "";
    try {
      await api.playRestart(closeNow);
      toast(closeNow ? "Игра закрывается — сейчас запустится снова" : "Когда выйдете из игры, лаунчер сразу запустит её снова");
      onclose();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<Modal title="Перезапустить и вернуться" width={540} {onclose}>
  <p>
    Новые моды и их настройки игра подхватывает только при запуске. Лаунчер перезапустит игру и сразу вернёт вас в тот
    же мир или на тот же сервер — без лишних кликов.
  </p>
  <ul class="what">
    <li><b>Закрыть игру сейчас</b> — как крестиком окна: мир сохранится, как при выходе через меню.</li>
    <li><b>После выхода</b> — доиграете и выйдете сами, лаунчер подхватит.</li>
  </ul>
  <p class="muted small">
    Перезапуск занимает обычное время загрузки сборки. В мир лаунчер вернёт на 1.20 и новее; на старых версиях — на
    сервер, а одиночный мир откроется из меню. Если игра упадёт — перезапуска не будет, покажется ошибка.
  </p>
  {#if error}<p class="bad"><Icon name="error" size={18} />{error}</p>{/if}
  {#snippet footer()}
    <button class="btn btn-quiet back" onclick={onclose}>Отмена</button>
    <button class="btn" onclick={() => arm(false)}>После выхода</button>
    <button class="btn btn-primary" onclick={() => arm(true)}><Icon name="refresh" size={18} />Закрыть игру сейчас</button>
  {/snippet}
</Modal>

<style>
  p {
    margin: 0 0 var(--s3);
  }
  .what {
    margin: 0 0 var(--s3);
    padding-left: var(--s5);
  }
  .bad {
    display: flex;
    gap: 6px;
    color: var(--danger);
  }
  .back {
    margin-right: auto;
  }
</style>
