import "$styles/global.css";
import { expect, test, vi } from "vitest";
import { render } from "vitest-browser-svelte";
import { page, userEvent } from "vitest/browser";
import { PROFILE, models } from "$shared/testing/work-models";
import AiPage from "../components/sections/AiPage.svelte";

vi.mock("$domain/tabs", () => ({
  tabs: { profile: () => ({ id: "00000000000000000000000001", kind: "regular" }) },
}));
const native = vi.hoisted(() => ({
  read: vi.fn(),
  set: vi.fn(),
  test: vi.fn(),
  choose: vi.fn(),
  account: vi.fn(),
}));
vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  return mockBindings({
    workModels: native.read,
    workSetProviderKey: native.set,
    workTestProviderKey: native.test,
    workChooseModel: native.choose,
    accountGet: native.account,
    workDecisionPreference: async () => ({
      version: 1,
      profile: PROFILE,
      choice: "recommended",
      effective: "standard",
      typesafe_key_present: false,
      error: null,
    }),
  });
});

test("a refused key is explained and acceptance makes no billing promise", async () => {
  await page.viewport(1100, 1200);
  native.read.mockResolvedValue(models({ keys: { open_ai: "set" }, lead: "openai/gpt-6-sol" }));
  native.set
    .mockResolvedValueOnce({ ...models({ keys: { open_ai: "set" } }), fault: "key_refused" })
    .mockResolvedValueOnce(models({ keys: { open_ai: "set", anthropic: "valid" } }));
  const screen = await render(AiPage);

  await screen.getByRole("button", { name: "Add key" }).first().click();
  await userEvent.keyboard("sk-ant-wrong");
  await screen.getByRole("button", { name: "Save" }).click();
  expect(native.set).toHaveBeenCalledWith(PROFILE, "anthropic", "sk-ant-wrong");
  await expect.element(screen.getByText("Anthropic didn’t accept this key.")).toBeVisible();

  await screen.getByLabelText("Anthropic API key").fill("sk-ant-right");
  await screen.getByRole("button", { name: "Save" }).click();
  await expect.element(screen.getByText("Key accepted")).toBeVisible();
  await expect.element(screen.getByRole("button", { name: "Save" })).not.toBeInTheDocument();
});

test("testing a stored key reports what the provider said", async () => {
  await page.viewport(1100, 1200);
  native.read.mockResolvedValue(models({ keys: { open_ai: "set" } }));
  native.test.mockResolvedValue({ ...models({ keys: { open_ai: "set" } }), fault: "unreachable" });
  const screen = await render(AiPage);
  await screen.getByRole("button", { name: "Test" }).click();
  expect(native.test).toHaveBeenCalledWith(PROFILE, "open_ai");
  await expect
    .element(screen.getByText("Couldn’t reach OpenAI. Check your connection and try again."))
    .toBeVisible();
});

test("a role follows Automatic until the person chooses a model", async () => {
  await page.viewport(1100, 1200);
  native.read.mockResolvedValue(
    models({ keys: { anthropic: "valid" }, lead: "anthropic/claude-opus-5-5" }),
  );
  native.choose.mockResolvedValue(
    models({
      keys: { anthropic: "valid" },
      lead: "anthropic/claude-sonnet-5-5",
      chosen: "anthropic/claude-sonnet-5-5",
    }),
  );
  const screen = await render(AiPage);
  await expect.element(screen.getByText("Automatic · Claude Opus 5.5")).toBeVisible();
  await screen.getByText("Automatic · Claude Opus 5.5").click();
  await page.getByRole("option", { name: /Claude Sonnet 5.5/ }).click();
  expect(native.choose).toHaveBeenCalledWith(PROFILE, "lead", "anthropic/claude-sonnet-5-5");
});

test("billing trouble preserves acceptance and the old Cloud group stays gone", async () => {
  await page.viewport(1100, 1200);
  const view = models({ keys: { open_ai: "valid" }, cloud: true });
  view.providers = view.providers.map((provider) =>
    provider.provider === "open_ai" ? { ...provider, fault: "billing" } : provider,
  );
  native.read.mockResolvedValue(view);
  const screen = await render(AiPage);
  await expect.element(screen.getByText("Key accepted")).toBeVisible();
  await expect
    .element(
      screen.getByText("Check your provider’s billing or available credits, then try again."),
    )
    .toBeVisible();
  await expect.element(screen.getByText("Zephium Cloud", { exact: true })).not.toBeInTheDocument();
});

test("Settings AI exposes Jev setup beside the regular model controls with honest fallback", async () => {
  await page.viewport(1100, 1200);
  native.read.mockResolvedValue(models({ keys: { open_ai: "set" }, lead: "openai/gpt-6-sol" }));
  const screen = await render(AiPage);
  await expect.element(screen.getByRole("button", { name: "Add Jev key" })).toBeVisible();
  await expect.element(screen.getByRole("radio", { name: "Recommended" })).toBeChecked();
  await expect.element(screen.getByText("Active: Standard")).toBeVisible();
  await expect.element(screen.getByRole("heading", { name: "Work", exact: true })).toBeVisible();
});

test("Zephium AI is offered where the server hosts models, and shows the account once signed in", async () => {
  await page.viewport(1100, 1200);
  native.read.mockResolvedValue(models({}));
  const account = {
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
  native.account.mockResolvedValue({ account, fault: null, retry_after: null });
  const screen = await render(AiPage);
  await expect.element(screen.getByRole("button", { name: "Sign in…" })).toBeVisible();
  await screen.unmount();

  native.read.mockResolvedValue(models({ cloud: true }));
  native.account.mockResolvedValue({
    account: {
      ...account,
      signed_in: true,
      email: "ada@example.com",
      plan: "free",
      usage: { used_percent: 30, resets_at: null, extra: false },
    },
    fault: null,
    retry_after: null,
  });
  const signed = await render(AiPage);
  await expect.element(signed.getByText("Signed in · Free plan · 30% used")).toBeVisible();
  await expect
    .element(signed.getByText("Add a provider key or sign in", { exact: false }))
    .not.toBeInTheDocument();
});
