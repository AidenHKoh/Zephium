<script module lang="ts">
  import type { WorkModelEntry, WorkModelReasoning } from "$shared/ipc/bindings";
  import type { Tier } from "./ModelTier.svelte";

  export type HeroSubject = {
    entry: WorkModelEntry;
    tier: Tier | null;
    levels: readonly WorkModelReasoning[];
    effort: WorkModelReasoning | null;
  };
</script>

<script lang="ts">
  import { contextSize, dollars } from "$domain/ai";
  import ProviderMark from "$shared/ui/ProviderMark";
  import * as m from "$shared/i18n/messages";
  import ModelEffort from "./ModelEffort.svelte";
  import ModelTier from "./ModelTier.svelte";

  let {
    subject,
    onEffort,
  }: { subject: HeroSubject; onEffort: (level: WorkModelReasoning) => void } = $props();

  const TIER_NAMES: Record<Tier, () => string> = {
    light: m.model_tier_light,
    balanced: m.model_tier_balanced,
    flagship: m.model_tier_flagship,
  };
  const CHARACTERS: Record<Tier, () => string> = {
    light: m.model_character_light,
    balanced: m.model_character_balanced,
    flagship: m.model_character_flagship,
  };

  const provider = $derived(subject.entry.model.provider);
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
</script>

<section class="hero" aria-label={name}>
  {#key subject.entry.id}<div class="identity" aria-hidden="true">
      <span class="tile"><ProviderMark {provider} size={22} /></span>
      <span class="who">
        <span class="name">{name}</span>
        {#if character}<span class="character">{character}</span>{/if}
      </span>
      {#if subject.tier}<span class="tier" title={price}
          ><ModelTier tier={subject.tier} /><span>{TIER_NAMES[subject.tier]()}</span></span
        >{/if}
    </div>{/key}
  <ModelEffort levels={subject.levels} value={subject.effort} onchange={onEffort} />
</section>

<style>
  .hero {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: 10px;
    padding: 8px 6px 4px;
  }

  .identity {
    display: flex;
    align-items: center;
    gap: 12px;
    min-block-size: 40px;
    padding-inline: 4px 6px;
    animation: hero-in var(--motion-base) var(--ease-out);
  }

  .tile {
    display: grid;
    flex: none;
    inline-size: 40px;
    block-size: 40px;
    place-items: center;
    border-radius: var(--radius-row);
    background: var(--row-active);
    box-shadow: var(--shine-top);
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

  .tier {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 5px;
    align-self: flex-start;
    margin-block-start: 3px;
    color: var(--color-faint);
    font-size: var(--text-caption);
    font-weight: 550;
    line-height: 14px;
    letter-spacing: 0.01em;
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
  }
</style>
