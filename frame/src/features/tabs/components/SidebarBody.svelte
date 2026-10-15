<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { rememberScroll } from "$shared/ui/scroll-memory";
  import { Add01Icon } from "@hugeicons/core-free-icons";
  import { tabs } from "$domain/tabs";
  import Icon from "$shared/ui/Icon";
  import type { SidebarEntry } from "../lib/sidebar-model";
  import TabList from "./TabList.svelte";
  import SpaceBar from "./SpaceBar.svelte";
  import SpaceGlyph from "./SpaceGlyph.svelte";
  import { arrive, spaceSwipe, stepSpace } from "../lib/space-swipe";
  import { untrack } from "svelte";

  let {
    pinned,
    today,
    splitting,
    onSelect,
  }: {
    pinned: SidebarEntry[];
    today: SidebarEntry[];
    splitting: boolean;
    onSelect: (id: string) => void;
  } = $props();

  // Private browsing keeps one space, so it has no switcher.
  const spaced = $derived(tabs.profile()?.kind !== "incognito" && tabs.activeSpace() !== undefined);
  const order = $derived(tabs.spaces().map((space) => space.id));
  const empty = $derived(pinned.length === 0 && today.length === 0);
  let scroller = $state<HTMLElement>();
  const canStep = (step: 1 | -1) => {
    const at = order.indexOf(tabs.activeSpaceId() ?? "");
    return spaced && at !== -1 && at + step >= 0 && at + step < order.length;
  };
  // Whatever moved the window to another space, the list arrives from the
  // side that space stands on.
  let shownSpace: string | null = null;
  $effect(() => {
    const next = tabs.activeSpaceId();
    untrack(() => {
      const from = shownSpace === null ? -1 : order.indexOf(shownSpace);
      const to = next === null ? -1 : order.indexOf(next);
      if (shownSpace !== null && next !== shownSpace && from !== -1 && to !== -1)
        arrive(scroller, to > from ? 1 : -1);
      shownSpace = next;
    });
  });
</script>

<!--
  The only scrolling region in the chrome. Everything above it is pinned so a
  long tab list can never push a scrollbar up alongside the navigation row.

  Pinned tabs are separated by a rule rather than a heading: the rule says
  "these stay" without spending a row on a word, and the rows themselves carry
  the difference by having no close control.
-->
<!-- Opening a tab never scrolls away with the tabs it adds to. -->
{#if spaced}<SpaceBar />{:else}<div class="new-tab-slot" data-cascade style:--cascade={0}>
    <button type="button" class="new-tab" onclick={tabs.open}>
      <Icon icon={Add01Icon} size={16} />
      <span>{m.new_tab()}</span>
    </button>
  </div>{/if}

<div
  bind:this={scroller}
  use:spaceSwipe={{ canStep, step: stepSpace }}
  data-tabs-drop
  data-under-panel
  use:rememberScroll={`${tabs.profile()?.id}/${tabs.activeSpaceId()}/tabs`}
  class="tab-scroller scroll-edges"
  data-glide-scroller
>
  <TabList
    entries={pinned}
    section="pinned"
    label={m.pinned_tabs()}
    cascadeFrom={1}
    {splitting}
    {onSelect}
  />

  {#if pinned.length > 0}
    <div class="tab-divider" aria-hidden="true"></div>
  {/if}

  <TabList
    entries={today}
    section="today"
    label={m.open_tabs()}
    cascadeFrom={pinned.length + 1}
    {splitting}
    {onSelect}
  />

  {#if empty && spaced}
    {@const space = tabs.activeSpace()!}
    <div class="space-empty">
      <SpaceGlyph icon={space.icon} color={space.color} name={space.name} size={28} />
      <p class="space-empty-title">{m.space_empty_title({ name: space.name })}</p>
      <p class="space-empty-body">{m.space_empty_body()}</p>
      <button type="button" class="space-empty-open" onclick={tabs.open}>
        <Icon icon={Add01Icon} size={14} />
        <span>{m.new_tab()}</span>
      </button>
    </div>
  {/if}
</div>

<style>
  .tab-scroller {
    min-height: 0;
    flex: 1;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding-block-end: 4px;
  }

  .tab-divider {
    height: 1px;
    margin: 5px calc(var(--sidebar-inset) + 8px);
    background: var(--color-border);
  }

  .new-tab-slot {
    flex: none;
    padding-inline: var(--sidebar-inset);
    padding-block: 2px calc(var(--sidebar-row-gap) + 2px);
  }

  .new-tab {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: var(--row-sidebar);
    padding-inline: 8px;
    border: 0;
    border-radius: var(--radius-row);
    background: transparent;
    color: var(--color-muted);
    font: inherit;
    font-size: var(--sidebar-row-text);
    font-weight: var(--sidebar-row-weight);
    letter-spacing: -0.005em;
    text-align: start;
    cursor: default;
    outline: none;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out);
  }

  .space-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 28px 16px 12px;
    text-align: center;
  }

  .space-empty-title {
    margin-block-start: 6px;
    color: var(--color-label-secondary);
    font-size: var(--text-body);
    font-weight: 600;
  }

  .space-empty-body {
    max-width: 200px;
    color: var(--color-faint);
    font-size: var(--text-label);
    line-height: 16px;
  }

  .space-empty-open {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-block-start: 8px;
    padding: 6px 12px;
    border: 0;
    border-radius: var(--radius-capsule);
    background: var(--row-active);
    box-shadow: var(--shine-top);
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-label);
    font-weight: 500;
    cursor: default;
    transition: background-color var(--motion-fast) var(--ease-out);
  }

  .space-empty-open:hover,
  .space-empty-open:focus-visible {
    outline: none;
    background: var(--row-pressed);
  }

  .new-tab:hover {
    background: var(--row-hover);
    color: var(--color-label-secondary);
  }
</style>
