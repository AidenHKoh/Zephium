<script lang="ts">
  import "$shared/ui/Menu/surface.css";
  import * as m from "$shared/i18n/messages";
  import { Add01Icon, ArrowDown01Icon } from "@hugeicons/core-free-icons";
  import type { SpaceView } from "$shared/ipc/bindings";
  import { tabs } from "$domain/tabs";
  import { uiCommands as ui } from "$domain/ui-commands";
  import Icon from "$shared/ui/Icon";
  import { untrack } from "svelte";
  // The panel's contents load the first time it opens.
  const loadEditor = () => import("./SpaceEditor.svelte");
  const loadSwitcher = () => import("./SpaceSwitcher.svelte");
  import SpaceGlyph from "./SpaceGlyph.svelte";
  import * as requests from "../lib/space-requests.svelte";

  const spaces = $derived(tabs.spaces());
  const active = $derived(tabs.activeSpace());
  let open = $state(false);
  /** Null lists the spaces; a space or "new" edits one. */
  let editing = $state<SpaceView | "new" | null>(null);
  let release: (() => void) | null = null;
  /** The editor was opened from the list, so going back returns to it. */
  let fromList = false;
  let root = $state<HTMLElement>();
  let chip = $state<HTMLButtonElement>();

  function show(next: SpaceView | "new" | null = null) {
    editing = next;
    fromList = false;
    open = true;
  }

  function edit(next: SpaceView | "new") {
    editing = next;
    fromList = true;
  }

  function dismiss(restore = false) {
    if (!open) return;
    open = false;
    editing = null;
    release?.();
    release = null;
    if (restore) chip?.focus();
  }

  // A menu asked for this before the column had room for it.
  $effect(() => {
    const request = requests.pending();
    if (!request || !active) return;
    untrack(() => {
      requests.take();
      release = request.release;
      show(request.kind === "new" ? "new" : active);
    });
  });

  $effect(() => {
    const command = ui.uiCommand();
    if (command.id !== "space.new" && command.id !== "space.edit") return;
    untrack(() => {
      if (requests.claim(command.seq)) requests.ask(command.id === "space.new" ? "new" : "edit");
    });
  });

  // Closing on the press, as every panel in the column does.
  $effect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!(event.target instanceof Node) || !root?.contains(event.target)) dismiss();
    };
    const away = () => dismiss();
    window.addEventListener("pointerdown", outside, true);
    window.addEventListener("blur", away);
    return () => {
      window.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("blur", away);
    };
  });

  function choose(id: string) {
    dismiss(true);
    if (id !== active?.id) void tabs.switchSpace(id);
  }
</script>

<!--
  The head of the tab list names the space it holds: its mark and name open
  the switcher, the dots beside them are every space at a glance (and where a
  swipe will land), and the plus opens a tab in it.
-->
<div
  class="space-bar"
  data-cascade
  style:--cascade={0}
  bind:this={root}
  role="presentation"
  onkeydown={(event) => {
    if (event.key === "Escape" && open) {
      event.stopPropagation();
      if (editing !== null) editing = null;
      else dismiss(true);
    }
  }}
>
  <button
    bind:this={chip}
    type="button"
    class="space-chip"
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-label={m.space_open_switcher({ name: active?.name ?? "" })}
    data-state={open ? "open" : "closed"}
    onclick={() => (open ? dismiss() : show())}
  >
    {#if active}<SpaceGlyph icon={active.icon} color={active.color} name={active.name} />{/if}
    <span class="space-name">{active?.name ?? ""}</span>
    <span class="space-chevron"><Icon icon={ArrowDown01Icon} size={12} strokeWidth={2} /></span>
  </button>
  {#if spaces.length > 1 && spaces.length <= 8}
    <div class="space-dots" role="group" aria-label={m.space_switcher()}>
      {#each spaces as space (space.id)}
        <button
          type="button"
          class="space-dot"
          data-color={space.color}
          aria-current={space.id === active?.id ? "true" : undefined}
          title={space.name}
          aria-label={space.name}
          onclick={() => choose(space.id)}><i></i></button
        >
      {/each}
    </div>
  {/if}
  <button
    type="button"
    class="space-new-tab"
    title={m.new_tab()}
    aria-label={m.new_tab()}
    onclick={tabs.open}><Icon icon={Add01Icon} size={16} /></button
  >
  <div
    class="ui-menu ui-disclosure-panel ui-popover space-panel"
    role="dialog"
    aria-label={m.space_switcher()}
    data-open={open}
    data-editing={editing !== null || undefined}
    inert={!open}
  >
    {#if open}<div class="space-panel-scroll">
        {#if editing !== null}
          {#key editing}{#await loadEditor() then { default: SpaceEditor }}<SpaceEditor
                space={editing === "new" ? null : editing}
                {spaces}
                onclose={(done) => {
                  if (done || !fromList) dismiss(true);
                  else editing = null;
                }}
              />{/await}{/key}
        {:else}
          {#await loadSwitcher() then { default: SpaceSwitcher }}<SpaceSwitcher
              {spaces}
              active={active?.id ?? null}
              onchoose={choose}
              onedit={edit}
              onnew={() => edit("new")}
            />{/await}
        {/if}
      </div>{/if}
  </div>
</div>

<style>
  .space-bar {
    position: relative;
    z-index: 30;
    display: flex;
    flex: none;
    align-items: center;
    gap: 4px;
    padding-inline: var(--sidebar-inset);
    padding-block: 2px calc(var(--sidebar-row-gap) + 2px);
  }

  .space-chip {
    display: flex;
    flex: 0 1 auto;
    align-items: center;
    gap: 9px;
    min-width: 0;
    height: var(--row-sidebar);
    padding-inline: 8px 6px;
    border: 0;
    border-radius: var(--radius-row);
    background: transparent;
    color: var(--color-label-secondary);
    font: inherit;
    font-size: var(--sidebar-row-text);
    font-weight: 600;
    letter-spacing: -0.005em;
    cursor: default;
    outline: none;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out);
  }

  .space-chip:hover,
  .space-chip:focus-visible,
  .space-chip[data-state="open"] {
    background: var(--row-hover);
    color: var(--color-text);
  }

  .space-chip:focus-visible {
    box-shadow: var(--row-rim);
  }

  .space-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .space-chevron {
    display: grid;
    flex: none;
    color: var(--color-faint);
    place-items: center;
    transition: rotate var(--motion-base) var(--ease-out);
  }

  .space-chip[data-state="open"] .space-chevron {
    rotate: 180deg;
  }

  /* Every space as a point of its own hue; the one shown is drawn out long. */
  .space-dots {
    display: flex;
    flex: none;
    align-items: center;
    margin-inline-start: auto;
  }

  .space-dot {
    --dot: var(--avatar-0-top);

    display: grid;
    width: 14px;
    height: 24px;
    padding: 0;
    border: 0;
    background: transparent;
    place-items: center;
  }

  .space-dot i {
    width: 14px;
    height: 5px;
    border-radius: var(--radius-capsule);
    background: var(--color-faint);
    opacity: 0.6;
    scale: 0.36 1;
    transition:
      scale var(--motion-base) var(--ease-spring),
      background-color var(--motion-fast) var(--ease-out),
      opacity var(--motion-fast) var(--ease-out);
  }

  .space-dot:hover i {
    background: var(--dot);
    opacity: 1;
  }

  .space-dot[aria-current="true"] {
    width: 22px;
  }

  .space-dot[aria-current="true"] i {
    scale: 1;
    background: var(--dot);
    opacity: 1;
  }

  .space-dot:focus-visible {
    outline: none;
  }

  .space-dot:focus-visible i {
    outline: 2px solid var(--color-text);
    outline-offset: 2px;
  }

  .space-new-tab {
    display: grid;
    flex: none;
    width: 32px;
    height: 32px;
    margin-inline-start: auto;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--color-muted);
    cursor: default;
    place-items: center;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out),
      scale var(--motion-slow) var(--ease-spring);
  }

  .space-dots + .space-new-tab {
    margin-inline-start: 2px;
  }

  .space-new-tab:hover,
  .space-new-tab:focus-visible {
    outline: none;
    background: var(--row-hover);
    color: var(--color-text);
  }

  .space-new-tab:active {
    scale: 0.92;
  }

  .space-panel {
    inset-inline: var(--sidebar-inset);
    min-width: 0;
    padding: 0;
  }

  /* The material stays still while a long editor scrolls inside it. */
  .space-panel-scroll {
    max-height: min(600px, calc(100vh - 220px));
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: var(--menu-pad);
  }

  .space-panel[data-editing] .space-panel-scroll {
    padding: 12px;
  }

  .space-dot[data-color="1"] {
    --dot: var(--avatar-1-top);
  }

  .space-dot[data-color="2"] {
    --dot: var(--avatar-2-top);
  }

  .space-dot[data-color="3"] {
    --dot: var(--avatar-3-top);
  }

  .space-dot[data-color="4"] {
    --dot: var(--avatar-4-top);
  }

  .space-dot[data-color="5"] {
    --dot: var(--avatar-5-top);
  }

  .space-dot[data-color="6"] {
    --dot: var(--avatar-6-top);
  }

  .space-dot[data-color="7"] {
    --dot: var(--avatar-7-top);
  }

  .space-dot[data-color="8"] {
    --dot: var(--avatar-8-top);
  }

  .space-dot[data-color="9"] {
    --dot: var(--avatar-9-top);
  }

  @media (prefers-reduced-motion: reduce) {
    .space-chevron,
    .space-dot i {
      transition: none;
    }
  }

  @media (forced-colors: active) {
    .space-dot[aria-current="true"] i {
      background: Highlight;
    }
  }
</style>
