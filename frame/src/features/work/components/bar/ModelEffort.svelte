<script module lang="ts">
  import type { WorkModelReasoning } from "$shared/ipc/bindings";

  const EFFORTS: readonly WorkModelReasoning[] = ["low", "medium", "high", "xhigh", "max"];

  /** The level a model runs `wanted` at: the closest it takes, the stronger on a tie (as Rust). */
  export function nearestEffort(
    levels: readonly WorkModelReasoning[],
    wanted: WorkModelReasoning,
  ): WorkModelReasoning | null {
    const rank = EFFORTS.indexOf(wanted);
    let best: WorkModelReasoning | null = null;
    for (const level of levels) {
      const gap = Math.abs(EFFORTS.indexOf(level) - rank);
      if (best === null || gap <= Math.abs(EFFORTS.indexOf(best) - rank)) best = level;
    }
    return best;
  }

  export function stepEffort(
    levels: readonly WorkModelReasoning[],
    from: WorkModelReasoning,
    step: number,
  ): WorkModelReasoning | null {
    const at = levels.indexOf(from);
    const next = levels[Math.max(0, Math.min(levels.length - 1, at + step))];
    return next && next !== from ? next : null;
  }
</script>

<script lang="ts">
  import { Orb } from "$shared/ui/presence";
  import { stillWhenUnseen } from "$shared/lib/still";
  import { effortName } from "$domain/ai";
  import * as m from "$shared/i18n/messages";

  let {
    levels,
    value,
    onchange,
  }: {
    levels: readonly WorkModelReasoning[];
    /** The level the hero's model runs at; null when it takes none. */
    value: WorkModelReasoning | null;
    onchange: (level: WorkModelReasoning) => void;
  } = $props();

  const HINTS: Record<WorkModelReasoning, () => string> = {
    low: m.model_effort_low_hint,
    medium: m.model_effort_medium_hint,
    high: m.model_effort_high_hint,
    xhigh: m.model_effort_xhigh_hint,
    max: m.model_effort_max_hint,
  };
  /** How fast the orb turns at each level: it idles at Low and races at Max. */
  const PACE: Record<WorkModelReasoning, number> = {
    low: 0.28,
    medium: 0.5,
    high: 0.8,
    xhigh: 1.2,
    max: 1.75,
  };

  const live = $derived(levels.length > 0 && value !== null);
  const index = $derived(value ? levels.indexOf(value) : -1);
  const level = $derived<WorkModelReasoning>(value ?? "low");
  const reach = $derived(levels.length > 1 ? Math.max(0, index) / (levels.length - 1) : 0);

  let core = $state<HTMLElement>();
  let track = $state<HTMLElement>();
  $effect(() => {
    const rate = live ? PACE[level] : 0.2;
    const host = core;
    if (!host) return;
    // The orb starts its own turn when it mounts; its pace is set once that exists.
    const pace = () => {
      for (const run of host.getAnimations({ subtree: true })) run.updatePlaybackRate(rate);
    };
    pace();
    queueMicrotask(pace);
  });

  function key(event: KeyboardEvent) {
    if (!live || !value) return;
    const rtl = getComputedStyle(event.currentTarget as Element).direction === "rtl";
    const steps: Record<string, number> = {
      ArrowRight: rtl ? -1 : 1,
      ArrowLeft: rtl ? 1 : -1,
      ArrowUp: 1,
      ArrowDown: -1,
      PageUp: 1,
      PageDown: -1,
    };
    let next: WorkModelReasoning | null;
    if (event.key === "Home") next = levels[0] ?? null;
    else if (event.key === "End") next = levels.at(-1) ?? null;
    else if (event.key in steps) next = stepEffort(levels, value, steps[event.key]!);
    else return;
    event.preventDefault();
    event.stopPropagation();
    if (next && next !== value) onchange(next);
  }

  const THUMB = 16;

  /** The stop nearest the pointer along the track; the thumb is as wide as its margin. */
  function scrub(event: PointerEvent) {
    if (!live || !track) return;
    const box = track.getBoundingClientRect();
    const span = Math.max(1, box.width - THUMB);
    let fraction = (event.clientX - box.left - THUMB / 2) / span;
    if (getComputedStyle(track).direction === "rtl") fraction = 1 - fraction;
    const next = levels[Math.round(Math.max(0, Math.min(1, fraction)) * (levels.length - 1))];
    if (next && next !== value) onchange(next);
  }

  function down(event: PointerEvent) {
    if (event.button !== 0 || !live) return;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    scrub(event);
  }

  function move(event: PointerEvent) {
    if ((event.currentTarget as HTMLElement).hasPointerCapture(event.pointerId)) scrub(event);
  }
</script>

<div class="effort" data-level={value ?? undefined}>
  <span class="well" aria-hidden="true" use:stillWhenUnseen>
    <span class="halo"></span>
    <span class="core" bind:this={core}><Orb size={36} /></span>
  </span>
  <span class="words">
    {#key live ? level : null}<span class="name"
        >{live ? effortName(level) : m.model_effort_fixed()}</span
      ><span class="hint">{live ? HINTS[level]() : m.model_effort_fixed_hint()}</span>{/key}
  </span>
  {#if live && levels.length > 1}<div
      class="track"
      bind:this={track}
      role="slider"
      tabindex="0"
      aria-label={m.model_effort()}
      aria-valuemin={0}
      aria-valuemax={levels.length - 1}
      aria-valuenow={index}
      aria-valuetext={effortName(level)}
      style:--reach={reach}
      onkeydown={key}
      onpointerdown={down}
      onpointermove={move}
    >
      <span class="groove">
        <span class="fill"></span>
      </span>
      <span class="ticks">
        {#each levels as stop, step (stop)}<span
            class="tick"
            data-on={step <= index ? "" : undefined}
          ></span>{/each}
      </span>
      <span class="thumb"></span>
    </div>{/if}
</div>

<style>
  .effort {
    --alive-a: color-mix(in oklab, var(--color-soft-sky) 66%, var(--color-text));
    --alive-b: color-mix(in oklab, var(--color-soft-lilac) 66%, var(--color-text));

    display: flex;
    align-items: center;
    gap: 12px;
    min-block-size: 56px;
    padding: 6px 8px;
    border-radius: calc(var(--radius-menu) - var(--menu-pad));
    background: var(--color-fill);
  }

  .well {
    position: relative;
    display: grid;
    flex: none;
    inline-size: 40px;
    block-size: 40px;
    place-items: center;
  }

  /* The orb gathers itself as the effort rises: dimmer and smaller at Low, full at Max. */
  .core {
    --orb-tone: var(--color-text);

    display: grid;
    place-items: center;
    opacity: 0.4;
    transform: scale(0.76);
    transition:
      opacity var(--motion-slow) var(--ease-out),
      transform var(--motion-page) var(--ease-spring);
  }

  [data-level="medium"] .core {
    opacity: 0.66;
    transform: scale(0.84);
  }

  [data-level="high"] .core {
    opacity: 0.86;
    transform: scale(0.92);
  }

  [data-level="xhigh"] .core,
  [data-level="max"] .core {
    opacity: 1;
    transform: scale(1);
  }

  [data-level="xhigh"] .core {
    --orb-tone: linear-gradient(
      180deg,
      color-mix(in oklab, var(--alive-a) 45%, var(--color-text)),
      var(--color-text)
    );
  }

  [data-level="max"] .core {
    --orb-tone: linear-gradient(
      180deg,
      var(--color-text) 10%,
      var(--alive-a) 55%,
      var(--alive-b) 90%
    );
  }

  .halo {
    position: absolute;
    inset: -10px;
    border-radius: 50%;
    background: radial-gradient(
      closest-side,
      color-mix(in oklab, var(--color-soft-lilac) 36%, transparent),
      color-mix(in oklab, var(--color-soft-sky) 14%, transparent) 58%,
      transparent
    );
    opacity: 0;
    transform: scale(0.6);
    transition:
      opacity var(--motion-slow) var(--ease-out),
      transform var(--motion-page) var(--ease-out);
  }

  [data-level="xhigh"] .halo {
    opacity: 0.4;
    transform: scale(0.8);
  }

  [data-level="max"] .halo {
    opacity: 1;
    transform: scale(1);
    animation: breathe 2.6s var(--ease-in-out) infinite alternate;
  }

  .well:global([data-still]) .halo {
    animation-play-state: paused;
  }

  .words {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 1px;
    min-inline-size: 0;
  }

  .name {
    color: var(--color-text);
    font-size: var(--text-body);
    font-weight: 600;
    line-height: 17px;
    letter-spacing: -0.005em;
    animation: word-in var(--motion-base) var(--ease-out);
  }

  [data-level="max"] .name {
    background: linear-gradient(90deg, var(--alive-a), var(--alive-b));
    background-clip: text;
    color: transparent;
  }

  .hint {
    overflow: hidden;
    color: var(--color-muted);
    font-size: var(--text-caption);
    line-height: 14px;
    text-overflow: ellipsis;
    white-space: nowrap;
    animation: word-in var(--motion-base) var(--ease-out) 30ms backwards;
  }

  /* A groove with a bead on it: one detent for each level the model takes. */
  .track {
    --thumb: 16px;

    position: relative;
    flex: none;
    inline-size: 116px;
    block-size: 36px;
    margin-inline-end: 4px;
    outline: none;
    cursor: default;
    touch-action: none;
  }

  .groove {
    position: absolute;
    inset-block-start: 14px;
    inset-inline: 0;
    block-size: 6px;
    overflow: hidden;
    border-radius: var(--radius-capsule);
    background: color-mix(in oklab, var(--color-text) 11%, transparent);
    box-shadow: inset 0 0.5px 1px color-mix(in oklab, black 22%, transparent);
  }

  .fill {
    position: absolute;
    inset-block: 0;
    inset-inline-start: 0;
    inline-size: calc(var(--thumb) / 2 + var(--reach) * (100% - var(--thumb)));
    border-radius: inherit;
    background: color-mix(in oklab, var(--color-text) 58%, transparent);
    transition: inline-size var(--motion-base) var(--ease-out);
  }

  [data-level="max"] .fill {
    background: linear-gradient(90deg, var(--alive-a), var(--alive-b));
  }

  .ticks {
    position: absolute;
    inset-block-start: 25px;
    inset-inline: calc(var(--thumb) / 2 - 0.75px);
    display: flex;
    justify-content: space-between;
  }

  .tick {
    inline-size: 1.5px;
    block-size: 4px;
    border-radius: 1px;
    background: var(--color-text);
    opacity: 0.2;
    transition: opacity var(--motion-base) var(--ease-out);
  }

  .tick[data-on] {
    opacity: 0.42;
  }

  .thumb {
    position: absolute;
    inset-block-start: 10px;
    inset-inline-start: calc(var(--reach) * (100% - var(--thumb)));
    inline-size: var(--thumb);
    block-size: var(--thumb);
    border-radius: 50%;
    background: var(--color-text);
    box-shadow:
      0 0 0 3px color-mix(in oklab, var(--color-fill) 100%, transparent),
      0 1px 3px color-mix(in oklab, black 36%, transparent);
    transition:
      inset-inline-start var(--motion-base) var(--ease-out),
      scale var(--motion-fast) var(--ease-out);
  }

  [data-level="max"] .thumb {
    background: linear-gradient(150deg, var(--alive-a), var(--alive-b));
    box-shadow:
      0 0 0 3px color-mix(in oklab, var(--color-fill) 100%, transparent),
      0 0 0 5px color-mix(in oklab, var(--alive-b) 24%, transparent);
  }

  .track:hover .thumb {
    scale: 1.08;
  }

  .track:active .thumb {
    scale: 0.94;
  }

  .track:focus-visible .groove {
    box-shadow: 0 0 0 1.5px var(--color-ring);
  }

  @keyframes breathe {
    from {
      transform: scale(0.9);
    }

    to {
      transform: scale(1.08);
    }
  }

  @keyframes word-in {
    from {
      opacity: 0;
      transform: translateY(3px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .core,
    .halo,
    .fill,
    .thumb,
    .tick {
      transition: none;
    }

    .name,
    .hint,
    [data-level="max"] .halo {
      animation: none;
    }
  }

  @media (forced-colors: active) {
    .thumb,
    .tick[data-on] {
      background: CanvasText;
    }

    .fill {
      background: Highlight;
    }
  }
</style>
