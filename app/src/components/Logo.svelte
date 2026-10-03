<script lang="ts">
  import { LOGO } from "../lib/logo";

  let { size = 48 }: { size?: number } = $props();
</script>

<!-- Пиксельный знак: три тона берутся из темы, поэтому он монохромный
     в чёрной и белой темах и подстраивается под свой акцент. -->
<svg viewBox="0 0 16 16" width={size} height={size} role="img" aria-label="MajorLauncher">
  <path d={LOGO.L} fill="var(--logo-lo, var(--border-strong))" />
  <path d={LOGO.M} fill="var(--logo-mid, var(--text-muted))" />
  <path d={LOGO.H} fill="var(--logo-hi, var(--accent))" />
</svg>

<style>
  svg {
    display: block;
    shape-rendering: crispEdges;
  }
</style>
