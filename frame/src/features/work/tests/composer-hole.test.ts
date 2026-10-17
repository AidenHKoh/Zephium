import { expect, test } from "vitest";
import { composerHole } from "../lib/composer-hole";

test("the hole follows the composer's width and runs out through the card's foot", () => {
  const card = { width: 1000, height: 700 };
  expect(composerHole(card, { left: 150, top: 620, width: 700 }, 22)).toBe(
    'path(evenodd, "M0 0H1000V716H0ZM150 716V642A22 22 0 0 1 172 620H828A22 22 0 0 1 850 642V716Z")',
  );
});

test("a corner is never larger than half the composer", () => {
  const path = composerHole({ width: 300, height: 300 }, { left: 10, top: 200, width: 20 }, 22);
  expect(path).toContain("A10 10 0 0 1 20 200H20A10 10 0 0 1 30 210");
});
