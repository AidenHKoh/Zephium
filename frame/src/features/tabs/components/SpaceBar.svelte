<script lang="ts">
  import "$shared/ui/Menu/surface.css";
  import * as m from "$shared/i18n/messages";
  import { Add01Icon, ArrowDown01Icon } from "@hugeicons/core-free-icons";
  import { tabs } from "$domain/tabs";
  import { uiCommands as ui } from "$domain/ui-commands";
  import Icon from "$shared/ui/Icon";
  import { untrack } from "svelte";
  // The panel's contents load the first time it opens.
  const loadEditor = () => import("./SpaceEditor.svelte");
  const loadSwitcher = () => import("./SpaceSwitcher.svelte");
  import SpaceGlyph from "./SpaceGlyph.svelte";
  import * as requests from "../lib/space-requests.svelte";
  import { freshLook } from "../lib/space-look";

  const spaces = $derived(tabs.spaces());
  const active = $derived(tabs.activeSpace());
  let open = $state(false);
  /** The space being edited; null lists them. */
  let editingId = $state<string | null>(null);
  const editing = $derived(spaces.find((space) => space.id === editingId) ?? null);
  /** Made a moment ago, so its name waits to be typed over. */
  let fresh = $state(false);
  /** The spaces there were before one was asked for, until it appears. */
  let making = $state.raw<Set<string> | null>(null);
  let makeFailed = $state(false);
  let release: (() => void) | null = null;
  /** The editor was opened from the list, so going back returns to it. */
  let fromList = false;
  let root = $state<HTMLElement>();
  let chip = $state<HTMLButtonElement>();

  function show(id: string | null = null) {
    editingId = id;
    fresh = false;
    fromList = false;
    open = true;
  }

  function edit(id: string) {
    editingId = id;
    fresh = false;
    fromList = true;
  }

  function dismiss(restore = false) {
    if (!open) return;
    open = false;
    editingId = null;
    making = null;
    makeFailed = false;
    release?.();
    release = null;
    if (restore) chip?.focus();
  }

  // A new space is made at once, named in order, and its editor opens on it
  // with the name ready to be typed over.
  async function make() {
    if (making) return;
    open = true;
    makeFailed = false;
    making = new Set(spaces.map((space) => space.id));
    const result = await tabs.createSpace("", freshLook(spaces));
    if (result.outcome === "failed" || result.outcome === "rejected") {
      making = null;
      makeFailed = true;
    }
  }

  $effect(() => {
    const before = making;
    if (!before) return;
    const made = spaces.find((space) => !before.has(space.id));
    if (!made) return;
    untrack(() => {
      making = null;
      editingId = made.id;
      fresh = true;
      fromList = false;
    });
  });

  // A space deleted from under its editor leaves the panel.
  $effect(() => {
    if (editingId !== null && editing === null && making === null) untrack(() => dismiss());
  });

  // A menu asked for this before the column had room for it.
  $effect(() => {
    const request = requests.pending();
    if (!request || !active) return;
    untrack(() => {
      requests.take();
      release = request.release;
      if (request.kind === "new") {
        show();
        void make();
      } else show(active.id);
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
    // WebKit does not focus a pressed button, so Escape is heard window-wide.
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.isComposing) return;
      event.stopPropagation();
      if (editingId !== null && fromList) editingId = null;
      else dismiss(true);
    };
    window.addEventListener("pointerdown", outside, true);
    window.addEventListener("blur", away);
    window.addEventListener("keydown", escape);
    return () => {
      window.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("blur", away);
      window.removeEventListener("keydown", escape);
    };
  });

  function choose(id: string) {
    dismiss(true);
    if (id !== active?.id) void tabs.switchSpace(id);
  }
</script>

<!--
  The head of the tab list names the space it holds: its name, with its mark
  when it has one, opens the switcher; the points beside it are every space
  at a glance (and where a swipe will land), and the plus opens a tab in it.
-->
<div
  class="space-bar"
  data-cascade
  data-under-panel
  style:--cascade={0}
  bind:this={root}
  role="presentation"
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
          data-space-hue={space.color}
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
    class="ui-menu ui-disclosure-panel ui-popover space-panel sidebar-softens"
    role="dialog"
    aria-label={editing ? m.space_edit_label({ name: editing.name }) : m.space_switcher()}
    data-open={open}
    data-editing={editing !== null || undefined}
    inert={!open}
  >
    {#if open}<div class="space-panel-body">
        {#if editing !== null}
          {#key editing.id}{#await loadEditor() then { default: SpaceEditor }}<SpaceEditor
                space={editing}
                {fresh}
                deletable={spaces.length > 1}
                onclose={(done) => {
                  if (done || !fromList) dismiss(true);
                  else editingId = null;
                }}
              />{/await}{/key}
        {:else}
          {#await loadSwitcher() then { default: SpaceSwitcher }}<SpaceSwitcher
              {spaces}
              active={active?.id ?? null}
              making={making !== null}
              failed={makeFailed}
              onchoose={choose}
              onedit={(space) => edit(space.id)}
              onnew={make}
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
    gap: 8px;
    min-width: 0;
    height: 30px;
    padding-inline: 10px 7px;
    border: 0;
    border-radius: var(--radius-row);
    background: transparent;
    color: var(--color-label-secondary);
    font: inherit;
    font-size: var(--sidebar-row-text);
    font-weight: 600;
    letter-spacing: -0.01em;
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
    margin-inline-start: -3px;
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
    display: grid;
    width: 12px;
    height: 24px;
    padding: 0;
    border: 0;
    background: transparent;
    place-items: center;
    transition: width var(--motion-base) var(--ease-spring);
  }

  .space-dot i {
    width: 14px;
    height: 5px;
    border-radius: var(--radius-capsule);
    background: var(--color-faint);
    opacity: 0.55;
    scale: 0.36 1;
    transition:
      scale var(--motion-base) var(--ease-spring),
      background-color var(--motion-fast) var(--ease-out),
      opacity var(--motion-fast) var(--ease-out);
  }

  .space-dot:hover i {
    background: var(--space-hue);
    opacity: 1;
  }

  .space-dot[aria-current="true"] {
    width: 20px;
  }

  .space-dot[aria-current="true"] i {
    scale: 1;
    background: var(--space-hue);
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
    width: 30px;
    height: 30px;
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
    margin-inline-start: 4px;
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

  /* The list scrolls inside the still material when it runs long; the
     editor keeps its own actions in view and scrolls only its body. */
  .space-panel-body {
    display: flex;
    flex-direction: column;
    max-height: min(560px, calc(100vh - 200px));
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: var(--menu-pad);
  }

  .space-panel[data-editing] .space-panel-body {
    overflow: hidden;
    padding: 0;
  }

  @media (prefers-reduced-motion: reduce) {
    .space-chevron,
    .space-dot,
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
