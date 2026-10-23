<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";
  import { Orb } from "$shared/ui/presence";
  import type { PageAssistDecisionV1, PageAssistStepV1 } from "$shared/ipc/bindings";
  import { consequence, controlOf } from "../../lib/format";
  import { AlertCircleIcon, Cancel01Icon, Tick02Icon } from "../../lib/icons";
  import "$shared/ui/Shine/shine.css";
  import "../assist.css";

  let {
    steps,
    site,
    ondecide,
  }: {
    steps: readonly PageAssistStepV1[];
    site: string | null;
    ondecide: (decision: PageAssistDecisionV1) => void;
  } = $props();

  const waiting = $derived(steps.find((step) => step.state === "confirm") ?? null);
  let answered = $state(false);
  $effect(() => {
    void waiting;
    answered = false;
  });

  function decide(decision: PageAssistDecisionV1) {
    answered = true;
    ondecide(decision);
  }
</script>

<ol class="steps" aria-label={m.assist_steps_label()}>
  {#each steps as step (step.index)}
    {#if step === waiting}
      <li class="confirm assist-card ui-shine assist-rise" aria-label={m.assist_confirm_label()}>
        <h4>{m.assist_confirm_title({ control: controlOf(step.label) })}</h4>
        <p>{consequence(step.category, site)}</p>
        <div class="assist-actions">
          <Button
            variant="primary"
            size="compact"
            class="control"
            disabled={answered}
            onclick={() => decide("approve")}>{controlOf(step.label)}</Button
          >
          <Button
            variant="ghost"
            size="compact"
            disabled={answered}
            onclick={() => decide("decline")}>{m.assist_decline()}</Button
          >
        </div>
      </li>
    {:else}
      <li class="step" data-state={step.state}>
        <span class="glyph" aria-hidden="true">
          {#if step.state === "running"}<Orb size={13} />{:else if step.state === "done"}<Icon
              icon={Tick02Icon}
              size={13}
              strokeWidth={2.2}
            />{:else if step.state === "failed"}<Icon
              icon={AlertCircleIcon}
              size={13}
            />{:else}<Icon icon={Cancel01Icon} size={11} strokeWidth={2.2} />{/if}
        </span>
        <span class="label">{step.label}</span>
        {#if step.state === "declined"}<span class="note">{m.assist_step_declined()}</span
          >{:else if step.state === "skipped"}<span class="note">{m.assist_step_skipped()}</span
          >{:else if step.state === "failed"}<span class="note">{m.assist_step_failed()}</span>{/if}
      </li>
    {/if}
  {/each}
</ol>

<style>
  .steps {
    display: flex;
    flex-direction: column;
    gap: 7px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .confirm {
    margin-block: 4px;
  }

  .step {
    display: flex;
    align-items: center;
    gap: 9px;
    padding-inline: 2px;
    color: var(--color-label-secondary);
    font-size: 13px;
    line-height: 18px;
  }

  .glyph {
    display: grid;
    flex: none;
    place-items: center;
    inline-size: 14px;
    block-size: 14px;
    color: var(--color-faint);
  }

  .step[data-state="done"] .glyph {
    color: var(--color-text);
  }

  .step[data-state="failed"] .glyph {
    color: var(--color-warning);
  }

  .label {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .step:is([data-state="declined"], [data-state="skipped"]) .label {
    color: var(--color-faint);
  }

  .note {
    flex: none;
    margin-inline-start: auto;
    color: var(--color-faint);
    font-size: 12px;
  }

  .confirm :global(.control) {
    max-inline-size: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
