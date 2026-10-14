<script lang="ts">
  import type { WorkModelReasoning } from "$shared/ipc/bindings";
  import { watchStill } from "$shared/lib/still";
  import { effortName } from "$domain/ai";
  import * as m from "$shared/i18n/messages";

  let {
    levels,
    value,
    onchange,
    onclose,
  }: {
    levels: readonly WorkModelReasoning[];
    value: WorkModelReasoning;
    onchange: (level: WorkModelReasoning) => void;
    /** Escape from the track: the tray folds and focus goes back to its chip. */
    onclose: () => void;
  } = $props();

  let track = $state<HTMLElement>();
  let width = $state(0);
  /** Where a drag has the thumb before it lets go. */
  let dragging = $state<number | null>(null);
  const settled = $derived(Math.max(0, levels.indexOf(value)));
  const index = $derived(dragging ?? settled);
  const last = $derived(Math.max(1, levels.length - 1));
  const level = $derived(levels[index] ?? value);
  /** The strongest level is the agent at full thought: its fill comes alive. */
  const alive = $derived(index === levels.length - 1 && levels.length > 1);
  const THUMB = 14;
  const travel = $derived(Math.max(0, width - THUMB));
  const at = (step: number) => (step / last) * travel;

  let still = $state(true);
  $effect(() => {
    const element = track;
    if (!element || !alive) return;
    return watchStill(element, (next) => (still = next));
  });

  function set(step: number) {
    const next = levels[Math.max(0, Math.min(levels.length - 1, step))];
    if (next && next !== value) onchange(next);
  }

  function stepAt(event: PointerEvent) {
    const box = track!.getBoundingClientRect();
    const x = event.clientX - box.left - THUMB / 2;
    const ratio = travel ? x / travel : 0;
    const rtl = getComputedStyle(track!).direction === "rtl";
    return Math.round(Math.max(0, Math.min(1, rtl ? 1 - ratio : ratio)) * last);
  }

  function down(event: PointerEvent) {
    if (event.button !== 0 || !track) return;
    event.preventDefault();
    track.focus();
    track.setPointerCapture(event.pointerId);
    dragging = stepAt(event);
  }

  function move(event: PointerEvent) {
    if (dragging === null || !track?.hasPointerCapture(event.pointerId)) return;
    dragging = stepAt(event);
  }

  function up(event: PointerEvent) {
    if (dragging === null) return;
    const step = dragging;
    dragging = null;
    if (track?.hasPointerCapture(event.pointerId)) track.releasePointerCapture(event.pointerId);
    set(step);
  }

  function key(event: KeyboardEvent) {
    const steps: Record<string, number> = {
      ArrowRight: 1,
      ArrowUp: 1,
      ArrowLeft: -1,
      ArrowDown: -1,
      PageUp: 1,
      PageDown: -1,
    };
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onclose();
      return;
    }
    let next: number | null = null;
    if (event.key === "Home") next = 0;
    else if (event.key === "End") next = levels.length - 1;
    else if (event.key in steps) {
      const rtl = getComputedStyle(event.currentTarget as Element).direction === "rtl";
      const sideways = event.key === "ArrowLeft" || event.key === "ArrowRight";
      next = settled + steps[event.key]! * (rtl && sideways ? -1 : 1);
    }
    if (next === null) return;
    event.preventDefault();
    event.stopPropagation();
    set(next);
  }
</script>

<div class="effort" data-alive={alive ? "" : undefined}>
  <div class="head">
    <span class="caption">{m.model_effort()}</span>
    {#key level}<span class="level">{effortName(level)}</span>{/key}
  </div>
  <div
    class="track"
    bind:this={track}
    bind:clientWidth={width}
    role="slider"
    tabindex="0"
    aria-label={m.model_effort()}
    aria-valuemin={0}
    aria-valuemax={levels.length - 1}
    aria-valuenow={index}
    aria-valuetext={effortName(level)}
    data-dragging={dragging !== null ? "" : undefined}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={() => (dragging = null)}
    onkeydown={key}
  >
    <span class="rail" aria-hidden="true">
      <span
        class="fill"
        style:transform="scaleX({travel ? (at(index) + THUMB / 2) / (travel + THUMB) : 0})"
      ></span>
      {#if alive}<span
          class="life"
          data-still={still ? "" : undefined}
          style:inline-size="{at(index) + THUMB / 2}px"
          ><span class="dashes"></span><span class="band"></span></span
        >{/if}
    </span>
    {#each levels as stop, step (stop)}<span
        class="stop"
        class:reached={step <= index}
        aria-hidden="true"
        style:--x="{at(step) + THUMB / 2 - 2}px"
      ></span>{/each}
    <span class="thumb" aria-hidden="true" style:--x="{at(index)}px"></span>
  </div>
  <div class="ends" aria-hidden="true">
    <span>{m.model_faster()}</span>
    <span>{m.model_smarter()}</span>
  </div>
</div>

<style>
  .effort {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px 9px;
    animation: tray-in var(--motion-base) var(--ease-out);
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    font-size: var(--text-label);
    line-height: 16px;
  }

  .caption {
    color: var(--color-faint);
    font-weight: 550;
    letter-spacing: 0.01em;
  }

  .level {
    color: var(--color-text);
    font-weight: 550;
    animation: level-in var(--motion-fast) var(--ease-out);
  }

  [data-alive] .level {
    color: color-mix(in oklab, var(--color-agent-lead-sheen) 72%, var(--color-text));
  }

  .track {
    --dir: 1;

    position: relative;
    block-size: 22px;
    border-radius: var(--radius-capsule);
    outline: none;
    cursor: default;
    touch-action: none;
  }

  .rail {
    position: absolute;
    inset-inline: 0;
    inset-block-start: 9px;
    block-size: 4px;
    overflow: hidden;
    border-radius: var(--radius-capsule);
    background: var(--color-track);
  }

  .fill {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: color-mix(in oklab, var(--color-text) 62%, transparent);
    transform-origin: left center;
    transition: transform var(--motion-base) var(--ease-out);
  }

  .track:dir(rtl) {
    --dir: -1;
  }

  [data-alive] .fill {
    background: linear-gradient(
      90deg,
      color-mix(in oklab, var(--color-agent-lead-sheen) 68%, var(--color-text)),
      var(--color-agent-lead-sheen)
    );
  }

  .track:dir(rtl) .fill {
    transform-origin: right center;
  }

  /* Full thought: the fill carries the work the way a lead's line does, fine
     dashes flowing along it under one passing light. */
  .life {
    position: absolute;
    inset-block: 0;
    inset-inline-start: 0;
    overflow: hidden;
    border-radius: inherit;
  }

  .dashes {
    position: absolute;
    inset-block: 0;
    inset-inline: -8px 0;
    background: repeating-linear-gradient(
      90deg,
      transparent 0,
      color-mix(in oklab, var(--color-glint) 30%, transparent) 2px 4px,
      transparent 6px 8px
    );
    animation: flow 720ms linear infinite;
  }

  .band {
    position: absolute;
    inset-block: 0;
    inset-inline-start: 0;
    inline-size: 40%;
    background: linear-gradient(
      90deg,
      transparent,
      color-mix(in oklab, var(--color-glint), var(--color-agent-lead-sheen)),
      transparent
    );
    animation: band 2.4s linear infinite;
  }

  [data-still] .dashes,
  [data-still] .band {
    animation-play-state: paused;
  }

  [data-still] .band {
    visibility: hidden;
  }

  .stop {
    position: absolute;
    inset-block-start: 9px;
    inset-inline-start: 0;
    inline-size: 4px;
    block-size: 4px;
    border-radius: 50%;
    background: var(--color-faint);
    opacity: 0.7;
    transform: translateX(calc(var(--x) * var(--dir)));
  }

  .stop.reached {
    opacity: 0;
  }

  .thumb {
    position: absolute;
    inset-block-start: 4px;
    inset-inline-start: 0;
    inline-size: 14px;
    block-size: 14px;
    border-radius: 50%;
    background: var(--color-thumb);
    box-shadow: var(--shadow-thumb);
    transform: translateX(calc(var(--x) * var(--dir)));
    transition:
      transform var(--motion-base) var(--ease-out),
      box-shadow var(--motion-base) var(--ease-out);
  }

  [data-alive] .thumb {
    box-shadow:
      var(--shadow-thumb),
      0 0 0 3px color-mix(in oklab, var(--color-agent-lead-sheen) 28%, transparent);
  }

  .track:focus-visible .thumb {
    box-shadow:
      var(--shadow-thumb),
      0 0 0 2px var(--color-ring);
  }

  .track[data-dragging] .fill,
  .track[data-dragging] .thumb {
    transition-duration: var(--motion-instant);
  }

  [data-alive] .track:focus-visible .thumb {
    box-shadow:
      var(--shadow-thumb),
      0 0 0 2px var(--color-ring);
  }

  .ends {
    display: flex;
    justify-content: space-between;
    color: var(--color-faint);
    font-size: var(--text-caption);
    line-height: 14px;
  }

  @keyframes flow {
    to {
      transform: translateX(8px);
    }
  }

  @keyframes band {
    0% {
      transform: translateX(-100%);
    }

    72%,
    100% {
      transform: translateX(260%);
    }
  }

  @keyframes tray-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }

  @keyframes level-in {
    from {
      opacity: 0;
      transform: translateY(2px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .effort,
    .level {
      animation: none;
    }

    .fill,
    .thumb {
      transition: none;
    }
  }
</style>
