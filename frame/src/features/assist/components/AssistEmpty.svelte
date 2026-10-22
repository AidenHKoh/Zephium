<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import Icon from "$shared/ui/Icon";
  import { ArrowRight01Icon } from "@hugeicons/core-free-icons";
  import type { PageAssistSuggestionV1 } from "$shared/ipc/bindings";
  import { suggestionLabel } from "../lib/suggestions";
  import "./assist.css";

  let {
    site,
    suggestions,
    onpick,
  }: {
    site: string | null;
    suggestions: readonly PageAssistSuggestionV1[];
    onpick: (suggestion: PageAssistSuggestionV1) => void;
  } = $props();
</script>

<div class="empty">
  <h3 class="assist-rise">{site ? m.assist_empty_site({ site }) : m.assist_empty()}</h3>
  <p class="assist-rise">{m.assist_empty_help()}</p>
  {#if suggestions.length}
    <ul class="suggestions">
      {#each suggestions as suggestion, index (suggestion)}
        <li class="assist-rise" style:animation-delay="{60 + index * 30}ms">
          <button type="button" onclick={() => onpick(suggestion)}>
            <span>{suggestionLabel(suggestion)}</span>
            <Icon icon={ArrowRight01Icon} size={14} />
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-block-start: 20px;
  }

  h3 {
    margin: 0;
    color: var(--color-text);
    font-size: 20px;
    font-weight: 640;
    letter-spacing: -0.02em;
    line-height: 1.2;
    overflow-wrap: anywhere;
    text-wrap: balance;
  }

  p {
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-body);
    line-height: 1.5;
    text-wrap: pretty;
  }

  .suggestions {
    display: flex;
    flex-direction: column;
    gap: 0;
    margin: 18px -8px 0;
    padding: 0;
    list-style: none;
  }

  li {
    position: relative;
  }

  li + li::before {
    position: absolute;
    inset-block-start: -1px;
    inset-inline: 12px;
    block-size: 1px;
    background: var(--color-border);
    content: "";
  }

  button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    inline-size: 100%;
    box-sizing: border-box;
    min-block-size: 36px;
    padding: 0 10px 0 12px;
    border: 0;
    border-radius: var(--radius-row);
    background: transparent;
    color: var(--color-label-secondary);
    font: inherit;
    font-size: var(--text-body);
    font-weight: 500;
    text-align: start;
    cursor: default;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      color var(--motion-instant) var(--ease-smooth);
  }

  button :global(svg) {
    flex: none;
    color: var(--color-faint);
    opacity: 0.4;
    transform: translateX(-2px);
    transition:
      opacity var(--motion-fast) var(--ease-smooth),
      transform var(--motion-fast) var(--ease-smooth);
  }

  button:hover,
  button:focus-visible {
    background: var(--color-fill-hover);
    color: var(--color-text);
  }

  button:hover :global(svg),
  button:focus-visible :global(svg) {
    opacity: 1;
    transform: none;
  }

  button:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: -2px;
  }

  @media (prefers-reduced-motion: reduce) {
    button :global(svg) {
      transform: none;
      transition: none;
    }
  }
</style>
