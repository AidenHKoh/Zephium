<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { preferences } from "$domain/preferences";
  import type { ThemeSpec } from "$domain/appearance";
  import MiniWindow from "./MiniWindow.svelte";

  let { spec, contrast, material }: { spec: ThemeSpec; contrast: number; material: string } =
    $props();

  const modes = [
    { value: "light", label: m.theme_light },
    { value: "dark", label: m.theme_dark },
    { value: "system", label: m.theme_system },
  ] as const;
</script>

<!-- The pictures are the choice, as the system's own Appearance pane draws
     it: one control, named under each picture, each drawn in this theme. -->
<div
  class="modes"
  role="radiogroup"
  aria-label={m.settings_color_scheme()}
  data-setting="appearance.theme"
>
  {#each modes as mode (mode.value)}
    {@const selected = preferences.value("appearance") === mode.value}
    <button
      type="button"
      role="radio"
      class="mode"
      aria-checked={selected}
      disabled={preferences.saving()}
      onclick={() => void preferences.set("appearance", mode.value)}
    >
      <span class="picture" data-selected={selected} aria-hidden="true">
        {#each mode.value === "system" ? (["light", "dark"] as const) : [mode.value] as tone (tone)}
          <span class="tone"
            ><MiniWindow {spec} {contrast} mode={tone} {material} detail={false} /></span
          >
        {/each}
      </span>
      <span class="label">{mode.label()}</span>
    </button>
  {/each}
</div>

<style>
  .modes {
    display: flex;
    gap: 14px;
    padding: 18px 18px 14px;
  }

  .mode {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    gap: 9px;
    min-width: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--color-label-secondary);
    font: inherit;
    font-size: var(--text-label);
    cursor: default;
  }

  .mode[aria-checked="true"] {
    color: var(--color-text);
    font-weight: 600;
  }

  .mode:focus-visible {
    outline: none;
  }

  .picture {
    position: relative;
    display: block;
    width: 100%;
    height: 88px;
    border-radius: var(--radius-row);
    box-shadow: inset 0 0 0 1px var(--color-border);
    transition:
      box-shadow var(--motion-fast) var(--ease-out),
      scale var(--motion-slow) var(--ease-spring);
  }

  .tone {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: inherit;
  }

  /* System is both: the dark window takes the lower right of the light one. */
  .tone + .tone {
    clip-path: polygon(100% 0, 100% 100%, 0 100%);
  }

  .mode:active .picture {
    scale: 0.98;
  }

  .picture[data-selected="true"] {
    box-shadow:
      0 0 0 2px var(--color-surface),
      0 0 0 3.5px var(--color-lit);
  }

  .mode:focus-visible .picture {
    box-shadow:
      0 0 0 2px var(--color-surface),
      0 0 0 3.5px var(--color-ring);
  }

  @container (max-width: 450px) {
    .modes {
      gap: 6px;
      padding-inline: 12px;
    }

    .picture {
      height: 70px;
    }
  }

  @media (forced-colors: active) {
    .picture[data-selected="true"] {
      outline: 2px solid Highlight;
      outline-offset: 2px;
    }
  }
</style>
