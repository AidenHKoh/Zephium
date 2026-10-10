<script lang="ts">
  import Icon from "$shared/ui/Icon";
  import type { IconSvgElement } from "@hugeicons/svelte";
  import type { ObjectActions, PlanView } from "../../lib/board/types";
  import { CALENDAR, firstWeekday, type CalendarDay } from "../../lib/board/plan-calendar";
  import { Tick02Icon } from "./icons";
  /** A dated plan as its weeks: each day holds its steps, a fixed cell that never grows. */
  let {
    object,
    weeks,
    glyphs,
    actions = {},
  }: {
    object: PlanView;
    weeks: CalendarDay[][];
    glyphs: Record<PlanView["steps"][number]["kind"], IconSvgElement>;
    actions?: ObjectActions;
  } = $props();
  /** Steps a day shows before it counts the rest. */
  const SHOWN = 3;
  const weekdays = $derived.by(() => {
    const first = firstWeekday();
    const format = new Intl.DateTimeFormat(undefined, { weekday: "short" });
    // 2024-01-01 was a Monday: day 1 of the week counting from it.
    return Array.from({ length: 7 }, (_, index) =>
      format.format(new Date(2024, 0, ((first - 1 + index) % 7) + 1)),
    );
  });
  const monthDay = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
  const label = (day: CalendarDay) => {
    if (!day.opens) return String(day.day);
    const [year, month, date] = day.date.split("-").map(Number);
    return monthDay.format(new Date(year!, month! - 1, date!));
  };
  const longDate = new Intl.DateTimeFormat(undefined, { dateStyle: "full" });
  const spoken = (day: CalendarDay) => {
    const [year, month, date] = day.date.split("-").map(Number);
    return longDate.format(new Date(year!, month! - 1, date!));
  };
</script>

<div
  class="calendar"
  role="grid"
  aria-label={object.title}
  style:--cell={`${CALENDAR.cell}px`}
  style:--gap={`${CALENDAR.gap}px`}
  style:--row={`${CALENDAR.row}px`}
  style:--head={`${CALENDAR.head}px`}
>
  <div class="weekdays" role="row">
    {#each weekdays as weekday (weekday)}<span role="columnheader">{weekday}</span>{/each}
  </div>
  {#each weeks as week, row (row)}<div class="week" role="row">
      {#each week as day (day.date)}<div
          class="day"
          class:outside={day.outside}
          class:busy={day.steps.length > 0}
          role="gridcell"
          aria-label={spoken(day)}
        >
          <span class="number" class:today={day.today} class:opens={day.opens}>{label(day)}</span>
          {#if day.steps.length}<ul
              style:--lines={day.steps.length >= SHOWN ? 1 : day.steps.length === 2 ? 2 : 3}
            >
              {#each day.steps.slice(0, SHOWN) as index (index)}
                {@const step = object.steps[index]!}
                <li
                  class:done={step.done}
                  title={step.detail ? `${step.title} — ${step.detail}` : step.title}
                >
                  {#if object.checkable && actions.check}<button
                      type="button"
                      class="step nodrag nopan"
                      aria-pressed={!!step.done}
                      onclick={() => actions.check?.(object.id, index, !step.done)}
                      ><Icon icon={step.done ? Tick02Icon : glyphs[step.kind]} size={11} /><span
                        >{step.title}</span
                      ></button
                    >{:else}<span class="step"
                      ><Icon icon={step.done ? Tick02Icon : glyphs[step.kind]} size={11} /><span
                        >{step.title}</span
                      ></span
                    >{/if}
                </li>
              {/each}
            </ul>
            {#if day.steps.length > SHOWN}<span class="more">+{day.steps.length - SHOWN}</span>{/if}
          {/if}
        </div>{/each}
    </div>{/each}
</div>

<style>
  .calendar {
    display: flex;
    flex-direction: column;
    gap: var(--gap);
  }

  .weekdays,
  .week {
    display: grid;
    grid-template-columns: repeat(7, var(--cell));
    gap: var(--gap);
  }

  .weekdays {
    block-size: calc(var(--head) - var(--gap));
    align-items: end;
    color: var(--color-muted);
    font-size: var(--text-caption);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .weekdays span {
    padding-inline: 8px;
  }

  .day {
    display: flex;
    flex-direction: column;
    gap: 4px;
    box-sizing: border-box;
    block-size: var(--row);
    min-inline-size: 0;
    padding: 7px 7px 6px;
    overflow: hidden;
    border-radius: var(--radius-inset);
    background: var(--color-fill);
  }

  .day.busy {
    background: var(--color-surface);
    box-shadow: var(--shadow-raised);
  }

  .day.outside {
    background: transparent;
  }

  .number {
    align-self: flex-start;
    padding: 0 5px;
    border-radius: var(--radius-capsule);
    color: var(--color-muted);
    font-size: var(--text-caption);
    font-variant-numeric: tabular-nums;
    font-weight: 600;
    line-height: 17px;
  }

  .number.opens {
    color: var(--color-text);
  }

  .outside .number {
    color: var(--color-faint);
  }

  .number.today {
    background: var(--color-accent);
    color: var(--color-on-accent);
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-inline-size: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .step {
    display: flex;
    align-items: flex-start;
    gap: 5px;
    inline-size: 100%;
    min-inline-size: 0;
    padding: 1px 2px;
    border: 0;
    border-radius: var(--radius-control-compact);
    background: none;
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-caption);
    line-height: 15px;
    text-align: start;
    cursor: default;
  }

  .step :global(svg) {
    flex: none;
    margin-block-start: 2px;
    color: var(--color-muted);
  }

  /* A day's few steps share its room: one reads in three lines, three in one each. */
  .step span {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: var(--lines);
    line-clamp: var(--lines);
    overflow-wrap: anywhere;
  }

  button.step:hover {
    background: var(--color-control-hover);
  }

  .done .step span {
    color: var(--color-muted);
    text-decoration: line-through;
  }

  .more {
    padding-inline: 4px;
    color: var(--color-muted);
    font-size: var(--text-caption);
  }
</style>
