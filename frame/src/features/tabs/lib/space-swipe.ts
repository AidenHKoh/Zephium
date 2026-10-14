import { reducedMotion } from "$shared/lib/motion";

/** How far the fingers travel before the list gives way to the next space. */
const COMMIT = 72;
/** Wheel events stop arriving this long after the fingers lift. */
const IDLE = 140;

type Options = { canStep: (step: 1 | -1) => boolean; step: (step: 1 | -1) => void };

/**
 * A two-finger horizontal swipe over the tab list moves to the neighbouring
 * space. WebKit reports trackpad travel as wheel deltas with no phase, so a
 * gesture ends when they stop: one short timer, alive only during a swipe.
 */
export function spaceSwipe(node: HTMLElement, options: Options) {
  let current = options;
  let travel = 0;
  let swiping = false;
  let spent = false;
  let idle: ReturnType<typeof setTimeout> | undefined;

  const place = (offset: number, settle = false) => {
    node.style.transition = settle ? "translate var(--motion-base) var(--ease-spring)" : "";
    node.style.translate = offset === 0 ? "" : `${offset}px 0`;
  };

  function end() {
    idle = undefined;
    swiping = false;
    spent = false;
    travel = 0;
    place(0, !reducedMotion());
  }

  function onwheel(event: WheelEvent) {
    if (!swiping) {
      if (Math.abs(event.deltaX) <= Math.abs(event.deltaY) * 1.5 || event.deltaX === 0) return;
      swiping = true;
    }
    clearTimeout(idle);
    idle = setTimeout(end, IDLE);
    if (spent) return;
    travel += event.deltaX;
    const step: 1 | -1 = travel > 0 ? 1 : -1;
    const open = current.canStep(step);
    if (open && Math.abs(travel) >= COMMIT) {
      spent = true;
      place(0);
      current.step(step);
      return;
    }
    // The list follows the fingers, and only resists where there is no space.
    const reach = open ? Math.abs(travel) * 0.45 : Math.min(Math.abs(travel) * 0.12, 14);
    if (!reducedMotion()) place(-Math.sign(travel) * reach);
  }

  node.addEventListener("wheel", onwheel, { passive: true });
  return {
    update(next: Options) {
      current = next;
    },
    destroy() {
      clearTimeout(idle);
      node.removeEventListener("wheel", onwheel);
    },
  };
}

/** The list arriving from the side its space stands on. */
export function arrive(node: HTMLElement | undefined, from: 1 | -1) {
  if (!node || reducedMotion() || typeof node.animate !== "function") return;
  node.animate(
    [
      { translate: `${from * 28}px 0`, opacity: 0 },
      { translate: "0 0", opacity: 1 },
    ],
    { duration: 260, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)" },
  );
}
