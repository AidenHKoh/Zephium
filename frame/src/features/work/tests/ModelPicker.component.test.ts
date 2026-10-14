import "$styles/global.css";
import { expect, test, vi } from "vitest";
import { render } from "vitest-browser-svelte";
import { page, userEvent } from "vitest/browser";
import type { WorkModelReasoning, WorkModelsReadyV1 } from "$shared/ipc/bindings";
import ModelPicker from "../components/bar/ModelPicker.svelte";
import { PROFILE, models } from "$shared/testing/work-models";

const native = vi.hoisted(() => ({
  read: vi.fn(),
  choose: vi.fn(),
  effort: vi.fn(),
  more: vi.fn(),
  account: vi.fn(),
  command: vi.fn(async () => ({ operation_id: "op", accepted: true })),
  ready: vi.fn(async (profile: string): Promise<WorkModelsReadyV1> => ({
    version: 1,
    profile,
    ready: true,
    missing_roles: [],
    fault: null,
  })),
}));
vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  return mockBindings({
    workModels: native.read,
    workChooseModel: native.choose,
    workSetModelEffort: native.effort,
    workMoreModels: native.more,
    workModelsReady: native.ready,
    accountGet: native.account,
    runCommand: native.command,
  });
});

const view = (lead: string, chosen: string | null = null, effort?: WorkModelReasoning) =>
  models({
    keys: { anthropic: "valid", open_ai: "set", open_router: "valid" },
    lead,
    chosen,
    effort,
  });

const ready = (value: boolean) => async (profile: string) => ({
  version: 1,
  profile,
  ready: value,
  missing_roles: value ? [] : (["lead"] as const).slice(),
  fault: null,
});

test("the trigger names the running model and its effort, and the menu changes the model", async () => {
  await page.viewport(1200, 800);
  native.ready.mockImplementation(ready(true));
  native.read.mockResolvedValue(view("anthropic/claude-opus-5-5"));
  native.choose.mockResolvedValue(view("openai/gpt-6-sol", "openai/gpt-6-sol"));
  const onsettings = vi.fn();
  const screen = await render(ModelPicker, { profile: PROFILE, onsettings });

  const trigger = screen.getByRole("button", { name: "Model: Claude Opus 5.5, Medium effort" });
  await expect.element(trigger).toHaveTextContent("Opus 5.5");
  await trigger.click();

  await expect
    .element(page.getByRole("tab", { name: "Anthropic" }))
    .toHaveAttribute("aria-selected", "true");
  await expect
    .element(page.getByRole("option", { name: /Claude Opus 5\.5/u }))
    .toHaveAttribute("aria-selected", "true");
  await expect.element(page.getByRole("option", { name: /Claude Opus 5\.5/u })).toHaveFocus();
  // The running model says what it can do, from the catalog.
  await expect
    .element(page.getByRole("option", { name: /Claude Opus 5\.5/u }))
    .toHaveTextContent(/1M/u);

  await page.getByRole("tab", { name: "OpenAI" }).click();
  await page.getByRole("option", { name: /GPT-6 Sol/u }).click();
  expect(native.choose).toHaveBeenCalledWith(PROFILE, "lead", "openai/gpt-6-sol");
  await expect.element(screen.getByRole("button", { name: /Model: GPT-6 Sol/u })).toBeVisible();

  await screen.getByRole("button", { name: /Model: GPT-6 Sol/u }).click();
  // Google has no key: its tile offers one.
  await page.getByRole("tab", { name: "Google" }).click();
  await expect.element(page.getByText("Add your Google key", { exact: false })).toBeVisible();
  await page.getByRole("button", { name: "Add key" }).click();
  expect(onsettings).toHaveBeenCalledTimes(1);
});

test("typing searches every provider, with each match marked by its provider", async () => {
  await page.viewport(1200, 800);
  native.ready.mockImplementation(ready(true));
  native.read.mockResolvedValue(view("openai/gpt-6-luna"));
  native.choose.mockResolvedValue(
    view("anthropic/claude-sonnet-5-5", "anthropic/claude-sonnet-5-5"),
  );
  const screen = await render(ModelPicker, { profile: PROFILE, onsettings: vi.fn() });
  await screen.getByRole("button", { name: /Model: GPT-6 Luna/u }).click();
  await expect.element(page.getByRole("option", { name: /GPT-6 Luna/u })).toHaveFocus();

  await userEvent.keyboard("sonnet");
  await expect.element(page.getByRole("textbox", { name: "Search models" })).toHaveValue("sonnet");
  const results = page.getByRole("listbox", { name: "Search models" });
  await expect.poll(() => results.getByRole("option").all().length).toBe(2);
  // Searching lifts the rail's selection.
  expect(
    page
      .getByRole("tab")
      .all()
      .some((tab) => tab.element().getAttribute("aria-selected") === "true"),
  ).toBe(false);
  await userEvent.keyboard("{ArrowDown}{Enter}");
  expect(native.choose).toHaveBeenCalledWith(PROFILE, "lead", "anthropic/claude-sonnet-5-5");
});

test("the effort chip opens a track that steps through the model's own levels", async () => {
  await page.viewport(1200, 800);
  native.ready.mockImplementation(ready(true));
  native.read.mockResolvedValue(view("openai/gpt-6-luna"));
  native.effort.mockImplementation(async (_profile: string, effort: WorkModelReasoning) =>
    view("openai/gpt-6-luna", null, effort),
  );
  const screen = await render(ModelPicker, { profile: PROFILE, onsettings: vi.fn() });
  await screen.getByRole("button", { name: /Model: GPT-6 Luna, Medium effort/u }).click();
  await page.getByRole("button", { name: "Effort: Medium" }).click();

  const track = page.getByRole("slider", { name: "Effort" });
  await expect.element(track).toHaveFocus();
  await expect.element(track).toHaveAttribute("aria-valuemax", "4");
  await expect.element(track).toHaveAttribute("aria-valuetext", "Medium");
  await userEvent.keyboard("{ArrowRight}");
  await expect.element(track).toHaveAttribute("aria-valuetext", "High");
  expect(native.effort).toHaveBeenLastCalledWith(PROFILE, "high");
  await userEvent.keyboard("{End}");
  await expect.element(track).toHaveAttribute("aria-valuetext", "Max");
  expect(native.effort).toHaveBeenLastCalledWith(PROFILE, "max");
  await expect
    .element(screen.getByRole("button", { name: /Model: GPT-6 Luna, Max effort/u }))
    .toBeInTheDocument();

  // Escape folds the track back into its chip, and the menu stays.
  await userEvent.keyboard("{Escape}");
  await expect.element(page.getByRole("button", { name: "Effort: Max" })).toHaveFocus();
  await expect.element(track).not.toBeInTheDocument();
});

test("a model whose wire sets no effort shows no effort control", async () => {
  await page.viewport(1200, 800);
  native.ready.mockImplementation(ready(true));
  const plain = models({ keys: { mistral: "valid" }, lead: "mistral/mistral-medium-3-5" });
  plain.entries.push({
    ...plain.entries[0]!,
    id: "mistral/mistral-medium-3-5",
    model: { provider: "mistral", wire: "chat_completions", model: "mistral-medium-3-5" },
    display_name: "Mistral Medium 3.5",
  });
  native.read.mockResolvedValue(plain);
  const screen = await render(ModelPicker, { profile: PROFILE, onsettings: vi.fn() });
  await screen.getByRole("button", { name: "Model: Mistral Medium 3.5" }).click();
  await expect
    .element(page.getByRole("option", { name: /Mistral Medium 3\.5/u }))
    .toHaveAttribute("aria-selected", "true");
  expect(page.getByRole("button", { name: /^Effort/u }).query()).toBeNull();
});

test("without any key the control is one calm way to add a key, straight to Settings", async () => {
  await page.viewport(1200, 800);
  native.read.mockResolvedValue(models({}));
  native.ready.mockImplementation(ready(false));
  const onsettings = vi.fn();
  const screen = await render(ModelPicker, { profile: PROFILE, onsettings });
  await screen.getByRole("button", { name: "Add a model key" }).click();
  expect(onsettings).toHaveBeenCalledTimes(1);
  expect(page.getByRole("tab").query()).toBeNull();
});

test("signed out where hosted models exist, Zephium AI is one sign-in away", async () => {
  await page.viewport(1200, 800);
  native.read.mockResolvedValue(view("anthropic/claude-opus-5-5"));
  native.account.mockResolvedValue({
    account: {
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
    },
    fault: null,
    retry_after: null,
  });
  native.ready.mockImplementation(ready(false));
  const keyless = await render(ModelPicker, { profile: PROFILE });
  await expect.element(keyless.getByRole("button", { name: "Sign in or add a key" })).toBeVisible();
  await keyless.unmount();

  native.ready.mockImplementation(ready(true));
  const screen = await render(ModelPicker, { profile: PROFILE });
  await screen.getByRole("button", { name: /Model: Claude Opus 5\.5/u }).click();
  await page.getByRole("tab", { name: "Zephium AI" }).click();
  await page.getByRole("button", { name: "Sign in" }).click();
  expect(native.command).toHaveBeenCalledWith("settings.account");
});

test("the menu says so when no chosen model can search the web", async () => {
  await page.viewport(1200, 800);
  native.read.mockResolvedValue(
    models({ keys: { deep_seek: "valid" }, lead: "deepseek/deepseek-v4-pro" }),
  );
  native.ready.mockImplementation(async (profile: string) => ({
    version: 1,
    profile,
    ready: true,
    missing_roles: [],
    search: "unavailable",
    fault: null,
  }));
  const screen = await render(ModelPicker, { profile: PROFILE });
  await screen.getByRole("button", { name: /Model: DeepSeek V4 Pro/u }).click();
  await expect.element(page.getByText("Web search is off", { exact: false })).toBeVisible();
});
