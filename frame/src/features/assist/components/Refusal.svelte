<script lang="ts">
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";
  import { LockIcon } from "@hugeicons/core-free-icons";
  import { commands } from "$shared/ipc/bindings";
  import { refusalView } from "../lib/faults";
  import type { Refusal } from "../lib/assist-session.svelte";
  import "./assist.css";

  let { refusal }: { refusal: Refusal } = $props();
  const view = $derived(refusalView(refusal));
</script>

<section class="refusal assist-rise" role="status">
  <span class="plate" aria-hidden="true"><Icon icon={LockIcon} size={18} /></span>
  <h3>{view.title}</h3>
  <p>{view.help}</p>
  {#if view.fix}{@const fix = view.fix}<Button
      size="compact"
      onclick={() => void commands.runCommand(fix.command).catch(() => {})}>{fix.label}</Button
    >{/if}
</section>

<style>
  .refusal {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    padding-block: 28px 8px;
  }

  .plate {
    display: grid;
    place-items: center;
    inline-size: 36px;
    block-size: 36px;
    margin-block-end: 6px;
    border-radius: var(--radius-row);
    background: var(--color-fill);
    box-shadow: inset 0 0 0 1px var(--color-border);
    color: var(--color-muted);
  }

  h3 {
    margin: 0;
    color: var(--color-text);
    font-size: 17px;
    font-weight: 620;
    letter-spacing: -0.016em;
    line-height: 1.3;
  }

  p {
    margin: 0 0 4px;
    max-inline-size: 28ch;
    color: var(--color-muted);
    font-size: var(--text-body);
    line-height: 1.5;
    text-wrap: pretty;
  }
</style>
