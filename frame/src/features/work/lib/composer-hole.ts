/**
 * A clip path for the canvas card with a hole the shape of the composer: round
 * at the top, running out through the card's foot. Even-odd, so the second
 * outline is the hole.
 */
export function composerHole(
  card: { width: number; height: number },
  bar: { left: number; top: number; width: number },
  radius: number,
): string {
  const r = Math.max(0, Math.min(radius, bar.width / 2));
  const { left: x, top: y, width: w } = bar;
  const foot = card.height + 16;
  const outer = `M0 0H${card.width}V${foot}H0Z`;
  const inner =
    `M${x} ${foot}V${y + r}A${r} ${r} 0 0 1 ${x + r} ${y}H${x + w - r}` +
    `A${r} ${r} 0 0 1 ${x + w} ${y + r}V${foot}Z`;
  return `path(evenodd, "${outer}${inner}")`;
}
