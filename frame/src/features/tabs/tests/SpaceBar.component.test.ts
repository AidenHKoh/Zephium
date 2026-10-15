import { afterEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import { render } from "vitest-browser-svelte";
import "$styles/global.css";
import type { ItemsState, SidebarNodeView } from "$shared/ipc/bindings";
import { emitNativeEvent } from "$shared/testing/native-events";
import { revision, spaceFixture, tabFixture } from "$shared/testing/fixtures";
import { tabs } from "$domain/tabs";
import { uiCommands } from "$domain/ui-commands";
import SidebarBody from "../components/SidebarBody.svelte";
import { sidebarTree } from "../lib/sidebar-model";

const native = vi.hoisted(() => ({
  open: vi.fn(),
  switchTo: vi.fn(),
  step: vi.fn(),
  create: vi.fn(),
  update: vi.fn(),
  remove: vi.fn(),
  reorder: vi.fn(),
}));

vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  const ok = async () => ({ accepted: true, operation_id: null });
  return mockBindings({
    tabsBootstrap: async () => {},
    tabsOpen: native.open.mockImplementation(ok),
    spacesSwitch: native.switchTo.mockImplementation(ok),
    spacesStep: native.step.mockImplementation(ok),
    spacesCreate: native.create.mockImplementation(ok),
    spacesUpdate: native.update.mockImplementation(ok),
    spacesDelete: native.remove.mockImplementation(ok),
    spacesReorder: native.reorder.mockImplementation(ok),
    keymapEntries: async () => [],
  });
});

const HOME = spaceFixture("home", "Home", {
  icon: { kind: "symbol", index: 2 },
  color: 1,
  tabs: 2,
});
const STUDIO = spaceFixture("studio", "Studio", {
  icon: { kind: "emoji", text: "🎨" },
  color: 4,
  theme: { kind: "preset", id: "nord" },
  tabs: 3,
});

let projection = 100;
function project(state: Partial<ItemsState>) {
  const open = [tabFixture({ id: "a", title: "Alpha" }), tabFixture({ id: "b", title: "Beta" })];
  const node = (id: string): SidebarNodeView => ({
    id,
    parent_id: null,
    section: "today",
    kind: { type: "tab", tab_id: id },
  });
  emitNativeEvent("itemsChanged", {
    projection_revision: revision((projection += 1)),
    profile: { id: "p", name: "Personal", kind: "default" },
    spaces: [HOME, STUDIO],
    active_space_id: "home",
    nodes: open.map((tab) => node(tab.id)),
    tabs: open,
    active: "a",
    split_group: null,
    ...state,
  });
}

async function mount(empty = false) {
  await page.viewport(420, 760);
  await tabs.init();
  await uiCommands.init();
  project(empty ? { nodes: [], tabs: [], active: null } : {});
  const tree = sidebarTree(tabs.sidebarNodes(), tabs.tabs());
  return render(SidebarBody, {
    pinned: tree.pinned,
    today: tree.today,
    splitting: false,
    onSelect: () => {},
  });
}

afterEach(() => {
  tabs.dispose();
  uiCommands.dispose();
  vi.clearAllMocks();
});

test("the row names the space and its plus opens a tab, with no pager beside them", async () => {
  const screen = await mount();
  await expect.element(screen.getByRole("button", { name: "Spaces — Home" })).toBeVisible();
  await screen.getByRole("button", { name: "New tab", exact: true }).click();
  expect(native.open).toHaveBeenCalledOnce();
  expect(screen.container.querySelector(".space-dots, .space-dot")).toBeNull();
  expect(screen.container.querySelectorAll(".space-bar button")).toHaveLength(2);
});

test("the switcher lists every space, checks the one shown and moves to another", async () => {
  const screen = await mount();
  await screen.getByRole("button", { name: "Spaces — Home" }).click();
  const rows = screen.getByRole("menuitemradio");
  await expect.element(rows.first()).toHaveAttribute("aria-checked", "true");
  await expect.element(rows.nth(1)).toHaveTextContent("Studio");
  await rows.nth(1).click();
  expect(native.switchTo).toHaveBeenCalledWith("studio");
});

test("a space changes as it is edited, and Done keeps it", async () => {
  const screen = await mount();
  await screen.getByRole("button", { name: "Spaces — Home" }).click();
  await screen.getByRole("button", { name: "Edit Studio" }).click();
  const look = {
    icon: { kind: "emoji", text: "🎨" },
    color: 4,
    theme: { kind: "preset", id: "nord" },
  };
  await screen.getByRole("textbox", { name: "Name" }).fill("Atelier");
  await expect.poll(() => native.update.mock.calls.at(-1)).toEqual(["studio", "Atelier", look]);
  await screen.getByRole("radio", { name: "Symbol 4" }).click();
  expect(native.update).toHaveBeenLastCalledWith("studio", "Atelier", {
    ...look,
    icon: { kind: "symbol", index: 3 },
  });
  await screen.getByRole("radio", { name: "No icon" }).click();
  await screen.getByRole("radio", { name: "Color 7" }).click();
  await screen.getByRole("radio", { name: "None", exact: true }).click();
  await expect
    .poll(() => native.update.mock.calls.at(-1))
    .toEqual(["studio", "Atelier", { icon: null, color: 6, theme: null }]);
  await screen.getByRole("button", { name: "Done" }).click();
  await expect
    .element(screen.getByRole("button", { name: "Spaces — Home" }))
    .toHaveAttribute("aria-expanded", "false");
});

test("Escape puts the space back as it was", async () => {
  const screen = await mount();
  await screen.getByRole("button", { name: "Spaces — Home" }).click();
  await screen.getByRole("button", { name: "Edit Studio" }).click();
  await screen.getByRole("radio", { name: "Color 7" }).click();
  await screen.getByRole("radio", { name: "No icon" }).click();
  await userEvent.keyboard("{Escape}");
  await expect
    .poll(() => native.update.mock.calls.at(-1))
    .toEqual([
      "studio",
      "Studio",
      { icon: { kind: "emoji", text: "🎨" }, color: 4, theme: { kind: "preset", id: "nord" } },
    ]);
  // Opened from the list, so Escape steps back to it.
  await expect.element(screen.getByRole("menuitemradio").first()).toBeVisible();
});

test("deleting asks first and says how many tabs will close", async () => {
  const screen = await mount();
  await screen.getByRole("button", { name: "Spaces — Home" }).click();
  await screen.getByRole("button", { name: "Edit Studio" }).click();
  await screen.getByRole("button", { name: "Delete…" }).click();
  await expect.element(screen.getByText("Its 3 tabs will close.")).toBeVisible();
  expect(native.remove).not.toHaveBeenCalled();
  await screen.getByRole("button", { name: "Delete Space" }).click();
  expect(native.remove).toHaveBeenCalledWith("studio");
});

test("New Space makes one at once, with no icon, and opens it to be named", async () => {
  const screen = await mount();
  await screen.getByRole("button", { name: "Spaces — Home" }).click();
  await screen.getByRole("menuitem", { name: "New Space" }).click();
  expect(native.create).toHaveBeenCalledWith("", { icon: null, color: 2, theme: null });
  project({
    spaces: [HOME, STUDIO, spaceFixture("made", "Space 3", { color: 2, tabs: 1 })],
    active_space_id: "made",
  });
  const name = screen.getByRole("textbox", { name: "Name" });
  await expect.element(name).toHaveValue("Space 3");
  await expect.element(name).toHaveFocus();
  await userEvent.keyboard("Reading{Enter}");
  await expect.poll(() => native.update.mock.calls.at(-1)?.[1]).toBe("Reading");

  emitNativeEvent("uiCommand", "space.edit");
  await expect.element(screen.getByRole("textbox", { name: "Name" })).toHaveValue("Space 3");
});

test("an empty space offers a new tab instead of a blank column", async () => {
  const screen = await mount(true);
  await expect.element(screen.getByText("Nothing open in Home")).toBeVisible();
  await screen.getByRole("button", { name: "New tab", exact: true }).nth(1).click();
  expect(native.open).toHaveBeenCalledOnce();
});

test("a sideways swipe over the list follows the fingers, then moves to the next space once", async () => {
  const screen = await mount();
  const list = screen.container.querySelector<HTMLElement>(".tab-scroller")!;
  const natural = true;
  const swipe = (deltaX: number, count = 6, deltaY = 0) => {
    for (let index = 0; index < count; index += 1) {
      const wheel = new WheelEvent("wheel", { deltaX, deltaY, bubbles: true });
      Object.defineProperty(wheel, "webkitDirectionInvertedFromDevice", { value: natural });
      list.dispatchEvent(wheel);
    }
  };
  // Fingers to the right on the first space: the list resists and comes back.
  swipe(-6, 4);
  expect(native.step).not.toHaveBeenCalled();
  expect(Number.parseFloat(list.style.translate)).toBeGreaterThan(0);
  await expect.poll(() => list.style.translate, { timeout: 1000 }).toBe("");

  // Mostly vertical: not ours.
  swipe(3, 6, 20);
  expect(list.style.translate).toBe("");
  await new Promise((resolve) => setTimeout(resolve, 200));

  swipe(30);
  expect(native.step).toHaveBeenCalledExactlyOnceWith(1);

  expect(list.style.translate).toBe("");
  expect(
    list
      .getAnimations()
      .some((animation) => (animation.effect as KeyframeEffect).getKeyframes().length === 2),
  ).toBe(true);
});

test("the fingers decide the direction whichever way the wheel is set to run", async () => {
  const screen = await mount();
  const list = screen.container.querySelector<HTMLElement>(".tab-scroller")!;
  for (let index = 0; index < 6; index += 1) {
    const wheel = new WheelEvent("wheel", { deltaX: -30, bubbles: true });
    Object.defineProperty(wheel, "webkitDirectionInvertedFromDevice", { value: false });
    list.dispatchEvent(wheel);
  }
  expect(native.step).toHaveBeenCalledExactlyOnceWith(1);
});

test("a private window has one space and no switcher", async () => {
  const screen = await mount();
  project({ profile: { id: "x", name: "Private", kind: "incognito" }, spaces: [HOME] });
  await expect.element(screen.getByRole("button", { name: "New tab" })).toBeVisible();
  expect(screen.container.querySelector(".space-bar")).toBeNull();
});
