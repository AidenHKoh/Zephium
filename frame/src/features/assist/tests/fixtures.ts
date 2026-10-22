import type {
  PageAssistEntryV1,
  PageAssistPartV1,
  PageAssistStatusV1,
  PageAssistTurnV1,
} from "$shared/ipc/bindings";

export const turn = (turn: number, patch: Partial<PageAssistTurnV1> = {}): PageAssistTurnV1 => ({
  turn,
  intent: "ask",
  suggestion: null,
  question: "",
  selection: null,
  state: "done",
  phase: null,
  parts: [],
  steps: [],
  style: null,
  offer: null,
  cost: null,
  fault: null,
  inert: false,
  seq: 10,
  ...patch,
});

export const status = (
  entries: PageAssistEntryV1[] = [],
  patch: Partial<PageAssistStatusV1> = {},
): PageAssistStatusV1 => ({
  version: 1,
  tab: "tab-1",
  availability: "ready",
  site: "nytimes.com",
  model: { name: "Luna", hosted: true },
  suggestions: ["summarize", "key_numbers", "catch", "explain", "redesign", "darker", "readable"],
  selection: null,
  can_act: true,
  styled: false,
  entries,
  ...patch,
});

export const asEntries = (...turns: PageAssistTurnV1[]): PageAssistEntryV1[] =>
  turns.map((item) => ({ kind: "turn", turn: item }));

const part = (
  kind: PageAssistPartV1["kind"],
  text: string,
  cites: number[] = [],
  label: string | null = null,
): PageAssistPartV1 => ({
  kind,
  text,
  label,
  cites: cites.map((n) => ({ n, excerpt: `Passage ${n}` })),
});

export const ANSWER: PageAssistPartV1[] = [
  part("answer", "The Fed held rates steady and signalled one cut before the year ends.", [1]),
  part("point", "Policymakers voted 10 to 2 to keep the benchmark range where it is.", [1, 2]),
  part(
    "point",
    "Inflation is described as “somewhat elevated”, down from “elevated” in September.",
    [3],
  ),
  part("point", "Two members preferred an immediate quarter-point cut.", [2]),
  part("figure", "4.25–4.50%", [1], "Benchmark range"),
  part("figure", "10 to 2", [2], "Vote"),
  part("figure", "2.6%", [3], "Core inflation, year on year"),
  part("caveat", "Not on this page: how markets reacted after the announcement."),
];
