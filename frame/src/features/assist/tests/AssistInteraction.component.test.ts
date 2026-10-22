import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import { render } from "vitest-browser-svelte";
import "$styles/global.css";
import type { PageAssistStatusV1 } from "$shared/ipc/bindings";
import { PROFILE } from "$shared/testing/work-models";
import { emitNativeEvent } from "$shared/testing/native-events";
import { AssistSession } from "../lib/assist-session.svelte";
import AssistStage from "./AssistStage.svelte";
import { ANSWER, asEntries, status as base, turn } from "./fixtures";

const status: typeof base = (entries, patch) => base(entries, { tab: "a", ...patch });

const native = vi.hoisted(() => ({
  statuses: {} as Record<string, PageAssistStatusV1>,
  ask: vi.fn(async () => ({ turn: 1, fault: null })),
  stop: vi.fn(async () => true),
  cite: vi.fn(async () => true),
  decide: vi.fn(async () => true),
  resume: vi.fn(async () => true),
  handoff: vi.fn(async () => ({ work: "w1", fault: null })),
}));

vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  return mockBindings({
    pageAssistStatus: async (tab: string) => native.statuses[tab]!,
    pageStyles: async () => ({ version: 1, tab: "a", site: null, styles: [] }),
    pageAssistAsk: native.ask,
    pageAssistStop: native.stop,
    pageAssistCite: native.cite,
    pageAssistDecide: native.decide,
    pageAssistResume: native.resume,
    pageAssistHandoff: native.handoff,
    workModels: async () => {
      throw new Error("offline");
    },
  });
});

let session: AssistSession;
beforeEach(() => {
  vi.clearAllMocks();
  native.statuses = { a: status(), b: status([], { tab: "b", site: "example.org" }) };
});
afterEach(() => session?.dispose());

async function open(tab = "a") {
  await page.viewport(380, 800);
  session = new AssistSession(PROFILE);
  await session.start();
  session.follow(tab);
  const screen = await render(AssistStage, { session });
  screen.container.style.cssText = "inline-size:336px;block-size:760px";
  await expect.poll(() => session.loading).toBe(false);
  return screen;
}

const send = (
  turnId: number,
  seq: number,
  kind: Parameters<typeof emitNativeEvent<"pageAssistChanged">>[1]["kind"],
  tab = "a",
) => emitNativeEvent("pageAssistChanged", { tab, turn: turnId, seq, kind });

test("typing and pressing Enter asks; Shift+Enter only adds a line", async () => {
  const screen = await open();
  const field = screen.getByRole("textbox");
  await field.fill("What is the price?");
  await userEvent.keyboard("{Shift>}{Enter}{/Shift}");
  expect(native.ask).not.toHaveBeenCalled();
  await userEvent.keyboard("{Enter}");
  await expect.poll(() => native.ask.mock.calls.length).toBe(1);
  expect(native.ask).toHaveBeenCalledWith({
    tab: "a",
    text: "What is the price?",
    intent: "ask",
    suggestion: null,
  });
});

test("a suggestion asks with its own id", async () => {
  const screen = await open();
  await screen.getByRole("button", { name: "Key numbers" }).click();
  await expect.poll(() => native.ask.mock.calls.length).toBe(1);
  expect(native.ask).toHaveBeenCalledWith({
    tab: "a",
    text: "Key numbers",
    intent: "ask",
    suggestion: "key_numbers",
  });
});

test("events for another tab and old sequence numbers never reach the thread", async () => {
  const screen = await open();
  send(1, 1, { type: "status", phase: { phase: "reading" } });
  await expect.element(screen.getByText("Reading the page…").first()).toBeVisible();
  send(1, 3, { type: "status", phase: { phase: "answering" } });
  send(1, 2, { type: "status", phase: { phase: "reading" } });
  await expect.element(screen.getByText("Answering…").first()).toBeVisible();
  send(9, 1, { type: "part", part: ANSWER[0]! }, "other");
  expect(screen.container.textContent).not.toContain("Fed held");
});

test("working shows Stop, and Stop asks the core to stop", async () => {
  const screen = await open();
  send(1, 1, { type: "status", phase: { phase: "reading" } });
  await screen.getByRole("button", { name: "Stop" }).click();
  expect(native.stop).toHaveBeenCalledWith("a");
});

test("switching tabs drops the thread and shows the other tab's", async () => {
  native.statuses.a = status(
    asEntries(turn(1, { question: "On tab a", parts: ANSWER.slice(0, 1) })),
  );
  const screen = await open();
  await expect.element(screen.getByText("On tab a")).toBeVisible();
  session.follow("b");
  expect(session.thread.items).toHaveLength(0);
  await expect.element(screen.getByText("example.org")).toBeVisible();
  expect(screen.container.textContent).not.toContain("On tab a");
});

test("a source goes to the page while it is live and is inert once the page moved on", async () => {
  native.statuses.a = status(
    asEntries(
      turn(1, { question: "Live", parts: ANSWER.slice(0, 2) }),
      turn(2, { question: "Old", parts: ANSWER.slice(0, 2), inert: true, seq: 11 }),
    ),
  );
  const screen = await open();
  const cites = screen.getByRole("button", { name: /^Source 1/ });
  await cites.first().click();
  expect(native.cite).toHaveBeenCalledWith("a", 1, 1);
  await expect.element(cites.nth(2)).toBeDisabled();
});

test("an action that commits is approved or declined by the person", async () => {
  native.statuses.a = status(
    asEntries(
      turn(1, {
        state: "waiting",
        question: "Buy it",
        steps: [{ index: 0, label: "Place order", category: "purchase", state: "confirm" }],
      }),
    ),
  );
  const screen = await open();
  await expect.element(screen.getByText("This spends money on nytimes.com.")).toBeVisible();
  await screen.getByRole("button", { name: "Place order" }).click();
  expect(native.decide).toHaveBeenCalledWith("a", 1, "approve");
  await screen.getByRole("button", { name: "Not now" }).click();
  expect(native.decide).toHaveBeenCalledWith("a", 1, "decline");
});

test("Continue resumes after a take-over and Run in Work hands the request over", async () => {
  native.statuses.a = status(
    asEntries(
      turn(1, { state: "paused", question: "Fill it in" }),
      turn(2, { offer: "multi_page", question: "Compare stores", seq: 11 }),
    ),
  );
  const screen = await open();
  await screen.getByRole("button", { name: "Continue" }).click();
  expect(native.resume).toHaveBeenCalledWith("a", 1);
  await screen.getByRole("button", { name: "Run in Work" }).click();
  expect(native.handoff).toHaveBeenCalledWith("a", 2);
});

test("a refused page shows why and offers no field to type in", async () => {
  native.statuses.a = status([], { availability: "private" });
  const screen = await open();
  await expect.element(screen.getByText("Not in private windows")).toBeVisible();
  await expect.element(screen.getByRole("textbox")).toBeDisabled();
});
