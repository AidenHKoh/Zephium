import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import { render } from "vitest-browser-svelte";
import "$styles/global.css";
import { emitNativeEvent } from "$shared/testing/native-events";
import { revision, spaceFixture, tabFixture } from "$shared/testing/fixtures";
import { tabs } from "$domain/tabs";
import * as tools from "$session/tools.svelte";
import SpacePanel from "../components/SpacePanel.svelte";
import * as requests from "../lib/space-requests.svelte";

const native = vi.hoisted(() => ({
  switchTo: vi.fn(),
  width: vi.fn(),
}));

vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  const ok = async () => ({ accepted: true, operation_id: null });
  return mockBindings({
    tabsBootstrap: async () => {},
    spacesSwitch: native.switchTo.mockImplementation(ok),
    sidebarSetWidth: native.width.mockImplementation(async () => {}),
    keymapEntries: async () => [],
  });
});

const HOME = spaceFixture("home", "Home", { color: 1, tabs: 2 });
const STUDIO = spaceFixture("studio", "Studio", { color: 4, tabs: 3 });

async function mount() {
  await page.viewport(420, 760);
  await tabs.init();
  emitNativeEvent("itemsChanged", {
    projection_revision: revision(1),
    profile: { id: "p", name: "Personal", kind: "default" },
    spaces: [HOME, STUDIO],
    active_space_id: "home",
    nodes: [],
    tabs: [tabFixture({ id: "a", title: "Alpha" })],
    active: "a",
    split_group: null,
  });
  tools.openSpaces();
  return render(SpacePanel);
}

afterEach(() => {
  tools.close();
  tabs.dispose();
  vi.clearAllMocks();
});

test("the panel beside the rail widens the column, lists the spaces and closes on a choice", async () => {
  const screen = await mount();
  expect(tools.spacesOpen()).toBe(true);
  expect(native.width).toHaveBeenLastCalledWith(56 + 8 + 264, true, expect.any(Number));
  await expect
    .element(screen.getByRole("menuitemradio").first())
    .toHaveAttribute("aria-checked", "true");
  await screen.getByRole("menuitemradio").nth(1).click();
  expect(native.switchTo).toHaveBeenCalledWith("studio");
  expect(tools.spacesOpen()).toBe(false);
  expect(native.width).toHaveBeenLastCalledWith(240, true, expect.any(Number));
});

test("an edit asked for from a menu opens the editor of the space shown", async () => {
  requests.ask("edit");
  const screen = await mount();
  await expect.element(screen.getByRole("textbox", { name: "Name" })).toHaveValue("Home");
  await screen.getByRole("button", { name: "Done" }).click();
  expect(tools.spacesOpen()).toBe(false);
});

test("the close control gives the column back", async () => {
  const screen = await mount();
  await screen.getByRole("button", { name: "Close Spaces" }).click();
  expect(tools.spacesOpen()).toBe(false);
});
