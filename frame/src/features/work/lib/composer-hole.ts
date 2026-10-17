const n = (value: number) => Math.round(value * 100) / 100;

/**
 * A clip path for the canvas card with a hole the shape of the composer: round
 * at the top, and where its walls meet the card's foot the card turns up into
 * them on a fillet, as the new tab's notch does at the window's edge.
 */
export function composerHole(
  card: { width: number; height: number },
  bar: { left: number; top: number; width: number },
  radius: number,
  flare: number,
): string {
  const { left: x, top: y, width: w } = bar;
  const r = Math.max(0, Math.min(radius, w / 2));
  const foot = card.height;
  const f = Math.max(0, Math.min(flare, foot - y - r, x, card.width - x - w));
  const right = x + w;
  return (
    `path("M0 0H${n(card.width)}V${n(foot)}H${n(right + f)}A${n(f)} ${n(f)} 0 0 1 ${n(right)} ${n(foot - f)}` +
    `V${n(y + r)}A${n(r)} ${n(r)} 0 0 0 ${n(right - r)} ${n(y)}H${n(x + r)}` +
    `A${n(r)} ${n(r)} 0 0 0 ${n(x)} ${n(y + r)}V${n(foot - f)}A${n(f)} ${n(f)} 0 0 1 ${n(x - f)} ${n(foot)}H0Z")`
  );
}
