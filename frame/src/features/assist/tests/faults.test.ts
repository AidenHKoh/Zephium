import { expect, test } from "vitest";
import type { WorkModelFaultV1 } from "$shared/ipc/bindings";
import { faultView, refusalView } from "../lib/faults";

const model = (fault: WorkModelFaultV1, retry_after_ms: number | null = null) =>
  faultView({ reason: "model", fault, retry_after_ms });

test("out of credits sends the person to buy more, not to retry", () => {
  const view = model("hosted_usage");
  expect(view.fix?.command).toBe("settings.account");
  expect(view.retry).toBe("no");
});

test("a rate limit waits, then offers the same question again", () => {
  expect(model("rate_limited", 12_000)).toMatchObject({ retry: "wait", fix: null, wait: 12 });
});

test("a missing or refused key leads to Settings → AI", () => {
  expect(model("missing_key").fix?.command).toBe("settings.ai");
  expect(model("key_refused").fix?.command).toBe("settings.ai");
  expect(faultView({ reason: "no_model" }).fix?.command).toBe("settings.ai");
});

test("a plan a model needs and the daily limit point at the account", () => {
  expect(model("plan_required").fix?.command).toBe("settings.account");
  expect(model("daily_limit").message).toContain("resets");
});

test("a page that changed may be asked again; no selection may not", () => {
  expect(faultView({ reason: "page_changed" }).retry).toBe("now");
  expect(faultView({ reason: "no_selection" }).retry).toBe("no");
});

test("every refusal has words of its own", () => {
  const titles = (
    ["ai_off", "private", "sensitive", "not_web", "no_model", "unsupported"] as const
  ).map((kind) => refusalView(kind).title);
  expect(new Set(titles).size).toBe(titles.length);
});
