import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import { render } from "vitest-browser-svelte";
import "$styles/global.css";
import { emitNativeEvent } from "$shared/testing/native-events";
import { revision, spaceFixture, tabFixture } from "$shared/testing/fixtures";
import { tabs } from "$domain/tabs";
import { uiCommands } from "$domain/ui-commands";
import { surface } from "$domain/surface";
import * as sidebar from "$session/sidebar-mode.svelte";
import TabRail from "../components/TabRail.svelte";
import * as requests from "../lib/space-requests.svelte";

vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  const ok = async () => ({ accepted: true, operation_id: null });
  return mockBindings({
    tabsBootstrap: async () => {},
    sidebarSetWidth: async () => {},
    settingSet: ok,
    settingGet: async () => null,
    spacesStep: ok,
    keymapEntries: async () => [],
  });
});

afterEach(() => {
  sidebar.adoptMode("default");
  tabs.dispose();
  uiCommands.dispose();
  surface.dispose();
  requests.take();
});

async function rail() {
  await page.viewport(420, 700);
  await tabs.init();
  await uiCommands.init();
  const open = [tabFixture({ id: "a", title: "Alpha" })];
  emitNativeEvent("itemsChanged", {
    projection_revision: revision(7),
    profile: { id: "p", name: "Personal", kind: "default" },
    spaces: [spaceFixture("home", "Home"), spaceFixture("studio", "Studio")],
    active_space_id: "home",
    nodes: [{ id: "a", parent_id: null, section: "today", kind: { type: "tab", tab_id: "a" } }],
    tabs: open,
    active: "a",
    split_group: null,
  });
  sidebar.adoptMode("compact");
  return render(TabRail, { entries: open, onSelect: () => {} });
}

test("the rail has no spaces control of its own", async () => {
  const screen = await rail();
  expect(screen.container.querySelector(".rail-space")).toBeNull();
  expect(screen.container.querySelectorAll(".rail-new")).toHaveLength(1);
});

test("the full flow opens the column in place and hands the rail back when it is done", async () => {
  await rail();
  expect(sidebar.isCompact()).toBe(true);
  requests.openInColumn("list");
  expect(sidebar.isCompact()).toBe(false);
  const asked = requests.pending();
  expect(asked?.kind).toBe("list");
  // The flow dismissing is what releases the column.
  asked?.release?.();
  expect(sidebar.isCompact()).toBe(true);
});

test("New Space from the menu bar asks for it in the column, not in a panel beside the rail", async () => {
  await rail();
  emitNativeEvent("uiCommand", "space.new");
  await vi.waitFor(() => expect(requests.pending()?.kind).toBe("new"));
  expect(sidebar.isCompact()).toBe(false);
});
