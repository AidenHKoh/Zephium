<script lang="ts">
  import Icon from "$shared/ui/Icon";
  import type { ObjectActions, PlanView } from "../../lib/board/types";
  import { CALENDAR, firstWeekday, type CalendarDay } from "../../lib/board/plan-calendar";
  import { Tick02Icon } from "./icons";
  /** A dated plan as its weeks: each day holds its steps, a fixed cell that never grows. */
  let {
    object,
    weeks,
    actions = {},
  }: {
    object: PlanView;
    weeks: CalendarDay[][];
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
  style:--row={`${CALENDAR.row}px`}
  style:--head={`${CALENDAR.head}px`}
>
  <div class="weekdays" role="row">
    {#each weekdays as weekday (weekday)}<span role="columnheader">{weekday}</span>{/each}
  </div>
  <div class="days">
    {#each weeks as week, row (row)}<div class="week" role="row">
        {#each week as day (day.date)}<div
            class="day"
            class:outside={day.outside}
            class:today={day.today}
            role="gridcell"
            aria-label={spoken(day)}
          >
            <span class="number" class:opens={day.opens}>{label(day)}</span>
            {#if day.steps.length}<ul
                style:--lines={day.steps.length >= SHOWN ? 1 : day.steps.length === 2 ? 2 : 3}
              >
                {#each day.steps.slice(0, SHOWN) as index (index)}
                  {@const step = object.steps[index]!}
                  <li
                    class="chip {step.kind}"
                    class:done={step.done}
                    title={step.detail ? `${step.title} — ${step.detail}` : step.title}
                  >
                    {#if object.checkable && actions.check}<button
                        type="button"
                        class="step nodrag nopan"
                        aria-pressed={!!step.done}
                        onclick={() => actions.check?.(object.id, index, !step.done)}
                        >{#if step.done}<Icon icon={Tick02Icon} size={11} />{/if}<span
                          >{step.title}</span
                        ></button
                      >{:else}<span class="step"
                        >{#if step.done}<Icon icon={Tick02Icon} size={11} />{/if}<span
                          >{step.title}</span
                        ></span
                      >{/if}
                  </li>
                {/each}
              </ul>
              {#if day.steps.length > SHOWN}<span class="more">+{day.steps.length - SHOWN}</span
                >{/if}
            {/if}
          </div>{/each}
      </div>{/each}
  </div>
</div>

<style>
  /* A month view: one frame, hairlines between the days, the week's names on top. */
  .calendar {
    display: flex;
    flex-direction: column;
    inline-size: max-content;
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--color-surface);
    box-shadow:
      0 0 0 1px var(--color-border),
      var(--shadow-raised);
  }

  .weekdays,
  .week {
    display: grid;
    grid-template-columns: repeat(7, var(--cell));
    gap: 1px;
  }

  .weekdays {
    align-items: center;
    block-size: var(--head);
    border-block-end: 1px solid var(--color-border);
    color: var(--color-muted);
    font-size: var(--text-caption);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .weekdays span {
    padding-inline: 10px;
  }

  .days {
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: var(--color-border);
  }

  .day {
    display: flex;
    flex-direction: column;
    gap: 5px;
    box-sizing: border-box;
    block-size: var(--row);
    min-inline-size: 0;
    padding: 7px 7px 6px;
    overflow: hidden;
    background: var(--color-surface);
    transition: background-color var(--motion-fast) var(--ease-out);
  }

  .day:hover {
    background: var(--color-control-hover);
  }

  .number {
    align-self: flex-start;
    min-inline-size: 20px;
    padding: 0 6px;
    border-radius: var(--radius-capsule);
    box-sizing: border-box;
    color: var(--color-label-secondary);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
    font-weight: 600;
    line-height: 20px;
    text-align: center;
  }

  .number.opens {
    color: var(--color-text);
  }

  .outside .number {
    color: var(--color-faint);
  }

  .today .number {
    background: var(--color-accent);
    color: var(--color-on-accent);
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-inline-size: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* An event as a calendar draws it: a tinted chip with its kind's edge. */
  .chip {
    --tone: var(--color-muted);

    border-radius: 5px;
    background: color-mix(in srgb, var(--tone) 14%, transparent);
    box-shadow: inset 2px 0 0 var(--tone);
  }

  .chip.milestone {
    --tone: var(--color-warning);
  }

  .chip.travel {
    --tone: var(--color-info);
  }

  .chip.stay {
    --tone: var(--color-success);
  }

  .chip.event {
    --tone: var(--color-info);
  }

  .step {
    display: flex;
    align-items: flex-start;
    gap: 4px;
    box-sizing: border-box;
    inline-size: 100%;
    min-inline-size: 0;
    padding: 2px 6px 2px 7px;
    border: 0;
    border-radius: inherit;
    background: none;
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-caption);
    font-weight: 500;
    line-height: 14px;
    text-align: start;
    cursor: default;
  }

  .step :global(svg) {
    flex: none;
    margin-block-start: 1px;
    color: var(--color-success);
  }

  /* A day's few events share its room: one reads in three lines, three in one each. */
  .step span {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: var(--lines);
    line-clamp: var(--lines);
    overflow-wrap: anywhere;
  }

  button.step:hover {
    background: color-mix(in srgb, var(--tone) 12%, transparent);
  }

  button.step:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: 1px;
  }

  .done .step span {
    color: var(--color-muted);
    text-decoration: line-through;
  }

  .more {
    padding-inline: 6px;
    color: var(--color-muted);
    font-size: var(--text-caption);
    font-weight: 500;
  }
</style>
