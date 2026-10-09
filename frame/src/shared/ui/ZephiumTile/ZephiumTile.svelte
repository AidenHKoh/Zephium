<script lang="ts">
  import Icon, { ZephiumMark } from "$shared/ui/Icon";

  /** Zephium's app icon: the mark on its dark tile, the same in either theme. */
  let { size = 16 }: { size?: number } = $props();
</script>

<span class="zephium-tile" style:--size={`${size}px`} aria-hidden="true"
  ><Icon icon={ZephiumMark} size={Math.round(size * 0.8)} /></span
>

<style>
  .zephium-tile {
    display: inline-grid;
    flex: none;
    inline-size: var(--size);
    block-size: var(--size);
    border-radius: calc(var(--size) * 0.23);
    background: var(--brand-tile);
    box-shadow: var(--shadow-brand-tile);
    color: var(--color-brand-tile-ink);
    place-items: center;
  }
</style>
