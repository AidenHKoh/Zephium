<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";
  import { AlertCircleIcon } from "@hugeicons/core-free-icons";
  import { commands } from "$shared/ipc/bindings";
  import type { PageAssistFaultV1 } from "$shared/ipc/bindings";
  import { faultView } from "../lib/faults";
  import "./assist.css";

  let { fault, onretry }: { fault: PageAssistFaultV1; onretry: () => void } = $props();

  const view = $derived(faultView(fault));
  let left = $state(0);
  $effect(() => {
    left = view.retry === "wait" ? 20 : 0;
    if (left <= 0) return;
    const timer = setInterval(() => {
      left -= 1;
      if (left <= 0) clearInterval(timer);
    }, 1000);
    return () => clearInterval(timer);
  });
</script>

<section class="assist-card assist-rise" role="alert" aria-label={m.assist_fault_label()}>
  <div class="head">
    <span class="mark" aria-hidden="true"><Icon icon={AlertCircleIcon} size={16} /></span>
    <p>{view.message}</p>
  </div>
  {#if view.fix || view.retry !== "no"}
    <div class="assist-actions">
      {#if view.fix}{@const fix = view.fix}<Button
          variant={view.retry === "no" ? "primary" : "secondary"}
          size="compact"
          onclick={() => void commands.runCommand(fix.command).catch(() => {})}>{fix.label}</Button
        >{/if}
      {#if view.retry === "now" || view.retry === "wait"}<Button
          size="compact"
          disabled={left > 0}
          onclick={onretry}
          >{left > 0 ? m.assist_retry_in({ seconds: left }) : m.assist_retry()}</Button
        >{/if}
    </div>
  {/if}
</section>

<style>
  .head {
    display: flex;
    gap: 9px;
  }

  .head p {
    color: var(--color-label-secondary);
    font-size: 12.5px;
  }

  .mark {
    display: grid;
    flex: none;
    place-items: center;
    block-size: 18px;
    color: var(--color-warning);
  }
</style>
