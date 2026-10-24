<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import type { Snippet } from "svelte";
  import { Settings05Icon } from "@hugeicons/core-free-icons";
  import Icon from "$shared/ui/Icon";
  import Disclosure from "$shared/ui/Disclosure";

  let { children, onopen }: { children: Snippet; onopen?: () => void } = $props();
</script>

<!--
  Everything that acts on the page without being the page: extensions, the
  protection standing, whatever else earns a place later. One glyph holds
  them, because none of them is worth a permanent seat in the chrome.
-->
<Disclosure
  {onopen}
  label={m.utilities()}
  menu
  triggerClass="utilities-trigger"
  panelClass="utilities-panel sidebar-softens"
  align="end"
>
  {#snippet trigger()}<Icon icon={Settings05Icon} size={15} />{/snippet}
  {@render children()}
</Disclosure>

<style>
  /*
    Present but unpainted until the column is in use. It acts on the page the
    field names, so it has no business drawing the eye the rest of the time;
    the pointer resting in the column brings it in after a beat, and focusing
    the field or opening its own panel brings it at once.
  */
  :global(.browser-sidebar[data-pointer-in="true"]) :global(.utilities-trigger) {
    opacity: 1;
    transition-delay: 160ms, 0ms, 0ms;
  }

  :global(.utilities-trigger[data-state="open"]),
  :global(.browser-sidebar[data-pointer-in="true"] .address-field:hover)
    :global(.utilities-trigger),
  :global(.address-field:focus-within) :global(.utilities-trigger) {
    opacity: 1;
    transition-delay: 0ms;
  }

  :global(.utilities-trigger) {
    opacity: 0;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: var(--radius-inset);
    background: transparent;
    color: var(--color-faint);
    cursor: default;
    outline: none;
    transition:
      opacity var(--motion-fast) var(--ease-out),
      background-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out);
  }

  :global(.utilities-panel) {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 212px;
  }

  :global(.utilities-trigger:hover),
  :global(.utilities-trigger[data-state="open"]) {
    background: var(--row-hover);
    color: var(--color-text);
  }
</style>
