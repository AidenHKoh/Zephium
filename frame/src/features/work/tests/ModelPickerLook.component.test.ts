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
const settle = (ms = 600) => new Promise((done) => setTimeout(done, ms));
const keyed = (lead: string) =>
  models({
    keys: { anthropic: "valid", open_ai: "valid", deep_seek: "valid", google: "invalid" },
    lead,
  });

/** Glass is the menus' own material; solid is the opaque policy a Windows sidebar asks for. */
async function shoot(name: string, materials: readonly ("glass" | "solid")[] = ["glass"]) {
  for (const theme of ["dark", "light"]) {
    for (const material of materials) {
      document.documentElement.dataset.theme = theme;
      const menu = document.querySelector(".model-menu");
      if (material === "solid") menu?.setAttribute("data-menu-material", "opaque");
      else menu?.removeAttribute("data-menu-material");
      await settle();
      const suffix = materials.length > 1 ? `-${material}` : "";
      await page.screenshot({ path: `${shots}/${name}-${theme}${suffix}.png` });
    }
  }
  document.documentElement.dataset.theme = "dark";
  document.querySelector(".model-menu")?.removeAttribute("data-menu-material");
}

afterEach(() => {
  native.ready = true;
  native.hosted = false;
});

test("the hero, a preview, a provider without a key, and search", async () => {
  await page.viewport(1100, 700);
  native.view = keyed("anthropic/claude-opus-5-5");
  const screen = await render(ModelPickerStage, { profile: PROFILE });
  await shoot("rest");
  await screen.getByRole("button", { name: /Model: Claude Opus 5\.5/u }).click();
  await shoot("hero", ["glass", "solid"]);
  await userEvent.hover(page.getByRole("option", { name: /Claude Sonnet 5\.5/u }));
  await shoot("preview");
  await userEvent.hover(page.getByRole("option", { name: /Google/u }));
  await shoot("needs-key");
  await userEvent.hover(page.getByRole("option", { name: /DeepSeek V4 Pro/u }));
  await shoot("deepseek");
  await userEvent.unhover(page.getByRole("listbox"));
  await userEvent.keyboard("son");
  await shoot("search");
});

test("the effort at every level, from the keys", async () => {
  await page.viewport(1100, 700);
  native.view = { ...keyed("openai/gpt-6-luna"), effort: "low" };
  const screen = await render(ModelPickerStage, { profile: PROFILE });
  await screen.getByRole("button", { name: /Model: GPT-6 Luna/u }).click();
  const levels = ["low", "medium", "high", "xhigh", "max"];
  for (const [index, level] of levels.entries()) {
    if (index) await userEvent.keyboard("{ArrowRight}");
    await shoot(`effort-${level}`, level === "max" ? ["glass", "solid"] : ["glass"]);
  }
  await userEvent.keyboard("{Escape}");
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
  await userEvent.hover(page.getByRole("option", { name: /Zephium AI/u }));
  await shoot("hosted");
});
