<script lang="ts">
  import { tick } from "svelte";
  import * as m from "$shared/i18n/messages";
  import Icon from "$shared/ui/Icon";
  import { ArrowUp02Icon, StopIcon, TextSelectionIcon } from "@hugeicons/core-free-icons";
  import { ASK_LIMIT, type AssistSession } from "../lib/assist-session.svelte";
  import ModelChip from "./ModelChip.svelte";

  let {
    session,
    draft,
    ondraft,
  }: {
    session: AssistSession;
    draft: string;
    ondraft: (text: string) => void;
  } = $props();

  let field = $state<HTMLTextAreaElement>();
  let useSelection = $state(false);
  const selection = $derived(session.status?.selection ?? null);
  const off = $derived(!!session.refusal || session.unavailable);
  const busy = $derived(session.busy);
  const ready = $derived(draft.trim().length > 0 && !off && !busy && !session.loading);
  // A selection that is gone cannot be asked about.
  $effect(() => {
    if (selection === null) useSelection = false;
  });

  /** Moves the caret into the field; the host calls it when Ask is asked for again. */
  export function focus() {
    field?.focus();
  }

  function grow() {
    if (!field) return;
    field.style.blockSize = "auto";
    field.style.blockSize = `${Math.min(field.scrollHeight, 132)}px`;
  }
  $effect(() => {
    void draft;
    void tick().then(grow);
  });

  async function send() {
    if (!ready) return;
    const sent = await session.ask(draft, useSelection ? "selection" : "ask");
    if (sent) {
      ondraft("");
      useSelection = false;
    }
  }

  function onkeydown(event: KeyboardEvent) {
    // While an answer is coming, Escape stops it before it would close the panel.
    if (event.key === "Escape" && busy) {
      event.preventDefault();
      session.stop();
      return;
    }
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      void send();
    }
  }
</script>

<form
  class="composer"
  data-off={off || undefined}
  onsubmit={(event) => {
    event.preventDefault();
    void send();
  }}
>
  <textarea
    bind:this={field}
    aria-label={m.assist_placeholder()}
    placeholder={useSelection ? m.assist_placeholder_selection() : m.assist_placeholder()}
    rows="1"
    maxlength={ASK_LIMIT}
    disabled={off}
    value={draft}
    onfocus={() => session.refresh()}
    oninput={(event) => ondraft(event.currentTarget.value)}
    {onkeydown}></textarea>
  <div class="bar">
    {#if selection !== null}
      <button
        type="button"
        class="selection"
        aria-pressed={useSelection}
        title={selection}
        onclick={() => (useSelection = !useSelection)}
      >
        <Icon icon={TextSelectionIcon} size={13} />{m.assist_selection()}
      </button>
    {/if}
    <ModelChip profile={session.profile} />
    <span class="grow"></span>
    {#if busy}
      <button
        type="button"
        class="send stop"
        aria-label={m.assist_stop()}
        title={m.assist_stop()}
        onclick={() => session.stop()}
      >
        <Icon icon={StopIcon} size={12} />
      </button>
    {:else}
      <button
        type="submit"
        class="send"
        aria-label={m.assist_send()}
        title={m.assist_send()}
        disabled={!ready}
      >
        <Icon icon={ArrowUp02Icon} size={15} strokeWidth={2.2} />
      </button>
    {/if}
  </div>
</form>

<style>
  .composer {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 4px 12px 12px;
    padding: 10px 8px 8px 12px;
    border-radius: var(--radius-card);

    /* An opaque plate: the column behind it is glass, and a sentence being typed
       must never sit over whatever the glass shows. */
    background: var(--color-raised);
    box-shadow:
      var(--shadow-field),
      inset 0 0 0 1px var(--color-border-strong);
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      box-shadow var(--motion-instant) var(--ease-smooth);
  }

  .composer:focus-within {
    background: linear-gradient(var(--color-field), var(--color-field)), var(--color-raised);
    box-shadow: var(--shadow-field-focus);
  }

  .composer[data-off] {
    opacity: 0.55;
  }

  textarea {
    inline-size: 100%;
    min-block-size: 22px;
    max-block-size: 132px;
    box-sizing: border-box;
    padding: 0 4px 0 0;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-body);
    line-height: 22px;
    resize: none;
  }

  textarea::placeholder {
    color: var(--color-faint);
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 4px;
    min-inline-size: 0;
    margin-inline-start: -4px;
  }

  .grow {
    flex: 1;
  }

  .selection {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    block-size: 24px;
    padding: 0 9px 0 7px;
    border: 0;
    border-radius: var(--radius-capsule);
    background: var(--color-fill);
    color: var(--color-muted);
    font: inherit;
    font-size: var(--text-label);
    font-weight: 500;
    cursor: default;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      color var(--motion-instant) var(--ease-smooth);
  }

  .selection:hover {
    background: var(--color-fill-hover);
    color: var(--color-text);
  }

  .selection[aria-pressed="true"] {
    background: var(--color-lit);
    color: var(--color-on-lit);
  }

  .selection:focus-visible {
    outline: 2px solid var(--color-ring);
  }

  .send {
    display: grid;
    flex: none;
    place-items: center;
    inline-size: 28px;
    block-size: 28px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-capsule);
    background: var(--color-lit);
    color: var(--color-on-lit);
    cursor: default;
    transition:
      opacity var(--motion-fast) var(--ease-smooth),
      scale var(--motion-fast) var(--ease-smooth),
      background-color var(--motion-instant) var(--ease-smooth);
  }

  .send:disabled {
    background: var(--color-fill-hover);
    color: var(--color-faint);
  }

  .send:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: 2px;
  }

  .send:hover:not(:disabled) {
    background: var(--color-lit-hover);
  }

  .send:active:not(:disabled) {
    scale: 0.92;
  }

  .stop {
    background: var(--color-fill-strong);
    color: var(--color-text);
  }

  @media (forced-colors: active) {
    .send {
      border: 1px solid ButtonText;
    }
  }
</style>
