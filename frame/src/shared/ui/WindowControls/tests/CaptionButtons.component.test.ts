import { afterEach, expect, test, vi } from "vitest";
import { render } from "vitest-browser-svelte";
import CaptionButtons from "../CaptionButtons.svelte";

const window = vi.hoisted(() => ({
  minimize: vi.fn(async () => {}),
  toggleMaximize: vi.fn(async () => {}),
  close: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => window }));

const push = (state: { maximized: boolean; focused: boolean }) => {
  (globalThis as { __zephiumWindow?: unknown }).__zephiumWindow = state;
  globalThis.dispatchEvent(new CustomEvent("zephium:window-state"));
};

afterEach(() => {
  delete (globalThis as { __zephiumWindow?: unknown }).__zephiumWindow;
  vi.clearAllMocks();
});

test("the middle button follows the state native pushes", async () => {
  const screen = await render(CaptionButtons);
  await expect.element(screen.getByRole("button", { name: "Maximize" })).toBeVisible();
  push({ maximized: true, focused: true });
  await expect.element(screen.getByRole("button", { name: "Restore Down" })).toBeVisible();
  push({ maximized: false, focused: false });
  await expect.element(screen.getByRole("button", { name: "Maximize" })).toBeVisible();
});

test("it starts from the state native pushed before it was shown", async () => {
  push({ maximized: true, focused: true });
  const screen = await render(CaptionButtons);
  await expect.element(screen.getByRole("button", { name: "Restore Down" })).toBeVisible();
});

test("the buttons act on the window", async () => {
  const screen = await render(CaptionButtons);
  await screen.getByRole("button", { name: "Minimize" }).click();
  await screen.getByRole("button", { name: "Maximize" }).click();
  await screen.getByRole("button", { name: "Close" }).click();
  expect(window.minimize).toHaveBeenCalledOnce();
  expect(window.toggleMaximize).toHaveBeenCalledOnce();
  expect(window.close).toHaveBeenCalledOnce();
});
