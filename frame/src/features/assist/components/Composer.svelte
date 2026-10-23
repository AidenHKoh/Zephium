<script lang="ts">
  import { onMount, tick } from "svelte";
  import * as m from "$shared/i18n/messages";
  import { tabs } from "$domain/tabs";
  import { favicons } from "$domain/favicons";
  import { loadModelPicker, loadVoice } from "$features/work";
  import FavIcon from "$shared/ui/FavIcon";
  import Icon from "$shared/ui/Icon";
  import Menu from "$shared/ui/Menu";
  import { MESSAGE_LIMIT, type AssistSession } from "../lib/assist-session.svelte";
  import { faultView } from "../lib/faults";
  import {
    Add01Icon,
    ArrowUp02Icon,
    Cancel01Icon,
    Globe02Icon,
    Mic01Icon,
    QuoteDownIcon,
    StopIcon,
    Target02Icon,
  } from "../lib/icons";
  import "$shared/ui/Shine/shine.css";

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
  let dictating = $state(false);
  let voice = $state<{ toggle(): void }>();

  const status = $derived(session.status);
  const page = $derived(session.pageAttached ? (status?.page ?? null) : null);
  const tab = $derived(page ? tabs.tabs().find((known) => known.id === page.tab) : undefined);
  const mark = $derived(tab ? favicons.mark(tab.icon, tab.url) : null);
  const selection = $derived(status?.selection ?? null);
  /** Rust said this conversation cannot talk; until it answers, the field is open to typing. */
  const off = $derived(!!status && !session.talks);
  const busy = $derived(session.busy);
  const ready = $derived(!!draft.trim() && session.talks && !busy && !session.sending);
  const picking = $derived(session.picking !== null);
  const notice = $derived(session.notice ? faultView(session.notice).message : null);
  const chips = $derived(!!page || selection !== null || session.picks.length > 0 || picking);

  // Opening the panel, from ⌘E or the dock, lands the caret in the field.
  onMount(() => field?.focus());

  /** Moves the caret into the field; the host calls it when the panel is asked for again. */
  export function focus() {
    field?.focus();
  }

  function grow() {
    if (!field) return;
    field.style.blockSize = "auto";
    field.style.blockSize = `${Math.min(field.scrollHeight, 168)}px`;
  }
  $effect(() => {
    void draft;
    void tick().then(grow);
  });

  async function send() {
    if (!ready) return;
    if (await session.ask(draft)) ondraft("");
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && (busy || picking)) {
      // Escape stops what is happening before it would close the panel.
      event.preventDefault();
      if (picking) session.cancelPick();
      else session.stop();
      return;
    }
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      void send();
    }
  }

  function commit(text: string) {
    const before = draft.trimEnd();
    ondraft(before ? `${before} ${text}` : text);
    dictating = false;
    requestAnimationFrame(() => {
      field?.focus();
      field?.setSelectionRange(field.value.length, field.value.length);
    });
  }

  const pick = () => void session.pick(tabs.activeTab()?.url ?? null);

  const menu = $derived([
    {
      kind: "item" as const,
      id: "page",
      label: m.assist_attach_page(),
      icon: Globe02Icon,
      disabled: !session.attachable || session.pageAttached,
    },
    {
      kind: "item" as const,
      id: "pick",
      label: m.assist_pick(),
      icon: Target02Icon,
      disabled: !status?.can_pick || picking,
    },
  ]);
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key !== "Escape" || !picking || event.defaultPrevented) return;
    event.preventDefault();
    session.cancelPick();
  }}
/>

<form
  class="surface composer ui-shine"
  data-off={off || undefined}
  onsubmit={(event) => {
    event.preventDefault();
    void send();
  }}
>
  {#if chips}
    <ul class="chips" aria-label={m.assist_attached_label()}>
      {#if page}
        <li class="chip" title={page.title}>
          <FavIcon image={mark?.image ?? null} tone={mark?.tone} size={14} fallback={Globe02Icon} />
          <span class="name">{page.host}</span>
          <button
            type="button"
            class="drop"
            aria-label={m.assist_detach({ name: page.host })}
            onclick={() => (session.pageOff = true)}
            ><Icon icon={Cancel01Icon} size={10} strokeWidth={2.4} /></button
          >
        </li>
      {/if}
      {#if selection !== null}
        <li>
          <button
            type="button"
            class="chip toggle"
            aria-pressed={session.selectionOn}
            title={selection}
            onclick={() => (session.selectionOn = !session.selectionOn)}
          >
            <Icon icon={QuoteDownIcon} size={12} strokeWidth={1.8} /><span class="name"
              >{m.assist_selection()}</span
            >
          </button>
        </li>
      {/if}
      {#each session.picks as picked (picked.pick)}
        <li class="chip" title={`${picked.label} · ${picked.host}`}>
          <Icon icon={Target02Icon} size={12} strokeWidth={1.8} />
          <span class="name">{picked.label}</span>
          <button
            type="button"
            class="drop"
            aria-label={m.assist_detach({ name: picked.label })}
            onclick={() => session.detach(picked)}
            ><Icon icon={Cancel01Icon} size={10} strokeWidth={2.4} /></button
          >
        </li>
      {/each}
      {#if picking}<li class="hint" role="status">{m.assist_picking()}</li>{/if}
    </ul>
  {/if}
  {#if dictating}
    <div class="voice">
      {#await loadVoice() then view}<view.default
          bind:this={voice}
          oncommit={commit}
          onclose={() => (dictating = false)}
        />{/await}
    </div>
  {:else}
    <textarea
      bind:this={field}
      aria-label={m.assist_placeholder()}
      placeholder={m.assist_placeholder()}
      rows="1"
      maxlength={MESSAGE_LIMIT}
      disabled={off}
      value={draft}
      onfocus={() => session.refresh()}
      oninput={(event) => {
        session.notice = null;
        ondraft(event.currentTarget.value);
      }}
      {onkeydown}></textarea>
  {/if}
  {#if notice}<p class="notice" role="alert">{notice}</p>{/if}
  <div class="bar">
    <Menu
      label={m.assist_add()}
      entries={menu}
      side="top"
      triggerClass="round ui-shine ui-shine-press"
      onselect={(id) => {
        if (id === "page") session.pageOff = false;
        else if (id === "pick") pick();
      }}
    >
      {#snippet trigger()}<Icon icon={Add01Icon} size={16} strokeWidth={1.8} />{/snippet}
    </Menu>
    <button
      type="button"
      class="round ghost"
      aria-label={picking ? m.assist_pick_cancel() : m.assist_pick()}
      title={picking ? m.assist_pick_cancel() : m.assist_pick()}
      aria-pressed={picking}
      disabled={!status?.can_pick && !picking}
      onclick={() => (picking ? session.cancelPick() : pick())}
      ><Icon icon={Target02Icon} size={16} strokeWidth={1.7} /></button
    >
    <span class="grow"></span>
    <span class="model">
      {#await loadModelPicker() then picker}<picker.default
          profile={session.profile}
          role="page"
        />{/await}
    </span>
    <button
      type="button"
      class="round ghost"
      aria-label={m.voice_start()}
      title={m.voice_start()}
      aria-pressed={dictating}
      disabled={off}
      onclick={() => (dictating ? voice?.toggle() : (dictating = true))}
      ><Icon icon={Mic01Icon} size={16} strokeWidth={1.7} /></button
    >
    {#if busy}
      <button
        type="button"
        class="send stop"
        aria-label={m.assist_stop()}
        title={m.assist_stop()}
        onclick={() => session.stop()}><Icon icon={StopIcon} size={12} /></button
      >
    {:else}
      <button
        type="submit"
        class="send"
        aria-label={m.assist_send()}
        title={m.assist_send()}
        disabled={!ready}><Icon icon={ArrowUp02Icon} size={15} strokeWidth={2.2} /></button
      >
    {/if}
  </div>
</form>

<style>
  .composer {
    --shine-radius: var(--radius-card);

    display: flex;
    flex: none;
    flex-direction: column;
    gap: 6px;
    margin: 4px 10px 10px;
    padding: 10px 8px 8px;
    transition: box-shadow var(--motion-fast) var(--ease-out);
  }

  .composer:focus-within {
    box-shadow: var(--shine-top), var(--shadow-field-focus);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin: 0;
    padding: 0 2px;
    list-style: none;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    box-sizing: border-box;
    max-inline-size: 100%;
    block-size: 26px;
    padding: 0 4px 0 7px;
    border: 0;
    border-radius: var(--radius-capsule);
    background: var(--color-field);
    color: var(--color-label-secondary);
    font: inherit;
    font-size: 12.5px;
    font-weight: 500;
  }

  .name {
    min-inline-size: 0;
    max-inline-size: 168px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .toggle {
    padding-inline-end: 10px;
    color: var(--color-muted);
    cursor: default;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      color var(--motion-instant) var(--ease-smooth);
  }

  .toggle:hover {
    color: var(--color-text);
  }

  .toggle[aria-pressed="true"] {
    background: var(--color-lit);
    color: var(--color-on-lit);
  }

  .drop {
    display: grid;
    flex: none;
    place-items: center;
    inline-size: 18px;
    block-size: 18px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--color-faint);
    cursor: default;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      color var(--motion-instant) var(--ease-smooth);
  }

  .drop:hover {
    background: var(--color-fill-hover);
    color: var(--color-text);
  }

  .toggle:focus-visible,
  .drop:focus-visible {
    outline: 2px solid var(--color-ring);
  }

  .hint {
    display: inline-flex;
    align-items: center;
    block-size: 26px;
    padding-inline: 4px;
    color: var(--color-muted);
    font-size: 12.5px;
  }

  textarea {
    inline-size: 100%;
    min-block-size: 24px;
    max-block-size: 168px;
    box-sizing: border-box;
    padding: 2px 6px;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-page-title);
    line-height: 21px;
    resize: none;
  }

  textarea::placeholder {
    color: var(--color-faint);
  }

  .composer[data-off] textarea {
    opacity: 0.5;
  }

  .voice {
    min-block-size: 32px;
  }

  .notice {
    margin: 0;
    padding: 0 6px;
    color: var(--color-warning);
    font-size: 12.5px;
    line-height: 1.4;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 2px;
    min-inline-size: 0;
  }

  .grow {
    flex: 1;
  }

  .model {
    display: flex;
    min-inline-size: 0;
    margin-inline-end: 2px;
  }

  .model :global(.model-trigger) {
    max-inline-size: 100%;
    block-size: 28px;
    margin: 0;
    padding-inline: 9px 11px;
    gap: 6px;
    line-height: 28px;
  }

  .bar :global(.round) {
    --shine-radius: var(--radius-capsule);

    display: grid;
    flex: none;
    place-items: center;
    inline-size: 28px;
    block-size: 28px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-capsule);
    color: var(--color-muted);
    cursor: default;
  }

  .ghost {
    background: transparent;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      color var(--motion-instant) var(--ease-smooth);
  }

  .ghost:disabled {
    opacity: 0.4;
  }

  .ghost:hover:not(:disabled) {
    background: var(--color-fill-hover);
    color: var(--color-text);
  }

  .ghost[aria-pressed="true"] {
    background: var(--color-lit);
    color: var(--color-on-lit);
  }

  .bar :global(.round:focus-visible) {
    outline: 2px solid var(--color-ring);
    outline-offset: 2px;
  }

  .send {
    display: grid;
    flex: none;
    place-items: center;
    inline-size: 30px;
    block-size: 30px;
    margin-inline-start: 2px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--color-lit);
    color: var(--color-on-lit);
    cursor: default;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      scale var(--motion-fast) var(--ease-smooth);
  }

  .send:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: 2px;
  }

  .send:disabled {
    background: var(--color-fill-strong);
    color: var(--color-faint);
  }

  .send:hover:not(:disabled) {
    background: var(--color-lit-hover);
  }

  .send:active:not(:disabled) {
    scale: 0.92;
  }

  @media (forced-colors: active) {
    .send,
    .chip {
      border: 1px solid ButtonText;
    }
  }
</style>
