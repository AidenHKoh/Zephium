import { tabs } from "$domain/tabs";
import { haptic } from "$shared/lib/haptic";
import { duration, easing, reducedMotion } from "$shared/lib/motion";
import { SwipeTracker, type Reading, type SwipeStep } from "./swipe-track";

/** Wheel events stop arriving this long after the fingers lift and momentum fades. */
const IDLE = 90;
/** How long a committed list waits for its space to arrive before coming back. */
const ARRIVAL = 700;
/** How far the list leaves and enters, as a share of its width. */
const REACH = 0.5;

type Options = {
  canStep: (step: SwipeStep) => boolean;
  /** Resolves false when native refused the step. */
  step: (step: SwipeStep) => Promise<boolean> | void;
};

type Wheel = WheelEvent & { webkitDirectionInvertedFromDevice?: boolean };

type Control = { arrive: (side: SwipeStep) => void };

/** Native's answer to a step, as the list needs it: whether a space is coming. */
export async function stepSpace(step: SwipeStep): Promise<boolean> {
  const result = await tabs.stepSpace(step);
  return result.outcome === "applied" || result.outcome === "deferred";
}

const controls = new WeakMap<HTMLElement, Control>();

const translateX = (node: HTMLElement) => {
  const value = getComputedStyle(node).translate;
  return value === "none" ? 0 : Number.parseFloat(value) || 0;
};

/**
 * A two-finger sideways swipe over the list moves to the neighbouring space:
 * the list follows the fingers, leaves when it commits and the next space
 * arrives from the other side; a short one springs back. See SwipeTracker for
 * what counts as a swipe. A timer runs only while the fingers are down, and a
 * second only while a committed list waits for its space.
 */
export function spaceSwipe(node: HTMLElement, options: Options) {
  let current = options;
  const tracker = new SwipeTracker({
    extent: () => node.clientWidth || 240,
    canStep: (step) => current.canStep(step),
  });
  let idle: ReturnType<typeof setTimeout> | undefined;
  let waiting: ReturnType<typeof setTimeout> | undefined;
  let heard = 0;
  let base = 0;
  let settling: Animation | undefined;
  let leaving: Animation | undefined;
  let leftBy: SwipeStep = 1;

  const extent = () => node.clientWidth || 240;
  const place = (offset: number) => {
    node.style.translate = offset === 0 ? "" : `${offset}px 0`;
  };

  function release() {
    node.style.willChange = "";
    node.style.translate = "";
  }

  function spring(from: number) {
    settling?.cancel();
    release();
    if (from === 0 || typeof node.animate !== "function") return;
    settling = node.animate([{ translate: `${from}px 0` }, { translate: "0 0" }], {
      duration: duration("slow"),
      easing: easing("spring"),
    });
  }

  function leave(step: SwipeStep, from: number, velocity: number) {
    release();
    leftBy = step;
    if (reducedMotion() || typeof node.animate !== "function") return;
    const to = (step === 1 ? -1 : 1) * extent() * REACH;
    const travel = Math.abs(to - from);
    const ms = Math.min(190, Math.max(90, travel / Math.max(Math.abs(velocity), 0.9)));
    leaving = node.animate(
      [
        { translate: `${from}px 0`, opacity: 1 },
        { translate: `${to}px 0`, opacity: 0 },
      ],
      { duration: ms, easing: easing("out"), fill: "forwards" },
    );
  }

  function recover() {
    clearTimeout(waiting);
    waiting = undefined;
    if (!leaving) return;
    const from = (leftBy === 1 ? -1 : 1) * extent() * REACH;
    leaving.cancel();
    leaving = undefined;
    if (typeof node.animate !== "function") return;
    settling = node.animate(
      [
        { translate: `${from}px 0`, opacity: 0 },
        { translate: "0 0", opacity: 1 },
      ],
      { duration: duration("slow"), easing: easing("spring") },
    );
  }

  function commit(reading: Extract<Reading, { kind: "commit" }>) {
    clearTimeout(idle);
    idle = undefined;
    haptic("level");
    leave(reading.step, base + reading.offset, reading.velocity);
    base = 0;
    if (leaving) waiting = setTimeout(recover, ARRIVAL);
    void Promise.resolve(current.step(reading.step)).then((accepted) => {
      if (accepted === false) recover();
    });
  }

  function apply(reading: Reading) {
    if (reading.kind === "track") {
      if (reducedMotion()) return;
      settling?.cancel();
      settling = undefined;
      node.style.willChange = "translate";
      place(base + reading.offset);
    } else if (reading.kind === "commit") commit(reading);
    else if (reading.kind === "cancel") {
      if (!reducedMotion()) spring(base + reading.offset);
      else release();
      base = 0;
    }
  }

  function watch() {
    if (idle !== undefined || !tracker.tracking) return;
    idle = setTimeout(function check() {
      const quiet = performance.now() - heard;
      if (quiet < IDLE) {
        idle = setTimeout(check, IDLE - quiet);
        return;
      }
      idle = undefined;
      apply(tracker.idle());
    }, IDLE);
  }

  function onwheel(event: Wheel) {
    if (event.ctrlKey || event.deltaMode !== 0 || leaving) return;
    heard = performance.now();
    const natural = event.webkitDirectionInvertedFromDevice !== false;
    const fingers = natural ? -event.deltaX : event.deltaX;
    if (!tracker.tracking) base = settling ? translateX(node) : 0;
    apply(tracker.wheel(fingers, event.deltaY, heard));
    watch();
  }

  controls.set(node, {
    arrive(side) {
      clearTimeout(waiting);
      waiting = undefined;
      const wasLeaving = leaving !== undefined;
      leaving?.cancel();
      leaving = undefined;
      if (reducedMotion() || typeof node.animate !== "function") return;
      const from = wasLeaving ? side * extent() * REACH : side * 28;
      settling?.cancel();
      settling = node.animate(
        [
          { translate: `${from}px 0`, opacity: 0 },
          { translate: "0 0", opacity: 1 },
        ],
        { duration: duration(wasLeaving ? "slow" : "base"), easing: easing("spring") },
      );
    },
  });

  node.addEventListener("wheel", onwheel, { passive: true });
  return {
    update(next: Options) {
      current = next;
    },
    destroy() {
      clearTimeout(idle);
      clearTimeout(waiting);
      settling?.cancel();
      leaving?.cancel();
      release();
      tracker.reset();
      controls.delete(node);
      node.removeEventListener("wheel", onwheel);
    },
  };
}

/** The list arriving from the side its space stands on: from where a swipe
 *  sent the last one, or a short slide when something else moved the window. */
export function arrive(node: HTMLElement | undefined, side: SwipeStep) {
  if (!node) return;
  controls.get(node)?.arrive(side);
}
