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
    ondecide,
  }: { steps: readonly PageAssistStepV1[]; ondecide: (approve: boolean) => void } = $props();

  const names: Record<WorkConfirmCategoryV1, () => string> = {
    communication: m.assist_category_communication,
    purchase: m.assist_category_purchase,
    destructive: m.assist_category_destructive,
    save: m.assist_category_save,
    edit: m.assist_category_edit,
    type: m.assist_category_type,
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
    <p class="kind">
      {waiting.category ? names[waiting.category]() : m.assist_category_other()}
    </p>
    <h4>{m.assist_confirm_title()}</h4>
    <p class="control">{waiting.label}</p>
    <div class="assist-actions">
      <Button variant="primary" size="compact" onclick={() => ondecide(true)}
        >{m.assist_approve()}</Button
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

  .kind {
    color: var(--color-faint);
    font-size: var(--text-caption);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  /* The exact control, quoted as the page names it, so approving is never approving a paraphrase. */
  .control {
    padding: 8px 10px;
    border-radius: var(--radius-inset);
    background: var(--color-field);
    color: var(--color-text);
    font-size: var(--text-body);
    font-weight: 550;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }
</style>
