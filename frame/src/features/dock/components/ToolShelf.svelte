<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { IncognitoIcon, Settings02Icon, UserIcon } from "@hugeicons/core-free-icons";
  import { commands } from "$shared/ipc/bindings";
  import * as tools from "$session/tools.svelte";
  import { tabs } from "$domain/tabs";
  import Disclosure from "$shared/ui/Disclosure";
  import Icon from "$shared/ui/Icon";
  import { RECORD_TOOLS, SHELF_TOOLS, toolPresentation } from "../lib/dock-tools";
  import NoticePip from "./NoticePip.svelte";
  import { keymap } from "$domain/keymap";
  import { acceleratorKeys } from "$shared/lib/accelerator";
  import { IS_MAC } from "$shared/platform";

  let { compact = false }: { compact?: boolean } = $props();
  const groups = [SHELF_TOOLS, RECORD_TOOLS] as const;
  // Settings closes the stack nearest the avatar; the private window opens it.
  const count = SHELF_TOOLS.length + RECORD_TOOLS.length + 2;
  const name = $derived(tabs.profile()?.name ?? "");
  function pick(kind: (typeof SHELF_TOOLS)[number] | (typeof RECORD_TOOLS)[number]) {
    if (tools.activeTool() === kind) tools.close();
    else tools.open(kind);
  }
  // Each row says its key, so the menu teaches the way past itself.
  function keysFor(command: string) {
    const accelerator = keymap.all().find((entry) => entry.id === command)?.accelerator;
    return accelerator ? acceleratorKeys(accelerator, IS_MAC).join(IS_MAC ? "" : "+") : null;
  }
  const named = (label: string, keys: string | null) => (keys ? `${label}  ${keys}` : label);
  const run = (id: string) => void commands.runCommand(id).catch(() => {});
  // The full native menu stays one gesture away, where a Mac user reaches for
  // everything else a control can do.
  function nativeMenu(event: MouseEvent) {
    event.preventDefault();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    void commands.toolsMenuPopup(rect.left, rect.top);
  }
</script>

{#snippet row(
  icon: typeof UserIcon,
  label: string,
  keys: string | null,
  step: number,
  onclick: () => void,
  current = false,
)}
  <button
    type="button"
    role="menuitem"
    class="ui-menu-item shelf-item"
    style:--step={step}
    title={compact ? named(label, keys) : undefined}
    aria-label={compact ? label : undefined}
    aria-current={current || undefined}
    {onclick}
  >
    <span class="ui-menu-icon"><Icon {icon} size={16} /></span>
    {#if !compact}<span>{label}</span>{#if keys}<span class="shelf-keys">{keys}</span>{/if}{/if}
  </button>
{/snippet}

<!--
  The foot of the column is you: the profile this window browses as, opening
  onto what belongs to it, stacked on demand. A click opens the stack rising
  out of the avatar, nearest first, the way a stack opens from the Dock;
  nothing opens on hover. At rail width a name has nowhere to go, so the
  stack is glyphs, each named by its tooltip.
-->
<div
  class="shelf"
  data-compact={compact}
  data-morph="profile"
  role="presentation"
  oncontextmenu={nativeMenu}
>
  <Disclosure
    label={name ? `${m.dock_tools()} — ${name}` : m.dock_tools()}
    menu
    triggerClass={["case", tools.activeTool() !== null && "case-lit"].filter(Boolean).join(" ")}
    panelClass="shelf-stack"
  >
    {#snippet trigger()}<Icon
        icon={UserIcon}
        size={compact ? 16 : 17}
        strokeWidth={1.7}
      />{#if compact}<NoticePip />{/if}{/snippet}
    {#if !compact && name}
      <div class="shelf-profile" role="presentation" style:--step={count + 1}>
        <span class="avatar" aria-hidden="true"
          ><Icon icon={UserIcon} size={14} strokeWidth={1.8} /></span
        >
        <span class="shelf-profile-name">{name}</span>
      </div>
    {/if}
    {@render row(
      IncognitoIcon,
      m.profile_incognito(),
      keysFor("window.newPrivate"),
      count - 1,
      () => run("window.newPrivate"),
    )}
    {#each groups as group, at (at)}
      <div class="shelf-rule" role="separator"></div>
      {#each group as kind, index (kind)}
        {@const view = toolPresentation(kind)}
        {@render row(
          view.icon,
          view.label(),
          keysFor(`tool.${kind}`),
          count - 2 - (at === 0 ? index : SHELF_TOOLS.length + index),
          () => pick(kind),
          tools.activeTool() === kind,
        )}
      {/each}
    {/each}
    <div class="shelf-rule" role="separator"></div>
    {@render row(Settings02Icon, m.command_browser_settings(), keysFor("browser.settings"), 0, () =>
      run("browser.settings"),
    )}
  </Disclosure>
</div>

<style>
  .shelf {
    position: relative;
    z-index: 20;
    flex: none;
  }

  /* You, at the head of the row: lit the way the current tab is, with light
     caught only along its top edge, and round where every kept site is
     square, so it reads as yours rather than as one more site. */
  .shelf :global(.case) {
    position: relative;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    margin-inline: 2px;
    border: 0;
    border-radius: var(--radius-capsule);
    background: var(--row-active);
    box-shadow: var(--shine-top);
    color: var(--color-label-secondary);
    cursor: default;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out),
      scale var(--motion-slow) var(--ease-spring);
  }

  .shelf :global(.case:hover),
  .shelf :global(.case[data-state="open"]),
  .shelf :global(.case.case-lit) {
    background: var(--row-pressed);
    color: var(--color-text);
  }

  .shelf :global(.case:active) {
    scale: 0.94;
    transition-duration: var(--motion-instant);
  }

  /* The same, small, beside the profile's name. */
  .avatar {
    display: grid;
    flex: none;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-capsule);
    background: var(--row-active);
    box-shadow: var(--shine-top);
    color: var(--color-label-secondary);
    place-items: center;
  }

  /* Whose menu this is, above everything it holds. */
  .shelf-profile {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px var(--menu-item-inset) 6px;
  }

  .shelf-profile-name {
    overflow: hidden;
    color: var(--color-text);
    font-size: var(--text-body);
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The stack rises out of the case rather than dropping from it. */
  .shelf :global(.shelf-stack) {
    top: auto;
    bottom: calc(100% + 8px);
    min-width: 236px;
    transform-origin: bottom left;
  }

  .shelf :global(.shelf-stack:not([data-open="true"])) {
    translate: 0 6px;
  }

  .shelf :global(.shelf-item) {
    min-height: 36px;
    width: 100%;
    border: 0;
    background: transparent;
    font: inherit;
    font-size: var(--text-body);
    text-align: start;
  }

  /* A caption, not a keycap: the key belongs to the row and steps forward
     with it when the row is pointed at. */
  .shelf-keys {
    margin-inline-start: auto;
    padding-inline-start: 24px;
    color: var(--color-faint);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
    transition: color var(--motion-instant) var(--ease-smooth);
  }

  .shelf :global(.shelf-item:hover .shelf-keys),
  .shelf :global(.shelf-item:focus-visible .shelf-keys) {
    color: var(--color-muted);
  }

  .shelf :global(.shelf-item:hover),
  .shelf :global(.shelf-item:focus-visible) {
    outline: none;
    background: var(--row-active);
  }

  .shelf :global(.shelf-item:active) {
    background: var(--row-pressed);
  }

  .shelf :global(.shelf-item[aria-current="true"] .ui-menu-icon) {
    color: var(--color-accent);
  }

  /* Each row settles in behind the one below it, so the stack reads as
     unfolding from the case instead of arriving as a block. */
  .shelf :global(.shelf-stack[data-open="true"] :is(.shelf-item, .shelf-profile)) {
    animation: shelf-item-in var(--motion-slow) var(--ease-out) both;
    animation-delay: calc(var(--step) * 16ms);
  }

  /* stylelint-disable-next-line keyframes-name-pattern -- Svelte's global prefix. */
  @keyframes -global-shelf-item-in {
    from {
      opacity: 0;
      translate: 0 6px;
    }
  }

  .shelf-rule {
    height: 1px;
    margin: 5px var(--menu-item-inset);
    background: var(--color-menu-separator);
  }

  /* At rail width: a square plate like every other rail item, and a stack
     of glyphs just wide enough for them. */
  .shelf[data-compact="true"] :global(.case) {
    width: 36px;
    height: 36px;
    margin-inline: 0;
  }

  /* Centred on the 40px case: four points either side of it. */
  .shelf[data-compact="true"] :global(.shelf-stack) {
    inset-inline-start: -4px;
    min-width: 0;
    width: 48px;
  }

  .shelf[data-compact="true"] :global(.shelf-item) {
    justify-content: center;
    padding-inline: 0;
  }

  .shelf[data-compact="true"] .shelf-rule {
    margin-inline: 8px;
  }

  @media (forced-colors: active) {
    .shelf :global(.case.case-lit) {
      border: 1px solid Highlight;
    }
  }
</style>
