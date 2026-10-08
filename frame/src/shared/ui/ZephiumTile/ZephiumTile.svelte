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
    background: linear-gradient(180deg, #29292b 0%, #3a3a3c 62%, #4a4a4c 100%);
    box-shadow: inset 0 0 0 0.5px rgb(255 255 255 / 0.14);
    color: #fff;
    place-items: center;
  }
</style>
