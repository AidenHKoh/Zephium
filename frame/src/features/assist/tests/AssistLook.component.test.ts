import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import { render } from "vitest-browser-svelte";
import "$styles/global.css";
import type { PageAssistStatusV1 } from "$shared/ipc/bindings";
import { PROFILE } from "$shared/testing/work-models";
import { AssistSession } from "../lib/assist-session.svelte";
import AssistStage from "./AssistStage.svelte";
import { ANSWER, asEntries, status, turn } from "./fixtures";

const native = vi.hoisted(() => ({ status: null as PageAssistStatusV1 | null }));

vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  const { models } = await import("$shared/testing/work-models");
  return mockBindings({
    pageAssistStatus: async () => native.status!,
    pageStyles: async () => ({ version: 1, tab: "tab-1", site: "nytimes.com", styles: [] }),
    workModels: async () => {
      const view = models({ cloud: true, lead: "zephium/claude-sonnet-5-5" });
      return { ...view, effective: { ...view.effective, page: "zephium/claude-sonnet-5-5" } };
    },
    runCommand: async () => ({ accepted: true, operation_id: null }),
  });
});

let session: AssistSession | null = null;
afterEach(() => {
  session?.dispose();
  session = null;
  delete document.documentElement.dataset.theme;
});

const settled = () =>
  Promise.all(
    document
      .getAnimations()
      .filter((animation) => animation.effect?.getComputedTiming().iterations !== Infinity)
      .map((animation) => animation.finished.catch(() => undefined)),
  );

async function capture(container: HTMLElement, name: string) {
  for (const theme of ["dark", "light"]) {
    document.documentElement.dataset.theme = theme;
    container.style.background = "var(--color-chrome)";
    await settled();
    await page
      .elementLocator(container)
      .screenshot({ path: `../../../../../target/look/assist/${name}-${theme}.png` });
  }
}

async function open(next: PageAssistStatusV1, height = 760) {
  native.status = next;
  await page.viewport(380, height + 40);
  session = new AssistSession(PROFILE);
  await session.start();
  session.follow("tab-1");
  const screen = await render(AssistStage, {
    session,
    page: { title: "Fed holds rates steady and signals one cut this year", url: null },
  });
  screen.container.style.cssText = `inline-size:336px;block-size:${height}px`;
  await expect.poll(() => session!.loading).toBe(false);
  return screen;
}

test("empty: a greeting for the page and the suggestions the core chose", async () => {
  const screen = await open(status());
  await expect.element(screen.getByText("Understand")).toBeVisible();
  await expect.element(screen.getByRole("button", { name: "Key numbers" })).toBeVisible();
  await capture(screen.container, "empty");
});

test("status: one line while the page is read", async () => {
  const screen = await open(
    status(
      asEntries(
        turn(1, {
          question: "What changed since September?",
          state: "working",
          phase: { phase: "read", passages: 214 },
          seq: 3,
        }),
      ),
    ),
    360,
  );
  await expect.element(screen.getByText("Read 214 passages").first()).toBeVisible();
  await capture(screen.container, "status");
});

test("answer: headline, points, figures and a caveat, each with its sources", async () => {
  const screen = await open(
    status(
      asEntries(
        turn(1, {
          question: "What did the Fed decide?",
          parts: ANSWER,
          cost: { kind: "hosted", model: "Luna", credits: "0.3" },
        }),
      ),
    ),
  );
  await expect.element(screen.getByText("Luna · 0.3 credits")).toBeVisible();
  await capture(screen.container, "answer");
});

test("a thread across a navigation: old sources go quiet and cost shows both ways", async () => {
  const screen = await open(
    status([
      ...asEntries(
        turn(1, {
          question: "Summarize",
          intent: "summarize",
          parts: ANSWER.slice(0, 2),
          inert: true,
          cost: { kind: "hosted", model: "Luna", credits: null },
        }),
      ),
      { kind: "divider", site: "nytimes.com", cross_site: false },
      { kind: "divider", site: "wikipedia.org", cross_site: true },
      ...asEntries(
        turn(2, {
          question: "Who founded it?",
          parts: [ANSWER[0]!],
          cost: { kind: "byok", model: "gpt-5", provider: "OpenAI", usd: "0.004" },
        }),
      ),
    ]),
  );
  await expect.element(screen.getByText("≈ $0.004 on your OpenAI key")).toBeVisible();
  await capture(screen.container, "thread");
});

test("refusal: a sensitive site is not read", async () => {
  const screen = await open(status([], { availability: "sensitive", site: "chase.com" }), 420);
  await expect.element(screen.getByText("Ask stays out of this site")).toBeVisible();
  await capture(screen.container, "refusal");
});

test("faults say what to do next", async () => {
  const screen = await open(
    status(
      asEntries(
        turn(1, {
          question: "Compare the plans",
          state: "failed",
          fault: { reason: "model", fault: "hosted_usage" },
        }),
        turn(2, {
          question: "Try again",
          state: "failed",
          fault: { reason: "model", fault: "rate_limited" },
          seq: 12,
        }),
      ),
    ),
    520,
  );
  await expect.element(screen.getByRole("button", { name: "Buy credits" })).toBeVisible();
  await capture(screen.container, "fault");
});

test("a step that commits waits for approval, naming the control", async () => {
  const screen = await open(
    status(
      asEntries(
        turn(1, {
          question: "Order the cheapest one",
          intent: "act",
          state: "waiting",
          steps: [
            { index: 0, label: "Sorted by price, low to high", category: null, state: "done" },
            { index: 1, label: "Opened “Aeropress Go”", category: null, state: "done" },
            { index: 2, label: "Place order", category: "purchase", state: "confirm" },
          ],
        }),
      ),
    ),
    480,
  );
  await expect.element(screen.getByRole("button", { name: "Place order" })).toBeVisible();
  await capture(screen.container, "confirm");
});

test("you took over, and a big request is offered to Work", async () => {
  const screen = await open(
    status(
      asEntries(
        turn(1, { question: "Fill in the form", intent: "act", state: "paused", seq: 4 }),
        turn(2, {
          question: "Compare these across the five stores",
          offer: "multi_page",
          state: "done",
          seq: 4,
        }),
      ),
    ),
    480,
  );
  await expect.element(screen.getByRole("button", { name: "Continue" })).toBeVisible();
  await expect.element(screen.getByRole("button", { name: "Run in Work" })).toBeVisible();
  await capture(screen.container, "paused-work");
});

test("a style proposal, then the badge of an applied one", async () => {
  const screen = await open(
    status(
      asEntries(
        turn(1, {
          question: "Make this darker",
          intent: "style",
          style: {
            id: null,
            summary: "Dark background, light text, softer links",
            site: "nytimes.com",
            rules: 6,
            state: "proposed",
          },
        }),
        turn(2, {
          question: "Bigger text too",
          intent: "style",
          style: {
            id: "s1",
            summary: "Text 18px",
            site: "nytimes.com",
            rules: 2,
            state: "applied",
          },
          seq: 12,
        }),
      ),
    ),
    520,
  );
  await expect.element(screen.getByRole("button", { name: "Apply" })).toBeVisible();
  await capture(screen.container, "style");
});

test("the model menu opens over the thread, grouped by provider", async () => {
  const screen = await open(
    status(asEntries(turn(1, { question: "Summarize", parts: ANSWER.slice(0, 2) }))),
    520,
  );
  await screen.getByRole("button", { name: /Sonnet/ }).click();
  await expect.element(screen.getByRole("listbox")).toBeVisible();
  await capture(screen.container, "model-menu");
});

test("a long answer scrolls inside the panel and stays pinned to its end", async () => {
  const points = Array.from({ length: 12 }, (_, at) => ({
    ...ANSWER[1]!,
    text: `Point ${at + 1}: ${ANSWER[1]!.text}`,
  }));
  const screen = await open(
    status(
      asEntries(
        turn(1, {
          question: "Give me everything on this page",
          parts: [ANSWER[0]!, ...points, ANSWER[7]!],
          cost: { kind: "hosted", model: "Luna", credits: "0.8" },
        }),
      ),
    ),
    520,
  );
  const scroller = screen.container.querySelector<HTMLElement>(".thread")!;
  await expect.poll(() => scroller.scrollTop).toBeGreaterThan(200);
  await expect.element(screen.getByText("Luna · 0.8 credits")).toBeVisible();
  await capture(screen.container, "long");
});
