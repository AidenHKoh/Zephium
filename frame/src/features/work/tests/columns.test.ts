import { expect, test } from "vitest";
import { columnsOf } from "../lib/run/columns";

test("work that overlapped in time stands in one column, whatever turn began it", () => {
  const columns = columnsOf([
    { turn: 1, began: 0, ended: 100 },
    { turn: 2, began: 1000, ended: 5000 },
    { turn: 3, began: 1200, ended: 3000 },
    { turn: 4, began: 1300, ended: 9000 },
  ]);
  expect(columns).toEqual([0, 1, 1, 1]);
});

test("a new column begins only with work that started after the column had ended", () => {
  expect(
    columnsOf([
      { began: 0, ended: 10 },
      { began: 5, ended: 20 },
      { began: 20, ended: 30 },
      { began: 25, ended: 26 },
    ]),
  ).toEqual([0, 0, 1, 1]);
});

test("work that still runs overlaps everything that starts after it", () => {
  expect(
    columnsOf([
      { began: 0, open: true },
      { began: 50, ended: 60 },
      { began: 70, open: true },
    ]),
  ).toEqual([0, 0, 0]);
});

test("work without times follows the lead's turns", () => {
  expect(columnsOf([{ turn: 3 }, { turn: 1 }, { turn: 2 }, { turn: 1 }])).toEqual([2, 0, 1, 0]);
});

test("untimed work stands beside timed work of its own turn", () => {
  expect(
    columnsOf([
      { turn: 1 },
      { turn: 2, began: 0, ended: 10 },
      { turn: 2 },
      { turn: 3, began: 20, ended: 30 },
    ]),
  ).toEqual([0, 1, 1, 2]);
});

test("a part not yet placed joins the newest column, never the first", () => {
  expect(columnsOf([{ turn: 1 }, { turn: 2 }, {}])).toEqual([0, 1, 1]);
  expect(columnsOf([{ began: 0, ended: 5 }, { began: 10, ended: 15 }, { open: true }])).toEqual([
    0, 1, 1,
  ]);
  expect(columnsOf([{}])).toEqual([0]);
});
