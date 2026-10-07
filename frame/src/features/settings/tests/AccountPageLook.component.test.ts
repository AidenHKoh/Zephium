import "$styles/global.css";
import { test, vi } from "vitest";
import { render } from "vitest-browser-svelte";
import { page } from "vitest/browser";
import type { AccountV1 } from "$shared/ipc/bindings";
import AccountPageStage from "./AccountPageStage.svelte";

const state = vi.hoisted(() => ({ account: null as AccountV1 | null }));
vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  return mockBindings({
    accountGet: async () => ({ account: state.account!, fault: null, retry_after: null }),
  });
});

const base: AccountV1 = {
  version: 1,
  available: true,
  server: "https://api.zephium.app",
  custom_server: false,
  hosted_models: true,
  billing: true,
  signup_open: true,
  signed_in: false,
  pending_email: null,
  resend_in: null,
  email: null,
  name: null,
  plan: null,
  usage: null,
  subscription: null,
  offers: [],
  checkout_open: false,
  notice: null,
};

const shots = "../../../../../target/account";
const settle = () => new Promise((done) => setTimeout(done, 400));

async function shoot(name: string, account: AccountV1) {
  state.account = account;
  await page.viewport(1100, 1300);
  const screen = await render(AccountPageStage);
  for (const theme of ["dark", "light"]) {
    document.documentElement.dataset.theme = theme;
    await settle();
    await page.screenshot({ path: `${shots}/${name}-${theme}.png` });
  }
  document.documentElement.dataset.theme = "dark";
  await screen.unmount();
}

test("Settings → Account signed out, waiting for a code, and signed in", async () => {
  await shoot("signed-out", base);
  await shoot("code", { ...base, pending_email: "ada@example.com", resend_in: 42 });
  await shoot("signed-in", {
    ...base,
    signed_in: true,
    email: "ada@example.com",
    name: "Ada Lovelace",
    plan: "free",
    usage: { used_percent: 62, resets_at: "2026-10-14T00:00:00Z", extra: true },
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
        price: { amount_minor: 1000, currency: "usd" },
        expires_days: 365,
      },
    ],
  });
});
