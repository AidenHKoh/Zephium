<script lang="ts">
  import type { WorkModelReasoning } from "$shared/ipc/bindings";

  let { levels, level }: { levels: readonly WorkModelReasoning[]; level: WorkModelReasoning } =
    $props();

  const POINTS = 8;
  const lit = $derived(
    Math.max(1, Math.round(((levels.indexOf(level) + 1) / levels.length) * POINTS)),
  );
  const dots = Array.from({ length: POINTS }, (_, at) => {
    const turn = (at / POINTS) * Math.PI * 2 - Math.PI / 2;
    return { x: 7 + Math.cos(turn) * 5.1, y: 7 + Math.sin(turn) * 5.1, t: at / (POINTS - 1) };
  });
</script>

<svg class="gauge" width="14" height="14" viewBox="0 0 14 14" data-level={level} aria-hidden="true">
  {#each dots as dot, at (at)}<circle
      cx={dot.x}
      cy={dot.y}
      r="1.05"
      data-on={at < lit ? "" : undefined}
      style:--t={dot.t}
    />{/each}
</svg>

<style>
  .gauge {
    --alive-a: color-mix(in oklab, var(--color-soft-sky) 66%, var(--color-text));
    --alive-b: color-mix(in oklab, var(--color-soft-lilac) 66%, var(--color-text));

    display: block;
    flex: none;
  }

  circle {
    fill: currentcolor;
    opacity: 0.3;
    transition: opacity var(--motion-base) var(--ease-out);
  }

  circle[data-on] {
    opacity: 0.92;
  }

  .gauge[data-level="max"] circle[data-on] {
    fill: color-mix(in oklab, var(--alive-a), var(--alive-b) calc(var(--t) * 100%));
    opacity: 1;
  }

  @media (prefers-reduced-motion: reduce) {
    circle {
      transition: none;
    }
  }
</style>
