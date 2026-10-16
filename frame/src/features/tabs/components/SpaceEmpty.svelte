<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { Add01Icon } from "@hugeicons/core-free-icons";
  import type { SpaceView } from "$shared/ipc/bindings";
  import Icon from "$shared/ui/Icon";
  import SpaceGlyph from "./SpaceGlyph.svelte";

  let { space, onopen }: { space: SpaceView; onopen?: () => void } = $props();
</script>

<div class="space-empty">
  <SpaceGlyph icon={space.icon} color={space.color} name={space.name} size={28} />
  <p class="space-empty-title">{m.space_empty_title({ name: space.name })}</p>
  <p class="space-empty-body">{m.space_empty_body()}</p>
  {#if onopen}<button type="button" class="space-empty-open" onclick={onopen}>
      <Icon icon={Add01Icon} size={14} />
      <span>{m.new_tab()}</span>
    </button>{/if}
</div>

<style>
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
</style>
