import type {
  PageAssistChangedV1,
  PageAssistEntryV1,
  PageAssistStepV1,
  PageAssistTurnV1,
} from "$shared/ipc/bindings";

export type TurnView = PageAssistTurnV1 & { type: "turn" };

type DividerView = {
  type: "divider";
  key: string;
  site: string;
  /** A different site: the thread stays in view, the model starts over. */
  fresh: boolean;
};

type ThreadItem = TurnView | DividerView;

/** What the panel shows for one tab, rebuilt only from the core's snapshots and events. */
export type Thread = { items: readonly ThreadItem[] };

export const EMPTY: Thread = { items: [] };

const blank = (turn: number): TurnView => ({
  type: "turn",
  turn,
  intent: "ask",
  suggestion: null,
  question: "",
  selection: null,
  state: "working",
  phase: null,
  parts: [],
  steps: [],
  style: null,
  offer: null,
  cost: null,
  fault: null,
  inert: false,
  seq: 0,
});

/**
 * The core's snapshot, merged with what live events already showed: for each
 * turn the one that has applied more wins, so a snapshot read a moment late
 * never rolls an answer back.
 */
export function reconcile(thread: Thread, entries: readonly PageAssistEntryV1[]): Thread {
  const known = new Map<number, TurnView>();
  for (const item of thread.items) if (item.type === "turn") known.set(item.turn, item);
  const items = entries.map((entry, at): ThreadItem => {
    if (entry.kind === "divider")
      return { type: "divider", key: `d${at}`, site: entry.site, fresh: entry.cross_site };
    const fresh: TurnView = { ...entry.turn, type: "turn" };
    const live = known.get(entry.turn.turn);
    if (!live || live.seq <= fresh.seq) return fresh;
    return { ...live, question: live.question || fresh.question, inert: live.inert || fresh.inert };
  });
  // A turn the snapshot has not reached yet is kept, after what it has.
  for (const [turn, live] of known)
    if (!items.some((item) => item.type === "turn" && item.turn === turn)) items.push(live);
  return { items };
}

/** Admits one event, or returns the same thread when it was already seen. */
export function admit(thread: Thread, event: PageAssistChangedV1): Thread {
  const at = thread.items.findIndex((item) => item.type === "turn" && item.turn === event.turn);
  const current = at < 0 ? null : (thread.items[at] as TurnView);
  if (event.kind.type === "invalidated")
    return {
      items: thread.items.map((item) => (item.type === "turn" ? { ...item, inert: true } : item)),
    };
  if (current && event.seq <= current.seq) return thread;
  const next = apply({ ...(current ?? blank(event.turn)), seq: event.seq }, event.kind);
  const items = [...thread.items];
  if (at < 0) items.push(next);
  else items[at] = next;
  return { items };
}

const byIndex = (steps: PageAssistStepV1[], step: PageAssistStepV1) =>
  steps.some((row) => row.index === step.index)
    ? steps.map((row) => (row.index === step.index ? step : row))
    : [...steps, step].sort((a, b) => a.index - b.index);

function apply(turn: TurnView, kind: PageAssistChangedV1["kind"]): TurnView {
  switch (kind.type) {
    case "status":
      return { ...turn, phase: kind.phase };
    case "part":
      return { ...turn, parts: [...turn.parts, kind.part] };
    case "step":
      return {
        ...turn,
        state:
          kind.step.state === "confirm"
            ? "waiting"
            : turn.state === "waiting"
              ? "working"
              : turn.state,
        steps: byIndex(turn.steps, kind.step),
      };
    case "style":
      return { ...turn, style: kind.style };
    case "offer":
      return { ...turn, offer: kind.offer };
    case "paused":
      return { ...turn, state: "paused" };
    case "done":
      return { ...turn, state: "done", phase: null, cost: kind.cost };
    case "cost":
      return { ...turn, cost: kind.cost };
    case "failed":
      return { ...turn, state: "failed", phase: null, fault: kind.fault };
    case "cancelled":
      return { ...turn, state: "cancelled", phase: null };
    case "invalidated":
      return turn;
  }
}

export const lastTurn = (thread: Thread): TurnView | null =>
  [...thread.items].reverse().find((item): item is TurnView => item.type === "turn") ?? null;

/** A turn is at work until the core says it is done, stopped or failed. */
export const working = (thread: Thread): boolean => {
  const turn = lastTurn(thread);
  return (
    turn !== null &&
    (turn.state === "working" || turn.state === "waiting" || turn.state === "paused")
  );
};
