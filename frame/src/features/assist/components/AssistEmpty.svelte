<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { favicons } from "$domain/favicons";
  import FavIcon from "$shared/ui/FavIcon";
  import Globe02Icon from "@hugeicons/core-free-icons/Globe02Icon";
  import type { PageAssistSuggestionV1 } from "$shared/ipc/bindings";
  import { suggestionLabel } from "../lib/suggestions";
  import "./assist.css";

  let {
    site,
    title,
    url,
    suggestions,
    onpick,
  }: {
    site: string | null;
    title: string;
    url: string | null;
    suggestions: readonly PageAssistSuggestionV1[];
    onpick: (suggestion: PageAssistSuggestionV1) => void;
  } = $props();

  const mark = $derived(url ? favicons.forPage(url) : null);
  const restyle = (suggestion: PageAssistSuggestionV1) =>
    suggestion === "darker" || suggestion === "readable";
  const groups = $derived(
    [
      { label: m.assist_group_understand(), items: suggestions.filter((s) => !restyle(s)) },
      { label: m.assist_group_restyle(), items: suggestions.filter(restyle) },
    ].filter((group) => group.items.length > 0),
  );
</script>

<div class="empty">
  <div class="page assist-rise">
    <span class="mark"
      ><FavIcon
        image={mark?.image ?? null}
        tone={mark?.tone}
        size={24}
        fallback={Globe02Icon}
      /></span
    >
    <span class="words">
      <strong>{title || site || m.assist_empty()}</strong>
      {#if site && title}<span>{site}</span>{/if}
    </span>
  </div>
  {#each groups as group, at (group.label)}
    <section class="group assist-rise" style:animation-delay="{70 + at * 50}ms">
      <h3>{group.label}</h3>
      <ul>
        {#each group.items as suggestion (suggestion)}
          <li>
            <button type="button" onclick={() => onpick(suggestion)}
              >{suggestionLabel(suggestion)}</button
            >
          </li>
        {/each}
      </ul>
    </section>
  {/each}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    gap: 22px;
    padding-block-start: 14px;
  }

  .page {
    display: flex;
    align-items: center;
    gap: 12px;
    box-sizing: border-box;
    padding: 12px;
    border-radius: var(--radius-row);
    background: var(--color-fill);
    box-shadow: inset 0 0 0 1px var(--color-border);
  }

  .mark {
    display: grid;
    flex: none;
    place-items: center;
    inline-size: 28px;
    block-size: 28px;
    border-radius: var(--radius-inset);
    background: var(--color-field);
  }

  .words {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-inline-size: 0;
  }

  strong {
    display: -webkit-box;
    overflow: hidden;
    color: var(--color-text);
    font-size: var(--text-body);
    font-weight: 600;
    line-height: 18px;
    overflow-wrap: anywhere;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .words > span {
    overflow: hidden;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 16px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  h3 {
    margin: 0;
    padding-inline: 2px;
    color: var(--color-muted);
    font-size: var(--text-label);
    font-weight: 550;
    line-height: 16px;
  }

  ul {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  button {
    box-sizing: border-box;
    block-size: 30px;
    padding: 0 13px;
    border: 0;
    border-radius: var(--radius-capsule);
    background: var(--color-fill);
    color: var(--color-label-secondary);
    font: inherit;
    font-size: var(--text-body);
    font-weight: 500;
    cursor: default;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      color var(--motion-instant) var(--ease-smooth),
      scale var(--motion-fast) var(--ease-smooth);
  }

  button:focus-visible {
    outline: 2px solid var(--color-ring);
  }

  button:hover {
    background: var(--color-fill-strong);
    color: var(--color-text);
  }

  button:active {
    scale: 0.97;
  }
</style>
