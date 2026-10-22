<script lang="ts">
  import { onMount, type Component, type Snippet } from "svelte";
  import type { ToolHostProps } from "$session/tool-drafts.svelte";
  import { tabs } from "$domain/tabs";
  import { events } from "$shared/ipc/native-events";
  import { AssistSession } from "../lib/assist-session.svelte";
  import AssistComposer from "./AssistComposer.svelte";
  import AssistThread from "./AssistThread.svelte";
  import IconButton from "$shared/ui/IconButton";
  import { PencilEdit02Icon } from "@hugeicons/core-free-icons";
  import * as m from "$shared/i18n/messages";
  /** The tool frame is the host's: the panel fills it with the thread and the composer. */
  type Frame = Component<
    ToolHostProps & {
      caption?: boolean;
      scrolls?: boolean;
      actions?: Snippet;
      footer?: Snippet;
      children?: Snippet;
    }
  >;
  let { Frame, ...viewProps }: ToolHostProps & { Frame: Frame } = $props();

  let session = $state.raw<AssistSession | null>(null);
  $effect(() => {
    const owner = new AssistSession(viewProps.profile);
    session = owner;
    void owner.start();
    return () => owner.dispose();
  });
  // The panel is about the tab the person is on; the focused pane of a split is the active tab.
  $effect(() => session?.follow(tabs.activeId() ?? null));
  // A navigation changes what the page is, not whose thread this is.
  const tab = $derived(tabs.activeTab());
  const address = $derived(tab?.url ?? null);
  $effect(() => {
    void address;
    session?.refresh();
  });

  let composer = $state<ReturnType<typeof AssistComposer>>();
  onMount(() => {
    // The field takes focus as it mounts; this is for asking again while open.
    let stop: (() => void) | null = null;
    let live = true;
    // Asking for Ask again, while it is open, goes to its field.
    void events.uiCommand
      .listen(({ payload }) => {
        if (payload === "tool.ai") composer?.focus();
      })
      .then((off) => {
        if (live) stop = off;
        else off();
      });
    return () => {
      live = false;
      stop?.();
    };
  });
</script>

<!-- Escape closes the panel; a menu inside it takes the key first. -->
<div
  class="assist-host"
  role="presentation"
  onkeydown={(event) => {
    if (event.key === "Escape" && !event.defaultPrevented) viewProps.onclose();
  }}
>
  <Frame {...viewProps} caption={false} scrolls={false}>
    {#snippet actions()}
      {#if session && session.thread.items.length > 0}<IconButton
          icon={PencilEdit02Icon}
          label={m.assist_new()}
          onclick={() => void session?.clear()}
        />{/if}
    {/snippet}
    {#if session}<AssistThread
        {session}
        page={{ title: tab?.title ?? "", url: tab?.url ?? null }}
      />{/if}
    {#snippet footer()}
      {#if session}<AssistComposer
          {session}
          draft={viewProps.state.draft}
          ondraft={(draft) => viewProps.edit({ draft })}
          bind:this={composer}
        />{/if}
    {/snippet}
  </Frame>
</div>

<style>
  .assist-host {
    display: contents;
  }
</style>
