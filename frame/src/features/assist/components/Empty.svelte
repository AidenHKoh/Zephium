<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { tabs } from "$domain/tabs";
  import { favicons } from "$domain/favicons";
  import Button from "$shared/ui/Button";
  import FavIcon from "$shared/ui/FavIcon";
  import Icon from "$shared/ui/Icon";
  import { ZephiumMark } from "$shared/brand/zephium";
  import {
    commands,
    type PageAssistStatusV1,
    type PageAssistSuggestionV1,
  } from "$shared/ipc/bindings";
  import { availabilityView } from "../lib/faults";
  import { isRestyle, suggestionLabel } from "../lib/suggestions";
  import { Globe02Icon } from "../lib/icons";
  import "$shared/ui/Shine/shine.css";
  import "./assist.css";

  let {
    status,
    onsuggest,
  }: {
    status: PageAssistStatusV1;
    onsuggest: (suggestion: PageAssistSuggestionV1) => void;
  } = $props();

  const page = $derived(status.availability === "ready" ? status.page : null);
  const tab = $derived(page ? tabs.tabs().find((known) => known.id === page.tab) : undefined);
  const mark = $derived(tab ? favicons.mark(tab.icon, tab.url) : null);
  const away = $derived(
    status.availability === "ready" ? null : availabilityView(status.availability),
  );
  const groups = $derived(
    [
      {
        label: m.assist_group_understand(),
        items: status.suggestions.filter((s) => !isRestyle(s)),
      },
      { label: m.assist_group_restyle(), items: status.suggestions.filter(isRestyle) },
    ].filter((group) => group.items.length > 0),
  );
</script>

<div class="empty">
  {#if page}
    <div class="page ui-shine assist-rise">
      <span class="mark"
        ><FavIcon
          image={mark?.image ?? null}
          tone={mark?.tone}
          size={20}
          fallback={Globe02Icon}
        /></span
      >
      <span class="words">
        <strong>{page.title || page.host}</strong>
        <span>{page.host}</span>
      </span>
    </div>
    {#each groups as group, at (group.label)}
      <section class="group assist-rise" style:animation-delay="{60 + at * 50}ms">
        <h3>{group.label}</h3>
        <ul>
          {#each group.items as suggestion (suggestion)}
            <li>
              <button
                type="button"
                class="ui-shine ui-shine-press"
                onclick={() => onsuggest(suggestion)}>{suggestionLabel(suggestion)}</button
              >
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  {:else if away}
    <div class="away assist-rise">
      <span class="glyph" aria-hidden="true"><Icon icon={ZephiumMark} size={20} /></span>
      <h3>{away.title}</h3>
      <p>{away.help}</p>
      {#if away.talks}<p class="still">{m.assist_still()}</p>{/if}
      {#if away.fix}{@const fix = away.fix}<Button
          size="compact"
          onclick={() => void commands.runCommand(fix.command).catch(() => {})}>{fix.label}</Button
        >{/if}
    </div>
  {/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    gap: 20px;
    margin-block-start: auto;
    padding-block: 12px 4px;
  }

  .page {
    --shine-radius: var(--radius-row);

    display: flex;
    align-items: center;
    gap: 11px;
    padding: 11px 12px;
  }

  .mark {
    display: grid;
    flex: none;
    place-items: center;
    inline-size: 32px;
    block-size: 32px;
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
    font-size: 13.5px;
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
    font-size: 12px;
    line-height: 16px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  h3 {
    margin: 0;
    padding-inline: 2px;
    color: var(--color-muted);
    font-size: 12px;
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

  ul button {
    --shine-radius: var(--radius-capsule);

    box-sizing: border-box;
    block-size: 30px;
    padding: 0 13px;
    border: 0;
    color: var(--color-label-secondary);
    font: inherit;
    font-size: 13px;
    font-weight: 500;
  }

  ul button:hover {
    color: var(--color-text);
  }

  .away {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding-inline: 2px;
  }

  .glyph {
    display: grid;
    place-items: center;
    inline-size: 36px;
    block-size: 36px;
    margin-block-end: 6px;
    border-radius: var(--radius-row);
    background: var(--row-active);
    color: var(--color-muted);
  }

  .away h3 {
    padding: 0;
    color: var(--color-text);
    font-size: var(--text-page-title);
    font-weight: 600;
    line-height: 1.35;
  }

  .away p {
    margin: 0;
    color: var(--color-muted);
    font-size: 13px;
    line-height: 1.45;
    text-wrap: pretty;
  }

  .away .still {
    color: var(--color-label-secondary);
  }

  .away :global(.ui-button) {
    margin-block-start: 6px;
  }
</style>
