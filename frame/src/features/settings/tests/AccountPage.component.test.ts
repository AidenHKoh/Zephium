import "$styles/global.css";
import { expect, test, vi } from "vitest";
import { render } from "vitest-browser-svelte";
import { page, userEvent } from "vitest/browser";
import type { AccountReplyV1, AccountV1 } from "$shared/ipc/bindings";
import AccountPage from "../components/sections/AccountPage.svelte";

const native = vi.hoisted(() => ({
  get: vi.fn(),
  send: vi.fn(),
  verify: vi.fn(),
  cancel: vi.fn(),
  buy: vi.fn(),
  signOut: vi.fn(),
}));
vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  return mockBindings({
    accountGet: native.get,
    accountSendCode: native.send,
    accountVerify: native.verify,
    accountCancelCode: native.cancel,
    accountBuy: native.buy,
    accountSignOut: native.signOut,
  });
});

function account(overrides: Partial<AccountV1> = {}): AccountV1 {
  return {
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
    ...overrides,
  };
}

const reply = (value: AccountV1, fault: AccountReplyV1["fault"] = null): AccountReplyV1 => ({
  account: value,
  fault,
  retry_after: null,
});

const signedIn = account({
  signed_in: true,
  email: "ada@example.com",
  plan: "free",
  usage: { used_percent: 42, resets_at: "2026-10-14T00:00:00Z", extra: false },
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
      features: [],
    },
  ],
});

test("signing in takes an address, then the emailed code, with no password", async () => {
  await page.viewport(1100, 1000);
  native.get.mockResolvedValue(reply(account()));
  const waiting = account({ pending_email: "ada@example.com", resend_in: 60 });
  native.send.mockResolvedValue(reply(waiting));
  native.verify
    .mockResolvedValueOnce(reply(waiting, "invalid_code"))
    .mockResolvedValueOnce(reply(signedIn));
  const screen = await render(AccountPage);

  await expect.element(screen.getByRole("heading", { name: "Zephium account" })).toBeVisible();
  const continueButton = screen.getByRole("button", { name: "Continue" });
  await expect.element(continueButton).toBeDisabled();
  await screen.getByLabelText("Email address").fill("ada@example.com");
  await continueButton.click();
  expect(native.send).toHaveBeenCalledWith("ada@example.com");

  await expect
    .element(screen.getByText("Enter the 6-digit code sent to ada@example.com."))
    .toBeVisible();
  await expect.element(screen.getByRole("button", { name: /New code in \d+s/u })).toBeDisabled();
  await screen.getByLabelText("Code").click();
  await userEvent.keyboard("111 111");
  expect(native.verify).toHaveBeenLastCalledWith("111111");
  await expect
    .element(screen.getByText("That code didn’t work. Check it, or send a new one."))
    .toBeVisible();

  await userEvent.keyboard("123456");
  expect(native.verify).toHaveBeenLastCalledWith("123456");
  await expect.element(screen.getByRole("heading", { name: "ada@example.com" })).toBeVisible();
  await expect.element(screen.getByText("42% used", { exact: false })).toBeVisible();
});

test("upgrading opens checkout and says the plan will update on its own", async () => {
  await page.viewport(1100, 1200);
  native.get.mockResolvedValue(reply(signedIn));
  native.buy.mockResolvedValue(reply({ ...signedIn, checkout_open: true }));
  const screen = await render(AccountPage);

  await expect.element(screen.getByText("$20 a month")).toBeVisible();
  await screen.getByRole("button", { name: "Upgrade" }).click();
  expect(native.buy).toHaveBeenCalledWith({ kind: "plan", plan: "pro", interval: "monthly" });
  await expect
    .element(screen.getByText("Finish checkout in the new tab. Your plan updates here on its own."))
    .toBeVisible();
});

test("signing out returns to the sign-in screen", async () => {
  await page.viewport(1100, 1200);
  native.get.mockResolvedValue(reply(signedIn));
  native.signOut.mockResolvedValue(reply(account()));
  const screen = await render(AccountPage);
  await screen.getByRole("button", { name: "Sign out" }).first().click();
  await expect.element(screen.getByLabelText("Email address")).toBeVisible();
});
