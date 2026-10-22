<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import Icon from "$shared/ui/Icon";
  import { InformationCircleIcon } from "@hugeicons/core-free-icons";
  import type { PageAssistCiteV1, PageAssistPartV1 } from "$shared/ipc/bindings";
  import "./assist.css";

  let {
    parts,
    inert = false,
    oncite,
  }: {
    parts: readonly PageAssistPartV1[];
    /** The page has moved on: a source can no longer be scrolled to. */
    inert?: boolean;
    oncite: (n: number) => void;
  } = $props();

  type Segment =
    | { kind: "answer" | "caveat"; at: number; part: PageAssistPartV1 }
    | { kind: "point" | "figure"; at: number; parts: PageAssistPartV1[] };

  /** Parts arrive in the core's order; neighbours of one kind read as one list or one row of figures. */
  const segments = $derived.by(() => {
    const out: Segment[] = [];
    parts.forEach((part, at) => {
      const last = out.at(-1);
      if (part.kind === "point" || part.kind === "figure") {
        if (last && last.kind === part.kind) last.parts.push(part);
        else out.push({ kind: part.kind, at, parts: [part] });
      } else out.push({ kind: part.kind, at, part });
    });
    return out;
  });
</script>

{#snippet chips(cites: readonly PageAssistCiteV1[])}
  {#each cites as cite (cite.n)}
    <button
      type="button"
      class="cite"
      disabled={inert}
      title={inert ? m.assist_cite_gone() : cite.excerpt}
      aria-label={m.assist_cite_label({ n: cite.n, excerpt: cite.excerpt })}
      onclick={() => oncite(cite.n)}>{cite.n}</button
    >
  {/each}
{/snippet}

<article class="answer" aria-label={m.assist_answer_label()}>
  {#each segments as segment (segment.at)}
    {#if segment.kind === "answer"}
      <h3 class="headline assist-rise">{segment.part.text}{@render chips(segment.part.cites)}</h3>
    {:else if segment.kind === "point"}
      <ul class="points">
        {#each segment.parts as part, index (index)}
          <li class="assist-rise">{part.text}{@render chips(part.cites)}</li>
        {/each}
      </ul>
    {:else if segment.kind === "figure"}
      <dl class="figures">
        {#each segment.parts as part, index (index)}
          <div class="assist-rise">
            <dd class="value">{part.text}{@render chips(part.cites)}</dd>
            <dt class="label">{part.label}</dt>
          </div>
        {/each}
      </dl>
    {:else if segment.kind === "caveat"}
      <p class="caveat assist-rise">
        <span class="mark"><Icon icon={InformationCircleIcon} size={14} /></span>
        <span>{segment.part.text}</span>
      </p>
    {/if}
  {/each}
</article>

<style>
  .answer {
    display: flex;
    flex-direction: column;
    gap: 12px;
    color: var(--color-text);
  }

  .headline {
    margin: 0;
    font-size: 17px;
    font-weight: 620;
    line-height: 1.32;
    letter-spacing: -0.016em;
    text-wrap: pretty;
  }

  .points {
    display: flex;
    flex-direction: column;
    gap: 9px;
    margin: 0;
    padding: 0;
    list-style: none;
    color: var(--color-label-secondary);
    font-size: 13.5px;
    line-height: 1.5;
    text-wrap: pretty;
  }

  .points li {
    position: relative;
    padding-inline-start: 15px;
  }

  .points li::before {
    position: absolute;
    inset-block-start: 0.68em;
    inset-inline-start: 1px;
    inline-size: 4px;
    block-size: 4px;
    border-radius: var(--radius-capsule);
    background: var(--color-faint);
    content: "";
  }

  .figures {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(96px, 1fr));
    gap: 14px 18px;
    margin: 2px 0 0;
    padding-block-start: 12px;
    border-block-start: 1px solid var(--color-border);
  }

  .figures div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-inline-size: 0;
  }

  .value {
    order: 1;
    margin: 0;
    font-size: 20px;
    font-weight: 640;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.02em;
    line-height: 1.15;
    overflow-wrap: anywhere;
  }

  .label {
    order: 2;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 1.35;
  }

  .caveat {
    display: flex;
    gap: 7px;
    margin: 0;
    color: var(--color-muted);
    font-size: 12.5px;
    line-height: 1.45;
    text-wrap: pretty;
  }

  .mark {
    display: grid;
    flex: none;
    place-items: center;
    block-size: 1.45em;
    color: var(--color-faint);
  }

  /* A source marker: small enough to sit in a line of text, solid enough to hit. */
  .cite {
    display: inline-grid;
    place-items: center;
    box-sizing: border-box;
    min-inline-size: 17px;
    block-size: 17px;
    margin-inline-start: 5px;
    padding: 0 4px;
    border: 0;
    border-radius: var(--radius-inset);
    background: var(--color-fill);
    color: var(--color-muted);
    font: inherit;
    font-size: 10.5px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    line-height: 1;
    vertical-align: 0.12em;
    cursor: default;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      color var(--motion-instant) var(--ease-smooth),
      scale var(--motion-fast) var(--ease-smooth);
  }

  .cite:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: 1px;
  }

  .cite:disabled {
    opacity: 0.42;
  }

  .cite:hover:not(:disabled) {
    background: var(--color-fill-strong);
    color: var(--color-text);
  }

  .cite:active:not(:disabled) {
    scale: 0.92;
  }

  @media (forced-colors: active) {
    .cite {
      border: 1px solid ButtonText;
    }
  }
</style>
