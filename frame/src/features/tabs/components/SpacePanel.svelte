<script lang="ts">
  import "$shared/ui/Menu/surface.css";
  import * as m from "$shared/i18n/messages";
  import { Cancel01Icon } from "@hugeicons/core-free-icons";
  import IconButton from "$shared/ui/IconButton";
  import * as tools from "$session/tools.svelte";
  import SpaceFlowBody from "./SpaceFlowBody.svelte";
  import { SpaceFlow } from "../lib/space-flow.svelte";

  const flow = new SpaceFlow(() => tools.closeSpaces());
  flow.open = true;

  $effect(() => {
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.isComposing) return;
      event.stopPropagation();
      flow.escape();
    };
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  });
</script>

<section
  class="space-host ui-menu"
  aria-label={flow.editing ? m.space_edit_label({ name: flow.editing.name }) : m.space_switcher()}
>
  {#if flow.editing === null}<header class="space-host-head">
      <h2>{m.space_switcher()}</h2>
      <IconButton
        icon={Cancel01Icon}
        label={m.browse_spaces_close()}
        onclick={() => flow.dismiss()}
      />
    </header>{/if}
  <div class="space-host-body" data-editing={flow.editing !== null || undefined}>
    <SpaceFlowBody {flow} />
  </div>
</section>

<style>
  .space-host {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: none;
    box-shadow: none;
    color: var(--color-text);
  }

  .space-host-head {
    display: flex;
    flex: none;
    align-items: center;
    gap: 8px;
    min-height: 48px;
    padding: 6px 12px 6px 16px;
    user-select: none;
  }

  .space-host-head h2 {
    flex: 1;
    margin: 0;
    font-size: 14px;
    font-weight: 550;
    letter-spacing: -0.015em;
  }

  .space-host-body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 0 var(--menu-pad) var(--menu-pad);
  }

  .space-host-body[data-editing] {
    overflow: hidden;
    padding: 0;
  }
</style>
