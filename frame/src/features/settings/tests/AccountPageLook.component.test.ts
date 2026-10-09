import "$styles/global.css";
import { test, vi } from "vitest";
import { render } from "vitest-browser-svelte";
import { page } from "vitest/browser";
import type { AccountHistoryV1, AccountRequestV1, AccountV1 } from "$shared/ipc/bindings";
import { accountFixture, usageFixture } from "$shared/testing/account";
import AccountPageStage from "./AccountPageStage.svelte";

const state = vi.hoisted(() => ({ account: null as AccountV1 | null }));
vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  return mockBindings({
    accountGet: async () => ({ account: state.account!, fault: null, retry_after: null }),
    accountUsageHistory: async () => history,
    accountReceipt: async (id: string) => ({
      request: history.requests.find((row) => row.id === id) ?? null,
      fault: null,
    }),
  });
});

const base = accountFixture();

function request(id: string, model: string, state: string, charged: string): AccountRequestV1 {
  return {
    id,
    model_id: model,
    state,
    charged,
    reserved: "1200000",
    started_at: "2026-10-09T10:0" + id + ":00Z",
    finished_at: state === "running" ? null : "2026-10-09T10:0" + id + ":20Z",
    error_class: null,
  };
}

const history: AccountHistoryV1 = {
  requests: [
    request("5", "fast", "running", "0"),
    request("4", "fast", "settled", "312000"),
    request("3", "deep", "settled", "1840000"),
    request("2", "fast", "usage_unknown", "0"),
    request("1", "fast", "released", "0"),
  ],
  next_cursor: "more",
  fault: null,
};

const shots = "../../../../../target/account";
const settle = () => new Promise((done) => setTimeout(done, 400));

async function shoot(
  name: string,
  account: AccountV1,
  act?: (screen: Awaited<ReturnType<typeof render>>) => Promise<void>,
) {
  state.account = account;
  await page.viewport(1100, 1600);
  const screen = await render(AccountPageStage);
  await settle();
  await act?.(screen);
  for (const theme of ["dark", "light"]) {
    document.documentElement.dataset.theme = theme;
    await settle();
    await page.screenshot({ path: `${shots}/${name}-${theme}.png` });
  }
  document.documentElement.dataset.theme = "dark";
  await screen.unmount();
}

const signedIn: AccountV1 = {
  ...base,
  signed_in: true,
  email: "ada@example.com",
  name: "Ada Lovelace",
  plan: "free",
  usage: usageFixture({
    used_percent: 62,
    allowance_used: "1240000",
    bonus_remaining: "1500000",
    bonus_expires_at: "2026-10-20T00:00:00Z",
    purchased_remaining: "5000000",
    held: "120000",
    spent_today: "300000",
    daily_limit: "1000000",
  }),
  subscription: {
    status: "inactive",
    interval: null,
    paid_until: null,
    cancel_at_period_end: false,
  },
  offers: [
    {
      kind: "plan",
      plan: "pro",
      interval: "monthly",
      name: "Zephium Pro",
      price: { amount_minor: 2000, currency: "usd" },
      features: ["Frontier models", "More usage"],
    },
    {
      kind: "plan",
      plan: "pro",
      interval: "yearly",
      name: "Zephium Pro",
      price: { amount_minor: 19200, currency: "usd" },
      features: ["Frontier models", "More usage"],
    },
    {
      kind: "top_up",
      id: "ten",
      name: "10 credits",
      units: "10000000",
      price: { amount_minor: 1000, currency: "usd" },
      expires_days: 365,
    },
  ],
};

test("Settings → Account signed out, waiting for a code, and signed in", async () => {
  await shoot("signed-out", base);
  await shoot("code", { ...base, pending_email: "ada@example.com", resend_in: 42 });
  await shoot("signed-in", signedIn);
  await shoot("history", signedIn, async (screen) => {
    await screen.getByRole("button", { name: "Show" }).first().click();
    await settle();
    await screen.getByRole("button", { name: "Details for this request" }).nth(1).click();
  });
  await shoot("email-change", signedIn, async (screen) => {
    await screen.getByRole("button", { name: "Change…" }).first().click();
    await settle();
    await screen.getByLabelText("New email address").fill("ada@new.example");
  });
  await shoot("billing-off", {
    ...signedIn,
    billing: false,
    offers: [],
    custom_server: true,
    server: "https://zephium.example.org",
    update_required: true,
    unsettled: [history.requests[3]!],
  });
});
