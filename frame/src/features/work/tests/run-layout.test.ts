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
    run(
      [row("a", 400, 0), row("b", 400, 0), row("c", 400, 0), row("d", 120, 1), row("e", 120, 1)],
      100,
    ),
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

test("a row whose column is not known yet stands in the newest column", () => {
  const rows = [row("a", 120, 0), row("b", 120, 1), row("c", 120)];
  const place = placeRun(100, run(rows, 100));
  expect(place.rects["c"]!.x).toBe(place.rects["b"]!.x);
});

type Box = { x: number; y: number; width: number; height: number };
// A segment crosses a rect when it passes through its interior.
const crosses = (a: { x: number; y: number }, b: { x: number; y: number }, r: Box) =>
  Math.min(a.x, b.x) < r.x + r.width &&
  Math.max(a.x, b.x) > r.x &&
  Math.min(a.y, b.y) < r.y + r.height &&
  Math.max(a.y, b.y) > r.y;

function crossings(place: ReturnType<typeof placeRun>) {
  const hits: string[] = [];
  for (const line of place.lines)
    for (let at = 1; at < line.points.length; at++)
      for (const [id, rect] of Object.entries(place.rects))
        if (crosses(line.points[at - 1]!, line.points[at]!, rect)) hits.push(`${line.id} x ${id}`);
  return hits;
}

const found = (id: string) => ({ id, width: 280, height: 140 });

test("no line crosses a card, in columns of parallel and sequential work", () => {
  const rows = [
    { ...row("a", 220, 0), found: [found("a1"), found("a2")] },
    row("b", 140, 0),
    row("c", 300, 1),
    { ...row("d", 160, 1), found: [found("d1")] },
    row("e", 120, 1),
    row("f", 200, 2),
  ];
  expect(crossings(placeRun(100, run(rows, 300)))).toEqual([]);
});

test("no line crosses a card when the last column feeds nothing", () => {
  const rows = [row("a", 220, 0), row("b", 140, 1), row("c", 200, 1)];
  const nothing = rows.map((entry) => ({ ...entry, feeds: entry.part.id === "a" }));
  expect(crossings(placeRun(100, run(nothing, 300)))).toEqual([]);
  expect(crossings(placeRun(100, run(rows, 300)))).toEqual([]);
});

test("no line crosses a card in a run with inputs, a single column or no parts", () => {
  const withInputs = {
    ...run([row("a", 120), row("b", 300)], 200),
    inputs: [
      { id: "i1", width: RUN.inputs, height: 60 },
      { id: "i2", width: RUN.inputs, height: 60 },
    ],
  };
  expect(crossings(placeRun(0, withInputs))).toEqual([]);
  expect(crossings(placeRun(0, run([], 200)))).toEqual([]);
});
