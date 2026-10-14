import "$styles/global.css";
import { afterEach, test, vi } from "vitest";
import { render } from "vitest-browser-svelte";
import { page, userEvent } from "vitest/browser";
import type {
  AccountV1,
  WorkModelProvider,
  WorkModelReasoning,
  WorkModelRole,
  WorkModelsV1,
} from "$shared/ipc/bindings";
import { PROFILE, models } from "$shared/testing/work-models";
import ModelPickerStage from "./ModelPickerStage.svelte";

const native = vi.hoisted(() => ({
  view: null as unknown as WorkModelsV1,
  ready: true,
  hosted: false,
}));

vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  const { accountFixture } = await import("$shared/testing/account");
  return mockBindings({
    workModels: vi.fn(async () => native.view),
    workModelsReady: vi.fn(async (profile: string) => ({
      version: 1,
      profile,
      ready: native.ready,
      missing_roles: (native.ready ? [] : ["lead"]) as WorkModelRole[],
      fault: null,
    })),
    workSetModelEffort: vi.fn(async (_profile: string, effort: WorkModelReasoning | null) => {
      native.view = { ...native.view, effort };
      return native.view;
    }),
    workMoreModels: vi.fn(async (_profile: string, provider: WorkModelProvider) => ({
      provider,
      entries: [],
      fault: null,
    })),
    accountGet: vi.fn(async () => ({
      account: native.hosted
        ? accountFixture({ available: true, hosted_models: true, signed_in: false })
        : (null as unknown as AccountV1),
      fault: null,
      retry_after: null,
    })),
  });
});

const shots = "../../../../../target/look/model-picker";
const settle = (ms = 450) => new Promise((done) => setTimeout(done, ms));
const keyed = (lead: string) =>
  models({
    keys: { anthropic: "valid", open_ai: "valid", deep_seek: "valid", google: "invalid" },
    lead,
  });

async function shoot(name: string, materials: readonly string[] = ["none"]) {
  for (const theme of ["dark", "light"]) {
    for (const material of materials) {
      document.documentElement.dataset.theme = theme;
      document.documentElement.dataset.material = material;
      await settle();
      const suffix = materials.length > 1 ? `-${material === "none" ? "solid" : "glass"}` : "";
      await page.screenshot({ path: `${shots}/${name}-${theme}${suffix}.png` });
    }
  }
  document.documentElement.dataset.theme = "dark";
  document.documentElement.dataset.material = "none";
}

afterEach(() => {
  native.ready = true;
  native.hosted = false;
  delete document.documentElement.dataset.material;
});

test("providers, search and the trigger at rest", async () => {
  await page.viewport(1100, 700);
  native.view = keyed("anthropic/claude-opus-5-5");
  const screen = await render(ModelPickerStage, { profile: PROFILE });
  await shoot("rest");
  await screen.getByRole("button", { name: /Model: Claude Opus 5\.5/u }).click();
  await shoot("anthropic", ["none", "liquid_glass"]);
  await page.getByRole("tab", { name: "OpenAI" }).click();
  await shoot("openai");
  await page.getByRole("tab", { name: "DeepSeek" }).click();
  await shoot("deepseek");
  await page.getByRole("tab", { name: "Google" }).click();
  await shoot("needs-key");
  await page.getByRole("textbox", { name: "Search models" }).click();
  await userEvent.keyboard("5.5");
  await shoot("search");
});

test("the effort panel at every level", async () => {
  await page.viewport(1100, 700);
  native.view = { ...keyed("openai/gpt-6-luna"), effort: "low" };
  const screen = await render(ModelPickerStage, { profile: PROFILE });
  await screen.getByRole("button", { name: /Model: GPT-6 Luna/u }).click();
  await page.getByRole("button", { name: /^Effort/u }).click();
  const levels = ["low", "medium", "high", "xhigh", "max"];
  for (const [index, level] of levels.entries()) {
    if (index) await userEvent.keyboard("{ArrowRight}");
    if (!index) await userEvent.hover(page.getByText("Faster"));
    await shoot(`effort-${level}`);
  }
  await userEvent.keyboard("{Escape}{Escape}");
  await shoot("rest-max");
});

test("keyless and hosted", async () => {
  await page.viewport(1100, 700);
  native.view = models({});
  native.ready = false;
  const keyless = await render(ModelPickerStage, { profile: PROFILE });
  await shoot("keyless");
  await keyless.unmount();

  native.ready = true;
  native.hosted = true;
  native.view = keyed("anthropic/claude-sonnet-5-5");
  const screen = await render(ModelPickerStage, { profile: PROFILE });
  await screen.getByRole("button", { name: /Model: Claude Sonnet 5\.5/u }).click();
  await page.getByRole("tab", { name: "Zephium AI" }).click();
  await shoot("hosted");
});
