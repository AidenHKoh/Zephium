<script module lang="ts">
  import type { WorkModelEntry, WorkModelReasoning } from "$shared/ipc/bindings";
  import type { Tier } from "./model-tier";

  export type HeroSubject = {
    entry: WorkModelEntry;
    tier: Tier | null;
    levels: readonly WorkModelReasoning[];
    effort: WorkModelReasoning | null;
  };
</script>

<script lang="ts">
  import { contextSize, dollars, effortName } from "$domain/ai";
  import Icon from "$shared/ui/Icon";
  import ProviderMark from "$shared/ui/ProviderMark";
  import * as m from "$shared/i18n/messages";
  import { ArrowDown01Icon } from "../../lib/icons";
  import ModelEffort from "./ModelEffort.svelte";
  import ModelEffortMark from "./ModelEffortMark.svelte";
  import "$shared/ui/Shine/shine.css";

  let {
    subject,
    expanded,
    region,
    ontoggle,
    onEffort,
    pill = $bindable(),
  }: {
    subject: HeroSubject;
    /** The effort control is open in place under the model. */
    expanded: boolean;
    /** The id of the region the effort button opens. */
    region: string;
    ontoggle: () => void;
    onEffort: (level: WorkModelReasoning) => void;
    pill?: HTMLElement;
  } = $props();

  const CHARACTERS: Record<Tier, () => string> = {
    light: m.model_character_light,
    balanced: m.model_character_balanced,
    flagship: m.model_character_flagship,
  };

  const name = $derived(subject.entry.display_name);

  /** What the model is like, then only what limits it. */
  const character = $derived.by(() => {
    const { entry, tier } = subject;
    const words: string[] = [];
    if (tier) words.push(CHARACTERS[tier]());
    else if (entry.model.provider === "compatible") words.push(m.model_character_own());
    if (entry.context_window < 500_000)
      words.push(m.model_context({ size: contextSize(entry.context_window) }));
    if (!entry.supports.vision) words.push(m.model_no_images());
    return words.join(" · ");
  });

  const price = $derived(
    subject.entry.price
      ? m.model_price({
          input: dollars(subject.entry.price.input),
          output: dollars(subject.entry.price.output),
        })
      : undefined,
  );

  const live = $derived(subject.levels.length > 1 && subject.effort !== null);

  /** The control is built while it is open and for the moment it folds away, so its orb never turns unseen. */
  let built = $state(false);
  $effect(() => {
    if (expanded) {
      built = true;
      return;
    }
    const timer = setTimeout(() => (built = false), 360);
    return () => clearTimeout(timer);
  });
</script>

<section class="hero" aria-label={name}>
  <div class="top">
    {#key subject.entry.id}<div class="identity" aria-hidden="true">
        <span class="tile ui-shine"
          ><ProviderMark provider={subject.entry.model.provider} size={22} /></span
        >
        <span class="who">
          <span class="name" title={price}>{name}</span>
          {#if character}<span class="character">{character}</span>{/if}
        </span>
      </div>{/key}
    {#if live && subject.effort}<button
        bind:this={pill}
        type="button"
        class="pill ui-shine ui-shine-press"
        aria-expanded={expanded}
        aria-controls={region}
        aria-label={m.model_effort_choose({ level: effortName(subject.effort) })}
        onclick={ontoggle}
      >
        <span class="pill-mark"
          ><ModelEffortMark levels={subject.levels} level={subject.effort} /></span
        >
        <span class="pill-label" data-level={subject.effort}>{effortName(subject.effort)}</span>
        <span class="pill-chevron" data-open={expanded}
          ><Icon icon={ArrowDown01Icon} size={12} strokeWidth={2.2} /></span
        >
      </button>{:else}<span class="fixed">{m.model_effort_fixed()}</span>{/if}
  </div>
  <div class="reveal" id={region} data-open={expanded}>
    <div class="reveal-inner" inert={!expanded}>
      <div class="reveal-body">
        {#if built}<ModelEffort
            levels={subject.levels}
            value={subject.effort}
            onchange={onEffort}
          />{/if}
      </div>
    </div>
  </div>
</section>

<style>
  .hero {
    display: flex;
    flex: none;
    flex-direction: column;
    padding: 14px 8px 8px;
  }

  .top {
    display: flex;
    align-items: center;
    gap: 10px;
    min-block-size: 40px;
  }

  .identity {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 12px;
    min-inline-size: 0;
    animation: hero-in var(--motion-base) var(--ease-out);
  }

  .tile {
    --shine-radius: var(--radius-row);

    display: grid;
    flex: none;
    inline-size: 40px;
    block-size: 40px;
    place-items: center;
    animation: tile-in var(--motion-page) var(--ease-spring);
  }

  .who {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-inline-size: 0;
  }

  .name {
    overflow: hidden;
    color: var(--color-text);
    font-size: 15px;
    font-weight: 600;
    line-height: 19px;
    letter-spacing: -0.012em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .character {
    overflow: hidden;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 16px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pill {
    --shine-radius: var(--radius-capsule);
    --alive-a: color-mix(in oklab, var(--color-soft-sky) 66%, var(--color-text));
    --alive-b: color-mix(in oklab, var(--color-soft-lilac) 66%, var(--color-text));

    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 8px;
    block-size: 30px;
    padding-inline: 11px 9px;
    border: 0;
    color: var(--color-muted);
    font: inherit;
    font-size: var(--text-label);
  }

  .pill:hover,
  .pill[aria-expanded="true"] {
    color: var(--color-text);
  }

  .pill-mark {
    display: grid;
    place-items: center;
  }

  .pill-label {
    color: var(--color-text);
    font-weight: 550;
    letter-spacing: -0.003em;
    white-space: nowrap;
  }

  .pill-label[data-level="max"] {
    background: linear-gradient(90deg, var(--alive-a), var(--alive-b));
    background-clip: text;
    color: transparent;
  }

  .pill-chevron {
    display: grid;
    place-items: center;
    transition: rotate var(--motion-base) var(--ease-out);
  }

  .pill-chevron[data-open="true"] {
    rotate: 180deg;
  }

  .fixed {
    flex: none;
    padding-inline: 4px;
    color: var(--color-faint);
    font-size: var(--text-caption);
  }

  /* The effort control opens in place: the rows below it give way as it grows. */
  .reveal {
    display: grid;
    grid-template-rows: 0fr;
    transition: grid-template-rows var(--motion-slow) var(--ease-emphasized);
  }

  .reveal[data-open="true"] {
    grid-template-rows: 1fr;
  }

  .reveal-inner {
    min-block-size: 0;
    overflow: hidden;
    opacity: 0;
    transition: opacity var(--motion-fast) var(--ease-exit);
  }

  .reveal-body {
    padding-block-start: 10px;
  }

  .reveal[data-open="true"] .reveal-inner {
    opacity: 1;
    transition: opacity var(--motion-base) var(--ease-out) 60ms;
  }

  @keyframes hero-in {
    from {
      opacity: 0;
      transform: translateY(3px);
    }
  }

  @keyframes tile-in {
    from {
      transform: scale(0.86);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .identity,
    .tile {
      animation: none;
    }

    .reveal,
    .reveal-inner,
    .pill-chevron {
      transition: none;
    }
  }
</style>
