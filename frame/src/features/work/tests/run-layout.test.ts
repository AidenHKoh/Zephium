import { expect, test } from "vitest";
import { boardLayout, type LayoutBlock } from "../lib/board/layout";
import { RUN, placeRun, type RunInputs } from "../lib/run/layout";

const row = (id: string, height: number, phase?: number) => ({
  part: { id, width: 240, height },
  found: [],
  feeds: true,
  ...(phase === undefined ? {} : { phase }),
});
const run = (rows: RunInputs["rows"], result: number): RunInputs => ({
  request: { id: "request", width: RUN.request, height: 80 },
  rows,
  head: { id: "head", width: 640, height: 40 },
  board: { width: 640, height: result, at: { body: { x: 0, y: 0, width: 640, height: result } } },
});

// The run's top: the request, the first row of every column and the result's headline share it.
function topped(place: ReturnType<typeof placeRun>, ids: string[]) {
  expect(place.rects["request"]!.y).toBe(100);
  expect(place.corner.y).toBe(place.rects[ids[0]!]!.y + RUN.labelMid - RUN.resultMid);
  for (const id of ids) expect(place.rects[id]!.y).toBe(place.rects[ids[0]!]!.y);
}

test("a run with one column begins at its top and its result starts level with the first row", () => {
  const place = placeRun(100, run([row("a", 120), row("b", 120)], 300));
  topped(place, ["a"]);
  expect(place.rects["head"]!.y).toBe(place.corner.y);
});

test("a tall left side does not move the request or the result down", () => {
  const short = placeRun(100, run([row("a", 120)], 100));
  const tall = placeRun(
    100,
    run([row("a", 400), row("b", 400), row("c", 400), row("d", 120, 1), row("e", 120, 1)], 100),
  );
  topped(tall, ["a", "d"]);
  expect(tall.corner.y).toBe(short.corner.y);
  expect(tall.rects["request"]!.y).toBe(short.rects["request"]!.y);
  // Later steps stand further right.
  expect(tall.rects["d"]!.x).toBeGreaterThan(tall.rects["a"]!.x + 240);
  expect(tall.corner.x).toBeGreaterThan(tall.rects["d"]!.x + 240);
});

test("a tall result grows down and leaves the left side where it was", () => {
  const short = placeRun(100, run([row("a", 120), row("b", 120)], 100));
  const tall = placeRun(100, run([row("a", 120), row("b", 120)], 1800));
  expect(tall.rects["a"]).toEqual(short.rects["a"]);
  expect(tall.rects["b"]).toEqual(short.rects["b"]);
  expect(tall.rects["request"]).toEqual(short.rects["request"]);
  expect(tall.corner.y).toBe(short.corner.y);
  expect(tall.extent).toBeGreaterThan(short.extent + 1500);
});

const block = (id: string, kind: string, ideal: number, max = ideal): LayoutBlock => ({
  id,
  kind,
  emphasis: id === "wide" ? "hero" : "primary",
  width: { min: Math.min(ideal, 480), ideal, max },
  height: () => 200,
});

test("a wide object stands at the result's left edge at the width it draws at", () => {
  const narrow = boardLayout([block("wide", "diagram", 520), block("text", "prose", 640, 680)]);
  expect(narrow.at["wide"]).toMatchObject({ x: 0, width: 520 });
  expect(narrow.at["text"]!.x).toBe(0);
  const wide = boardLayout([block("wide", "diagram", 1500), block("text", "prose", 640, 680)]);
  expect(wide.at["wide"]).toMatchObject({ x: 0, width: 1500 });
  expect(wide.width).toBe(1500);
});
