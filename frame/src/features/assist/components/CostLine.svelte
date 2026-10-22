<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import type { PageAssistCostV1 } from "$shared/ipc/bindings";

  let { cost }: { cost: PageAssistCostV1 } = $props();

  const line = $derived(
    cost.kind === "local"
      ? m.assist_cost_local({ model: cost.model })
      : cost.kind === "byok"
        ? cost.usd === null
          ? m.assist_cost_confirming({ model: cost.model })
          : m.assist_cost_own({ amount: `$${cost.usd}`, provider: cost.provider })
        : cost.credits === null
          ? m.assist_cost_confirming({ model: cost.model })
          : cost.credits === "1"
            ? m.assist_cost_credit({ model: cost.model })
            : m.assist_cost_credits({ model: cost.model, credits: cost.credits }),
  );
</script>

<p class="cost">{line}</p>

<style>
  .cost {
    margin: 0;
    color: var(--color-faint);
    font-size: var(--text-caption);
    font-variant-numeric: tabular-nums;
    line-height: 16px;
  }
</style>
