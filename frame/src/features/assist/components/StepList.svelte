<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";
  import { Orb } from "$shared/ui/presence";
  import { AlertCircleIcon, Cancel01Icon, Tick02Icon } from "@hugeicons/core-free-icons";
  import type { WorkConfirmCategoryV1 } from "$shared/ipc/bindings";
  import type { PageAssistStepV1 } from "$shared/ipc/bindings";
  import "./assist.css";

  let {
    steps,
    site = null,
    ondecide,
  }: {
    steps: readonly PageAssistStepV1[];
    site?: string | null;
    ondecide: (approve: boolean) => void;
  } = $props();

  /** One quiet line on what pressing it does, from the core's category. */
  const consequence = (category: WorkConfirmCategoryV1 | null) => {
    const args = { site: site ?? m.assist_this_page() };
    switch (category) {
      case "communication":
        return m.assist_consequence_communication(args);
      case "purchase":
        return m.assist_consequence_purchase(args);
      case "destructive":
        return m.assist_consequence_destructive(args);
      case "save":
        return m.assist_consequence_save(args);
      case "edit":
        return m.assist_consequence_edit(args);
      case "type":
        return m.assist_consequence_type(args);
      default:
        return m.assist_consequence_other(args);
    }
  };
  const waiting = $derived(steps.find((step) => step.state === "confirm") ?? null);
  const settled = $derived(steps.filter((step) => step !== waiting));
</script>

{#if settled.length}
  <ol class="steps">
    {#each settled as step (step.index)}
      <li class="step assist-rise" data-state={step.state}>
        <span class="glyph" aria-hidden="true">
          {#if step.state === "running"}<Orb size={13} />{:else if step.state === "done"}<Icon
              icon={Tick02Icon}
              size={13}
              strokeWidth={2.2}
            />{:else if step.state === "failed"}<Icon
              icon={AlertCircleIcon}
              size={13}
            />{:else}<Icon icon={Cancel01Icon} size={12} strokeWidth={2.2} />{/if}
        </span>
        <span class="label">{step.label}</span>
        {#if step.state === "declined"}<span class="note">{m.assist_step_declined()}</span
          >{:else if step.state === "skipped"}<span class="note">{m.assist_step_skipped()}</span
          >{:else if step.state === "failed"}<span class="note">{m.assist_step_failed()}</span>{/if}
      </li>
    {/each}
  </ol>
{/if}

{#if waiting}
  <section class="assist-card confirm assist-rise" aria-label={m.assist_confirm_label()}>
    <h4>{m.assist_confirm_title({ control: waiting.label })}</h4>
    <p>{consequence(waiting.category)}</p>
    <div class="assist-actions">
      <Button variant="primary" size="compact" class="control" onclick={() => ondecide(true)}
        >{waiting.label}</Button
      >
      <Button size="compact" onclick={() => ondecide(false)}>{m.assist_decline()}</Button>
    </div>
  </section>
{/if}

<style>
  .steps {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .step {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--color-muted);
    font-size: 12.5px;
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
    color: var(--color-success);
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

  .note {
    flex: none;
    color: var(--color-faint);
  }

  .confirm :global(.control) {
    max-inline-size: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
