<script lang="ts">
  import "$shared/ui/Menu/surface.css";
  import * as m from "$shared/i18n/messages";
  import { Add01Icon, Layers01Icon } from "@hugeicons/core-free-icons";
  import { tabs } from "$domain/tabs";
  import Icon from "$shared/ui/Icon";
  import { openInColumn } from "../lib/space-requests.svelte";
  import SpaceGlyph from "./SpaceGlyph.svelte";

  let { step = 0 }: { step?: number } = $props();
</script>

<!-- The spaces where the rail has no room for their own control: a glyph each
     to switch by, then the rest of the flow, which opens the column in place. -->
<div class="shelf-spaces" role="presentation" style:--step={step}>
  {#each tabs.spaces() as space (space.id)}
    {@const current = space.id === tabs.activeSpaceId()}
    <button
      type="button"
      role="menuitem"
      class="ui-menu-item shelf-space"
      title={space.name}
      aria-label={space.name}
      aria-current={current || undefined}
      onclick={() => !current && void tabs.switchSpace(space.id)}
      ><SpaceGlyph
        icon={space.icon}
        color={space.color}
        name={space.name}
        size={18}
        fallback="monogram"
      /></button
    >
  {/each}
  <button
    type="button"
    role="menuitem"
    class="ui-menu-item shelf-space"
    title={m.space_new()}
    aria-label={m.space_new()}
    onclick={() => openInColumn("new")}><Icon icon={Add01Icon} size={16} /></button
  >
  <button
    type="button"
    role="menuitem"
    class="ui-menu-item shelf-space"
    title={m.browse_spaces_all()}
    aria-label={m.browse_spaces_all()}
    onclick={() => openInColumn("list")}><Icon icon={Layers01Icon} size={16} /></button
  >
</div>

<style>
  .shelf-spaces {
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: spaces-in var(--motion-slow) var(--ease-out) both;
    animation-delay: calc(var(--step) * 14ms);
  }

  /* A glyph each: the current one lit like the current tab. */
  .shelf-space {
    justify-content: center;
    width: 100%;
    min-height: 28px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--color-muted);
  }

  .shelf-space:is(:hover, :focus-visible) {
    background: var(--row-active);
    color: var(--color-text);
  }

  .shelf-space[aria-current="true"] {
    background: var(--row-active);
    box-shadow: var(--row-rim);
  }

  @keyframes spaces-in {
    from {
      opacity: 0;
      translate: 0 6px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .shelf-spaces {
      animation: none;
    }
  }
</style>
