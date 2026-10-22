<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import type { AssistSession } from "../lib/assist-session.svelte";
  import AssistEmpty from "./AssistEmpty.svelte";
  import AssistTurn from "./AssistTurn.svelte";
  import Refusal from "./Refusal.svelte";
  import "./assist.css";

  let { session, page }: { session: AssistSession; page?: { title: string; url: string | null } } =
    $props();

  let scroller = $state<HTMLElement>();
  /** Follow the answer while the reader is at its end; leave them alone once they scroll up. */
  let pinned = true;
  const items = $derived(session.thread.items);
  const refusal = $derived(session.refusal);

  function onscroll() {
    if (!scroller) return;
    pinned = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 48;
  }

  $effect(() => {
    void session.thread;
    const element = scroller;
    if (!element || !pinned) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    queueMicrotask(() =>
      element.scrollTo({ top: element.scrollHeight, behavior: reduce ? "auto" : "smooth" }),
    );
  });
</script>

<div class="thread" bind:this={scroller} {onscroll} data-tauri-drag-region="false">
  {#if session.unavailable}
    <section class="down" role="alert">
      <p>{m.assist_unavailable()}</p>
      <Button size="compact" onclick={() => session.refresh()}>{m.assist_retry()}</Button>
    </section>
  {:else if refusal}
    <Refusal {refusal} />
  {:else if session.loading}
    <span class="sr-only" role="status">{m.panel_loading()}</span>
  {:else if items.length === 0 && session.status}
    <AssistEmpty
      site={session.site}
      title={page?.title ?? ""}
      url={page?.url ?? null}
      suggestions={session.status.suggestions}
      onpick={(suggestion) => void session.suggest(suggestion)}
    />
  {:else}
    <div class="items" role="log" aria-label={m.assist_thread_label()}>
      {#each items as item (item.type === "turn" ? `t${item.turn}` : `d${item.key}`)}
        {#if item.type === "turn"}
          <AssistTurn turn={item} {session} />
        {:else}
          <div
            class="divider assist-rise"
            role="separator"
            aria-label={item.fresh
              ? m.assist_divider_fresh({ site: item.site })
              : m.assist_divider_moved({ site: item.site })}
          >
            <span
              >{item.fresh
                ? m.assist_divider_fresh({ site: item.site })
                : m.assist_divider_moved({ site: item.site })}</span
            >
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .thread {
    flex: 1;
    min-block-size: 0;
    overflow: auto;
    padding: 4px 16px 16px;
    scrollbar-gutter: stable;
  }

  .items {
    display: flex;
    flex-direction: column;
    gap: 26px;
    padding-block-start: 8px;
  }

  .divider {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--color-faint);
    font-size: var(--text-caption);
    line-height: 16px;
  }

  .divider::before,
  .divider::after {
    flex: 1;
    block-size: 1px;
    background: var(--color-border);
    content: "";
  }

  .divider span {
    max-inline-size: 70%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .down {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
    padding-block-start: 28px;
  }

  .down p {
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-body);
    line-height: 1.5;
  }
</style>
