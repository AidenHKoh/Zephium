<script lang="ts">
  import "./space-hue.css";
  import type { SpaceIconView } from "$shared/ipc/bindings";
  import Icon from "$shared/ui/Icon";
  import { SPACE_COLORS, SPACE_SYMBOLS } from "../lib/space-look";

  let {
    icon,
    color,
    name,
    size = 16,
    fallback = "none",
  }: {
    icon: SpaceIconView | null;
    color: number;
    name: string;
    size?: number;
    /** What a space with no icon shows where a mark is needed. */
    fallback?: "none" | "dot" | "monogram";
  } = $props();
  const hue = $derived(Number.isInteger(color) && color >= 0 && color < SPACE_COLORS ? color : 0);
  const symbol = $derived(
    icon?.kind === "symbol" ? (SPACE_SYMBOLS[icon.index] ?? SPACE_SYMBOLS[0]) : null,
  );
  const initial = $derived([...name.trim()][0]?.toLocaleUpperCase() ?? "");
</script>

<!-- The mark alone, in the space's hue: a glyph drawn in it, an emoji as it
     is, and for a space without one a point of its hue or its initial. -->
{#if icon !== null || fallback !== "none"}<span
    class="space-glyph"
    data-space-hue={hue}
    data-kind={icon?.kind ?? fallback}
    style:--glyph-size={`${size}px`}
    aria-hidden="true"
    >{#if icon?.kind === "emoji"}<span class="emoji">{icon.text}</span>{:else if symbol}<Icon
        icon={symbol}
        {size}
        strokeWidth={size >= 24 ? 1.6 : 1.8}
      />{:else if fallback === "monogram"}{initial}{:else}<i></i>{/if}</span
  >{/if}

<style>
  .space-glyph {
    display: grid;
    flex: none;
    inline-size: var(--glyph-size);
    block-size: var(--glyph-size);
    color: var(--space-ink);
    place-items: center;
  }

  .emoji {
    font-size: calc(var(--glyph-size) * 0.94);
    line-height: 1;
  }

  .space-glyph[data-kind="monogram"] {
    font-size: calc(var(--glyph-size) * 0.86);
    font-weight: 650;
    letter-spacing: -0.01em;
    line-height: 1;
  }

  .space-glyph[data-kind="dot"] i {
    inline-size: 7px;
    block-size: 7px;
    border-radius: 50%;
    background: var(--space-hue);
  }
</style>
