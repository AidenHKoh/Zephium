import type { Notch } from "./ground";
import { WORDMARK } from "$shared/lib/wordmark";

/** A box in the page's own pixels, on whole pixels. */
export type Box = { x: number; y: number; w: number; h: number };

export type Layout = {
  /** The field's notch, narrowed so both top corners stay free. */
  notch: Notch;
  /** The page's controls, each in its own round pane (a capsule when it
   *  holds words), in the top corner beside the notch and level with the
   *  field, the last nearest the corner. */
  controls: Box[];
  mark: Box;
  /** Where the line with the time stands, or null without one. */
  when: number | null;
  /** The day's figures, in a row along the foot of the page. */
  tiles: Box[];
};

const NOTCH: Notch = { width: 640, height: 50, radius: 18, flare: 14 };
const CORNER = 110;
const CONTROL = { size: 32, gap: 6, inset: 14 };
/** What two round controls take from the corner, which CORNER was set for. */
const TWO_ROUND = CONTROL.inset + 2 * CONTROL.size + CONTROL.gap;
const TILE = { width: 200, height: 80, gap: 12, foot: 28 };
const GUTTER = 40;
const WHEN = { gap: 26, height: 18 };

/** Everything on the page laid out from its size alone, so the ground that
 *  is cut and the things that stand in the cuts come from the same numbers
 *  in the same frame, and a resize can never draw one ahead of the other. */
export function layout(
  width: number,
  height: number,
  { controls, clock, tiles }: { controls: readonly number[]; clock: boolean; tiles: number },
): Layout {
  // Both top corners stay clear of the notch's flare for the controls in
  // one of them, so the field stays in the middle however wide they are.
  const reach = controls.reduce((sum, w) => sum + w + CONTROL.gap, CONTROL.inset - CONTROL.gap);
  const corner = Math.max(CORNER, reach + CORNER - TWO_ROUND);
  const notch = { ...NOTCH, width: Math.min(NOTCH.width, width - 2 * corner) };

  const buttonY = Math.round((NOTCH.height - CONTROL.size) / 2);
  let right = width - CONTROL.inset;
  const buttons = controls
    .toReversed()
    .map((w) => {
      const box = { x: right - w, y: buttonY, w, h: CONTROL.size };
      right = box.x - CONTROL.gap;
      return box;
    })
    .toReversed();

  const tileW = tiles
    ? Math.floor(Math.min(TILE.width, (width - 2 * GUTTER - (tiles - 1) * TILE.gap) / tiles))
    : 0;
  const span = tiles * tileW + (tiles - 1) * TILE.gap;
  const start = Math.round((width - span) / 2);
  const row = height - TILE.foot - TILE.height;
  // The name and the time stand in the middle of what is left between the
  // notch and the figures, a touch high of it, where the eye puts a middle.
  const floor = tiles ? row : height;
  const markW = Math.round(Math.min(440, Math.max(220, width * 0.34)));
  const markH = Math.ceil((markW * WORDMARK.height) / WORDMARK.width);
  const group = markH + (clock ? WHEN.gap + WHEN.height : 0);
  const centre = NOTCH.height + (floor - NOTCH.height) * 0.46;
  const top = Math.max(NOTCH.height + 40, Math.round(centre - group / 2));

  const mark = { x: Math.round((width - markW) / 2), y: top, w: markW, h: markH };
  return {
    notch,
    controls: buttons,
    mark,
    when: clock ? mark.y + markH + WHEN.gap : null,
    tiles: Array.from({ length: tiles }, (_, index) => ({
      x: start + index * (tileW + TILE.gap),
      y: row,
      w: tileW,
      h: TILE.height,
    })),
  };
}
