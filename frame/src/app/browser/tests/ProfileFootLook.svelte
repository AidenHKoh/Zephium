<script lang="ts">
  import "$styles/global.css";
  import { Dock } from "$features/dock";

  /** The column's width, which the menu may not outgrow. */
  let { width = 260 }: { width?: number } = $props();
</script>

<!-- The foot of the sidebar at full width and at rail width, with the room
     above it the menu rises into. -->
<div class="stage">
  <div class="browser-sidebar column" style:width="{width}px" style:--sidebar-width="{width}px">
    <div class="sidebar-columns">
      <div class="sidebar-browser-column">
        <div class="rest"></div>
        <Dock />
      </div>
    </div>
  </div>
  <div class="browser-sidebar column column-rail">
    <div class="sidebar-columns">
      <div class="sidebar-browser-column">
        <div class="rest"></div>
        <Dock compact />
      </div>
    </div>
  </div>
</div>

<style>
  .stage {
    display: flex;
    gap: 24px;
    padding: 16px;
    background: var(--color-canvas);
  }

  .column {
    display: flex;
    flex: none;
    flex-direction: column;
    height: 680px;
    background: var(--color-chrome);
  }

  .column-rail {
    align-items: center;
    width: 56px;
  }

  .rest {
    flex: 1;
  }

  .column-rail :global(.sidebar-browser-column) {
    align-items: center;
  }
</style>
