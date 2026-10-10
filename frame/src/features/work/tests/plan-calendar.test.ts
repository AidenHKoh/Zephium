import { describe, expect, it } from "vitest";
import { planWeeks } from "../lib/board/plan-calendar";
import type { PlanView } from "../lib/board/types";

const plan = (dates: (string | undefined)[]): PlanView => ({
  id: "p",
  kind: "plan",
  checkable: false,
  steps: dates.map((date, index) => ({
    title: `Step ${index}`,
    kind: "task",
    ...(date ? { date } : {}),
  })),
});
const days = (from: number, count: number) =>
  Array.from({ length: count }, (_, i) => `2026-10-${String(from + i).padStart(2, "0")}`);

describe("plan calendar", () => {
  it("lays a month of dated steps out in whole weeks from the locale's first day", () => {
    const weeks = planWeeks(plan(days(9, 20)), { firstDay: 1, today: new Date(2026, 9, 10) })!;
    expect(weeks[0]![0]!.date).toBe("2026-10-05");
    expect(weeks.every((week) => week.length === 7)).toBe(true);
    expect(weeks.at(-1)!.at(-1)!.date).toBe("2026-11-01");
    const tenth = weeks.flat().find((day) => day.date === "2026-10-10")!;
    expect(tenth.today).toBe(true);
    expect(tenth.steps).toEqual([1]);
    expect(weeks.flat().find((day) => day.date === "2026-10-05")!.outside).toBe(true);
    expect(weeks.flat().find((day) => day.date === "2026-11-01")!.opens).toBe(true);
  });

  it("keeps short, sparse or undated plans as a timeline", () => {
    expect(planWeeks(plan(days(9, 4)))).toBeNull();
    expect(planWeeks(plan(days(9, 7)))).toBeNull();
    expect(planWeeks(plan([...days(9, 12), ...Array(6).fill(undefined)]))).toBeNull();
  });
});
