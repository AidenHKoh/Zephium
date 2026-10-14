<script module lang="ts">
  import type { WorkModelReasoning } from "$shared/ipc/bindings";

  export const EFFORTS: readonly WorkModelReasoning[] = ["low", "medium", "high", "xhigh", "max"];

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

  let core = $state<HTMLElement>();
  let steps = $state<HTMLElement>();
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

  /** The dot nearest the pointer, while it is over the row of dots. */
  function scrub(event: PointerEvent) {
    if (!live || !steps) return;
    const dots = [...steps.querySelectorAll<HTMLElement>("[data-step]")];
    const box = steps.getBoundingClientRect();
    if (event.clientX < box.left - 8 || event.clientX > box.right + 8) return;
    let best: HTMLElement | undefined;
    let gap = Infinity;
    for (const dot of dots) {
      const rect = dot.getBoundingClientRect();
      const distance = Math.abs(rect.left + rect.width / 2 - event.clientX);
      if (distance < gap) [best, gap] = [dot, distance];
    }
    const next = best?.dataset.step as WorkModelReasoning | undefined;
    if (next && next !== value) onchange(next);
  }

  function down(event: PointerEvent) {
    if (event.button !== 0 || !live) return;
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    scrub(event);
  }

  function move(event: PointerEvent) {
    if ((event.currentTarget as HTMLElement).hasPointerCapture(event.pointerId)) scrub(event);
  }
</script>

{#snippet body()}
  <span class="well" aria-hidden="true" use:stillWhenUnseen>
    <span class="halo"></span>
    <span class="core" bind:this={core}><Orb size={36} /></span>
  </span>
  <span class="words">
    {#key live ? level : null}<span class="name"
        >{live ? effortName(level) : m.model_effort_fixed()}</span
      ><span class="hint">{live ? HINTS[level]() : m.model_effort_fixed_hint()}</span>{/key}
  </span>
  {#if live && levels.length > 1}<span class="steps" bind:this={steps} aria-hidden="true">
      {#each levels as stop, step (stop)}<span
          class="step"
          data-step={stop}
          data-on={step <= index ? "" : undefined}
          data-at={step === index ? "" : undefined}
          style:--d="{5 + EFFORTS.indexOf(stop)}px"
        ></span>{/each}
    </span>{/if}
{/snippet}

{#if live}<div
    class="effort"
    role="slider"
    tabindex="0"
    aria-label={m.model_effort()}
    aria-valuemin={0}
    aria-valuemax={levels.length - 1}
    aria-valuenow={index}
    aria-valuetext={effortName(level)}
    data-level={level}
    onkeydown={key}
    onpointerdown={down}
    onpointermove={move}
  >
    {@render body()}
  </div>{:else}<div class="effort">{@render body()}</div>{/if}

<style>
  .effort {
    --alive-a: color-mix(in oklab, var(--color-soft-sky) 66%, var(--color-text));
    --alive-b: color-mix(in oklab, var(--color-soft-lilac) 66%, var(--color-text));

    display: flex;
    align-items: center;
    gap: 12px;
    min-block-size: 52px;
    padding: 6px 14px 6px 8px;
    border-radius: calc(var(--radius-menu) - var(--menu-pad));
    background: var(--color-fill);
    outline: none;
    cursor: default;
    touch-action: none;
    transition: background-color var(--motion-fast) var(--ease-out);
  }

  .effort[data-level]:hover {
    background: color-mix(in oklab, var(--color-fill), var(--row-active));
  }

  .effort:focus-visible {
    box-shadow: inset 0 0 0 1.5px var(--color-ring);
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

  /* Each level's dot is as large as the thought it stands for. */
  .steps {
    display: flex;
    flex: none;
    align-items: center;
    gap: 9px;
    block-size: 28px;
    padding-inline: 4px;
  }

  .step {
    inline-size: var(--d);
    block-size: var(--d);
    border-radius: 50%;
    background: var(--color-text);
    opacity: 0.16;
    transition:
      opacity var(--motion-base) var(--ease-out),
      transform var(--motion-page) var(--ease-spring);
  }

  .step[data-on] {
    opacity: 0.62;
  }

  .step[data-at] {
    opacity: 1;
    transform: scale(1.25);
  }

  [data-level="max"] .step[data-on] {
    background: linear-gradient(160deg, var(--alive-a), var(--alive-b));
    opacity: 1;
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
    .step {
      transition: none;
    }

    .name,
    .hint,
    [data-level="max"] .halo {
      animation: none;
    }
  }

  @media (forced-colors: active) {
    .step[data-on] {
      background: CanvasText;
    }
  }
</style>
