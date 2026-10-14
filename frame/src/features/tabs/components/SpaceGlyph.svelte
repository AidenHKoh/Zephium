<script lang="ts">
  import type { SpaceIconView } from "$shared/ipc/bindings";
  import Icon from "$shared/ui/Icon";
  import { SPACE_COLORS, SPACE_SYMBOLS } from "../lib/space-look";

  let {
    icon,
    color,
    name,
    size = 20,
  }: { icon: SpaceIconView | null; color: number; name: string; size?: number } = $props();
  const hue = $derived(Number.isInteger(color) && color >= 0 && color < SPACE_COLORS ? color : 0);
  const symbol = $derived(
    icon?.kind === "symbol" ? (SPACE_SYMBOLS[icon.index] ?? SPACE_SYMBOLS[0]) : null,
  );
  const initial = $derived([...name.trim()][0]?.toLocaleUpperCase() ?? "");
</script>

<!-- A space wears its hue as a soft squircle; an emoji sits on a quieter
     wash of it, so the picture stays the colour it is. -->
<span
  class="space-glyph"
  data-color={hue}
  data-emoji={icon?.kind === "emoji" || undefined}
  style:--glyph-size={`${size}px`}
  aria-hidden="true"
  >{#if icon?.kind === "emoji"}<span class="emoji">{icon.text}</span>{:else if symbol}<Icon
      icon={symbol}
      size={Math.round(size * 0.6)}
      strokeWidth={size >= 28 ? 1.7 : 1.9}
    />{:else}<span class="letter">{initial}</span>{/if}</span
>

<style>
  .space-glyph {
    --glyph-top: var(--avatar-0-top);
    --glyph-bottom: var(--avatar-0-bottom);

    position: relative;
    display: grid;
    flex: none;
    inline-size: var(--glyph-size);
    block-size: var(--glyph-size);
    border-radius: calc(var(--glyph-size) * 0.32);
    background: linear-gradient(180deg, var(--glyph-top), var(--glyph-bottom));
    color: var(--avatar-ink);
    place-items: center;
  }

  .space-glyph::after {
    position: absolute;
    border-radius: inherit;
    box-shadow:
      inset 0 0 0 0.5px var(--avatar-rim),
      var(--shine-top);
    content: "";
    inset: 0;
    pointer-events: none;
  }

  .space-glyph[data-emoji] {
    background: color-mix(in oklab, var(--glyph-top) 22%, transparent);
  }

  .emoji {
    font-size: calc(var(--glyph-size) * 0.68);
    line-height: 1;
  }

  .letter {
    font-size: calc(var(--glyph-size) * 0.5);
    font-weight: 650;
    line-height: 1;
  }

  .space-glyph[data-color="1"] {
    --glyph-top: var(--avatar-1-top);
    --glyph-bottom: var(--avatar-1-bottom);
  }

  .space-glyph[data-color="2"] {
    --glyph-top: var(--avatar-2-top);
    --glyph-bottom: var(--avatar-2-bottom);
  }

  .space-glyph[data-color="3"] {
    --glyph-top: var(--avatar-3-top);
    --glyph-bottom: var(--avatar-3-bottom);
  }

  .space-glyph[data-color="4"] {
    --glyph-top: var(--avatar-4-top);
    --glyph-bottom: var(--avatar-4-bottom);
  }

  .space-glyph[data-color="5"] {
    --glyph-top: var(--avatar-5-top);
    --glyph-bottom: var(--avatar-5-bottom);
  }

  .space-glyph[data-color="6"] {
    --glyph-top: var(--avatar-6-top);
    --glyph-bottom: var(--avatar-6-bottom);
  }

  .space-glyph[data-color="7"] {
    --glyph-top: var(--avatar-7-top);
    --glyph-bottom: var(--avatar-7-bottom);
  }

  .space-glyph[data-color="8"] {
    --glyph-top: var(--avatar-8-top);
    --glyph-bottom: var(--avatar-8-bottom);
  }

  .space-glyph[data-color="9"] {
    --glyph-top: var(--avatar-9-top);
    --glyph-bottom: var(--avatar-9-bottom);
  }

  @media (forced-colors: active) {
    .space-glyph {
      background: Canvas;
      color: CanvasText;
      outline: 1px solid CanvasText;
    }
  }
</style>
