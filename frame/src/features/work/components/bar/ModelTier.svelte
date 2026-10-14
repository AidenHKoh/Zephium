<script module lang="ts">
  import type { WorkModelEntry } from "$shared/ipc/bindings";

  export type Tier = "light" | "balanced" | "flagship";

  const DOTS: Record<Tier, number> = { light: 0, balanced: 1, flagship: 2 };

  /**
   * A model's class from what its output costs, per million tokens. The dearest
   * of a provider's own models is never called lightweight while a cheaper
   * sibling exists: that is the provider's own flagship, whatever it costs.
   */
  export function tiers(groups: readonly { entries: readonly WorkModelEntry[] }[]) {
    const out: Record<string, Tier> = {};
    for (const group of groups) {
      const priced = group.entries.filter((entry) => entry.price);
      const top = Math.max(...priced.map((entry) => entry.price!.output));
      for (const entry of priced) {
        const output = entry.price!.output / 1_000_000;
        let tier: Tier = output < 6 ? "light" : output < 15 ? "balanced" : "flagship";
        if (tier === "light" && priced.length > 1 && entry.price!.output === top && output >= 3)
          tier = "balanced";
        out[entry.id] = tier;
      }
    }
    return out;
  }
</script>

<script lang="ts">
  let { tier }: { tier: Tier } = $props();
  const lit = $derived(DOTS[tier] + 1);
</script>

<!-- Thought as dots, like the orb: one for a lightweight model, a full constellation for a flagship. -->
<svg class="tier" data-tier={tier} width="13" height="13" viewBox="0 0 13 13" aria-hidden="true">
  <circle class:on={lit >= 1} cx="3" cy="9.6" r="1.75" />
  <circle class:on={lit >= 2} cx="10" cy="9.6" r="1.75" />
  <circle class:on={lit >= 3} cx="6.5" cy="3.3" r="1.75" />
</svg>

<style>
  .tier {
    display: block;
    flex: none;
  }

  circle {
    fill: currentcolor;
    opacity: 0.22;
    transition: opacity var(--motion-base) var(--ease-out);
  }

  circle.on {
    opacity: 1;
  }

  @media (prefers-reduced-motion: reduce) {
    circle {
      transition: none;
    }
  }
</style>
