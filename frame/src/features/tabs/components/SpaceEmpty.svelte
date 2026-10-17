<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { Add01Icon } from "@hugeicons/core-free-icons";
  import type { SpaceView } from "$shared/ipc/bindings";
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";
  import SpaceGlyph from "./SpaceGlyph.svelte";

  let { space, onopen }: { space: SpaceView; onopen?: () => void } = $props();
</script>

<div class="space-empty">
  <SpaceGlyph icon={space.icon} color={space.color} size={28} />
  <p class="space-empty-title">{m.space_empty_title({ name: space.name })}</p>
  <p class="space-empty-body">{m.space_empty_body()}</p>
  {#if onopen}<Button class="space-empty-open" onclick={onopen}
      ><Icon icon={Add01Icon} size={14} />{m.new_tab()}</Button
    >{/if}
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

  .space-empty :global(.space-empty-open) {
    margin-block-start: 8px;
  }
</style>
