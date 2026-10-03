<script lang="ts">
  import { ICONS, type IconName } from "../lib/icons";

  let { name, size = 20 }: { name: IconName; size?: number } = $props();
</script>

<svg class="i" viewBox="0 0 24 24" width={size} height={size} aria-hidden="true">
  {@html ICONS[name]}
</svg>
