import { expect, test } from "vitest";
import type { PageAssistChangedV1 } from "$shared/ipc/bindings";
import { EMPTY, admit, reconcile, working, type Thread, type TurnView } from "../lib/thread-model";
import { asEntries, turn } from "./fixtures";

const event = (
  turnId: number,
  seq: number,
  kind: PageAssistChangedV1["kind"],
): PageAssistChangedV1 => ({ tab: "t", turn: turnId, seq, kind });
const only = (thread: Thread) => thread.items[0] as TurnView;

test("events build a turn in the core's order and a replay changes nothing", () => {
  const part = {
    kind: "answer" as const,
    text: "Yes",
    label: null,
    cites: [{ n: 1, excerpt: "p" }],
  };
  let thread = admit(EMPTY, event(1, 1, { type: "status", phase: { phase: "reading" } }));
  thread = admit(thread, event(1, 2, { type: "part", part }));
  const again = admit(thread, event(1, 2, { type: "part", part }));
  expect(again).toBe(thread);
  expect(only(thread).parts).toHaveLength(1);
  expect(working(thread)).toBe(true);
  thread = admit(thread, event(1, 3, { type: "done", cost: null }));
  expect(only(thread).state).toBe("done");
  expect(working(thread)).toBe(false);
});

test("an event older than the turn has applied is dropped", () => {
  let thread = admit(EMPTY, event(1, 5, { type: "cancelled" }));
  thread = admit(thread, event(1, 4, { type: "status", phase: { phase: "answering" } }));
  expect(only(thread).state).toBe("cancelled");
  expect(only(thread).phase).toBeNull();
});

test("a step replaces the one with its index and a confirm makes the turn wait", () => {
  const step = (state: "running" | "confirm" | "done") => ({
    type: "step" as const,
    step: { index: 0, label: "Place order", category: "purchase" as const, state },
  });
  let thread = admit(EMPTY, event(1, 1, step("running")));
  thread = admit(thread, event(1, 2, step("confirm")));
  expect(only(thread).steps).toHaveLength(1);
  expect(only(thread).state).toBe("waiting");
  thread = admit(thread, event(1, 3, step("done")));
  expect(only(thread).state).toBe("working");
});

test("a later cost settles the earlier one", () => {
  let thread = admit(
    EMPTY,
    event(1, 1, { type: "done", cost: { kind: "hosted", model: "Luna", credits: null } }),
  );
  thread = admit(
    thread,
    event(1, 2, { type: "cost", cost: { kind: "hosted", model: "Luna", credits: "0.3" } }),
  );
  expect(only(thread).cost).toEqual({ kind: "hosted", model: "Luna", credits: "0.3" });
});

test("invalidated makes every source inert", () => {
  const thread = admit(
    reconcile(EMPTY, asEntries(turn(1), turn(2))),
    event(2, 99, { type: "invalidated" }),
  );
  expect(thread.items.every((item) => item.type === "turn" && item.inert)).toBe(true);
});

test("a snapshot never rolls back what live events already showed", () => {
  const live = admit(EMPTY, event(1, 8, { type: "done", cost: null }));
  const merged = reconcile(
    live,
    asEntries(turn(1, { question: "Why?", state: "working", seq: 3 })),
  );
  expect(only(merged).state).toBe("done");
  expect(only(merged).question).toBe("Why?");
});

test("dividers come from the snapshot and stay between turns", () => {
  const thread = reconcile(EMPTY, [
    ...asEntries(turn(1)),
    { kind: "divider", site: "b.com", cross_site: true },
    ...asEntries(turn(2)),
  ]);
  expect(thread.items.map((item) => item.type)).toEqual(["turn", "divider", "turn"]);
});

test("started opens a turn with its question, and a divider ends every source before it", () => {
  let thread = admit(
    EMPTY,
    event(1, 1, { type: "started", turn: turn(1, { question: "Why?", state: "working", seq: 1 }) }),
  );
  expect(only(thread).question).toBe("Why?");
  thread = admit(thread, event(1, 2, { type: "done", cost: null }));
  thread = admit(thread, event(1, 3, { type: "divider", site: "b.com", cross_site: true }));
  expect(thread.items.map((item) => item.type)).toEqual(["turn", "divider"]);
  expect(only(thread).inert).toBe(true);
  expect(admit(thread, event(1, 3, { type: "divider", site: "b.com", cross_site: true }))).toBe(
    thread,
  );
});
