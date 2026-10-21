import { expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import { render } from "vitest-browser-svelte";
import "$styles/global.css";
import ToolShelf from "../components/ToolShelf.svelte";

const native = vi.hoisted(() => ({
  run: vi.fn(async () => ({ accepted: true, operation_id: null })),
}));

vi.mock("$shared/platform", () => ({ IS_MAC: false, IS_WINDOWS: true }));
vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  return mockBindings({
    runCommand: native.run,
    profilesGet: async () => ({ status: "ok" as const, data: null }),
    settingGet: async () => null,
    sidebarSetWidth: vi.fn(async () => undefined),
  });
});

test("with no menu bar, Help stands beneath Settings and opens the Help menu", async () => {
  await page.viewport(1200, 800);
  const screen = await render(ToolShelf);
  screen.container.style.paddingBlockStart = "480px";
  await screen.getByRole("button", { name: /^Tools/ }).click();
  const rows = [...screen.container.querySelectorAll<HTMLElement>(".foot-tool")].map((item) =>
    item.querySelector(".foot-label")?.textContent?.trim(),
  );
  expect(rows.slice(-2)).toEqual(["Settings", "Help"]);
  await screen.getByRole("menuitem", { name: "Help", exact: true }).click();
  expect(native.run).toHaveBeenLastCalledWith("help.menu");
});
