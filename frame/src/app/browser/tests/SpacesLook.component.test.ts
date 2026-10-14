import { afterEach, test, vi } from "vitest";
import { page } from "vitest/browser";
import { render } from "vitest-browser-svelte";
import "$styles/global.css";
import type { SpaceView } from "$shared/ipc/bindings";
import { emitNativeEvent } from "$shared/testing/native-events";
import { revision, spaceFixture, tabFixture } from "$shared/testing/fixtures";
import { favicons } from "$domain/favicons";
import { surface } from "$domain/surface";
import { tabs } from "$domain/tabs";
import { preferences } from "$domain/preferences";
import { keymap } from "$domain/keymap";
import * as sidebar from "$session/sidebar-mode.svelte";
import Shell from "../Shell.svelte";
import { paint, settle, themed, wallpaper } from "./look-bench";

const native = vi.hoisted(() => ({ settings: { appearance: "light" } as Record<string, string> }));
vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  const ok = async () => ({ accepted: true, operation_id: null });
  return mockBindings({
    tabsBootstrap: async () => {},
    sidebarSetWidth: async () => {},
    sidebarResize: async () => false,
    sidebarResizeGuide: async () => true,
    settingGet: async (key: string) => native.settings[key] ?? null,
    settingSet: ok,
    spacesSwitch: ok,
    spacesStep: ok,
    profilesGet: async () => ({ status: "ok" as const, data: null }),
    keymapEntries: async () =>
      [
        ["tool.notes", "Cmd+Shift+E"],
        ["tool.tasks", "Cmd+Shift+K"],
        ["tool.time", "Cmd+Shift+U"],
        ["tool.history", "Cmd+Y"],
        ["tool.downloads", "Cmd+Shift+J"],
        ["tool.bookmarks", "Cmd+Shift+B"],
        ["browser.settings", "Cmd+,"],
        ["space.select.1", "Ctrl+1"],
        ["space.select.2", "Ctrl+2"],
        ["space.select.3", "Ctrl+3"],
        ["space.select.4", "Ctrl+4"],
      ].map(([id, accelerator]) => ({
        id: id!,
        title: id!,
        group: "window" as const,
        accelerator: accelerator!,
        default_accelerator: accelerator!,
        customizable: true,
        customized: false,
      })),
  });
});

const SPACES: SpaceView[] = [
  spaceFixture("home", "Personal", { icon: { kind: "symbol", index: 2 }, color: 1, tabs: 4 }),
  spaceFixture("work", "Studio", {
    icon: { kind: "emoji", text: "🎨" },
    color: 4,
    theme: { kind: "preset", id: "rose-pine" },
    tabs: 6,
  }),
  spaceFixture("read", "Reading", { icon: { kind: "symbol", index: 5 }, color: 6, tabs: 2 }),
  spaceFixture("side", "Side project", { icon: { kind: "symbol", index: 3 }, color: 8 }),
];

let projection = 50_000;
/** The bench's sidebar with real spaces; `empty` shows a space with no tabs. */
function spaced(active: string, empty = false) {
  const open = ["github", "spotify", "ddg", "slack"].map((key) =>
    tabFixture({
      id: key,
      title: key,
      url: `https://${key}.example/`,
      icon: { origin: `https://${key}.example`, revision: "a" },
    }),
  );
  const current = tabs.tabs();
  const kept = current.filter((tab) => ["telegram", "proton", "notion"].includes(tab.id));
  const titled = open.map((tab) => current.find((candidate) => candidate.id === tab.id) ?? tab);
  emitNativeEvent("itemsChanged", {
    projection_revision: revision((projection += 1)),
    profile: { id: "p", name: "Personal", kind: "default" },
    spaces: SPACES,
    active_space_id: active,
    nodes: [
      ...kept.map((tab) => ({
        id: tab.id,
        parent_id: null,
        section: "favorites" as const,
        kind: { type: "tab" as const, tab_id: tab.id },
      })),
      ...(empty ? [] : titled).map((tab) => ({
        id: tab.id,
        parent_id: null,
        section: "today" as const,
        kind: { type: "tab" as const, tab_id: tab.id },
      })),
    ],
    tabs: [...kept, ...(empty ? [] : titled)],
    active: empty ? null : "github",
    split_group: null,
  });
}

const shot = (name: string) =>
  page.screenshot({ path: `../../../../../target/look/spaces/${name}.png` });
const crop = (name: string) =>
  page.screenshot({
    path: `../../../../../target/look/spaces/${name}.png`,
    clip: { x: 0, y: 0, width: 300, height: 760 },
  } as Parameters<typeof page.screenshot>[0]);

afterEach(() => {
  sidebar.adoptMode("default");
  favicons.dispose();
  surface.dispose();
  tabs.dispose();
  preferences.dispose();
  themed("zephium");
  document.getElementById("wallpaper")?.remove();
});

for (const theme of ["light", "dark"] as const) {
  test(`spaces in the column, ${theme}`, async () => {
    await page.viewport(1100, 760);
    native.settings = { appearance: theme };
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.material = "liquid_glass";
    document.body.style.background = "transparent";
    await paint();
    await keymap.init();
    spaced("home");
    const screen = await render(Shell);
    wallpaper("violet", theme, theme === "light" ? "#ffffff" : "#0d1117");
    await settle();
    await shot(`${theme}-expanded`);
    await screen.getByRole("button", { name: /^Tools/ }).click();
    await settle();
    await shot(`${theme}-profile-menu`);
    await screen.getByRole("button", { name: /^Tools/ }).click();

    await screen.getByRole("button", { name: "Spaces — Personal" }).click();
    await settle();
    await crop(`${theme}-switcher`);
    await screen.getByRole("button", { name: "Edit Studio" }).click();
    await settle();
    await crop(`${theme}-editor`);
    document.body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));

    // The space with a theme of its own wears it over the whole window.
    themed("rose-pine");
    spaced("work");
    await settle();
    await shot(`${theme}-themed-space`);

    themed("zephium");
    spaced("side", true);
    await settle();
    await crop(`${theme}-empty`);

    spaced("home");
    sidebar.adoptMode("compact");
    await settle();
    await crop(`${theme}-rail`);
    sidebar.adoptMode("default");

    emitNativeEvent("uiCommand", "browser.settings");
    await settle();
    await settle();
    await shot(`${theme}-beside-settings`);
  });
}
