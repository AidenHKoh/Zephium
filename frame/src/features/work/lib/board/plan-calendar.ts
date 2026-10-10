import type { PlanView } from "./types";

export type CalendarDay = {
  /** `YYYY-MM-DD`. */
  date: string;
  day: number;
  /** The first day shown of its month: it names the month. */
  opens: boolean;
  /** Before the plan's first day or after its last. */
  outside: boolean;
  today: boolean;
  /** Indices of the plan's steps on this day, in plan order. */
  steps: number[];
};

/** A plan of this many dated steps over at least this many days reads as a calendar. */
const LEAST_STEPS = 6;
const LEAST_DAYS = 10;
/** Longer than this, the calendar would outgrow a result. */
const MOST_WEEKS = 10;

const iso = (date: Date) =>
  `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
const parse = (value: string) => {
  const [year, month, day] = value.split("-").map(Number);
  return new Date(year!, month! - 1, day!);
};
const DAY_MS = 86_400_000;

/** 1 for Monday, 7 for Sunday: the locale's first day of the week. */
export function firstWeekday(locale?: string): number {
  try {
    const info = new Intl.Locale(locale ?? navigator.language) as Intl.Locale & {
      getWeekInfo?: () => { firstDay: number };
      weekInfo?: { firstDay: number };
    };
    return info.getWeekInfo?.().firstDay ?? info.weekInfo?.firstDay ?? 1;
  } catch {
    return 1;
  }
}

/**
 * A dated plan as weeks of days, or null when it reads better as a timeline:
 * too few dated steps, too short a span, or too long to stand on the canvas.
 */
export function planWeeks(
  plan: PlanView,
  options: { today?: Date; firstDay?: number } = {},
): CalendarDay[][] | null {
  const dated = plan.steps.flatMap((step, index) =>
    step.date ? [{ index, date: step.date }] : [],
  );
  if (dated.length < LEAST_STEPS || dated.length < plan.steps.length * 0.8) return null;
  const days = dated.map(({ date }) => parse(date).getTime());
  const first = new Date(Math.min(...days));
  const last = new Date(Math.max(...days));
  if (Math.round((last.getTime() - first.getTime()) / DAY_MS) + 1 < LEAST_DAYS) return null;
  const weekStart = (options.firstDay ?? firstWeekday()) % 7;
  const start = new Date(first);
  start.setDate(start.getDate() - ((start.getDay() - weekStart + 7) % 7));
  const end = new Date(last);
  end.setDate(end.getDate() + ((weekStart + 6 - end.getDay() + 7) % 7));
  const span = Math.round((end.getTime() - start.getTime()) / DAY_MS) + 1;
  if (span / 7 > MOST_WEEKS) return null;
  const byDay = new Map<string, number[]>();
  for (const { index, date } of dated) byDay.set(date, [...(byDay.get(date) ?? []), index]);
  const today = iso(options.today ?? new Date());
  const firstIso = iso(first);
  const lastIso = iso(last);
  const weeks: CalendarDay[][] = [];
  for (let offset = 0; offset < span; offset++) {
    const day = new Date(start.getFullYear(), start.getMonth(), start.getDate() + offset);
    const date = iso(day);
    if (offset % 7 === 0) weeks.push([]);
    weeks.at(-1)!.push({
      date,
      day: day.getDate(),
      opens: offset === 0 || day.getDate() === 1,
      outside: date < firstIso || date > lastIso,
      today: date === today,
      steps: byDay.get(date) ?? [],
    });
  }
  return weeks;
}

/** The calendar's size on the canvas: seven columns of days, a row a week. */
export const CALENDAR = { cell: 128, row: 100, head: 34 } as const;
/** Seven days, the hairlines between them and the frame's edge. */
export const calendarWidth = () => 7 * CALENDAR.cell + 6 + 2;
export const calendarHeight = (weeks: number) =>
  CALENDAR.head + 1 + weeks * CALENDAR.row + (weeks - 1) + 2;
