import { expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import { render } from "vitest-browser-svelte";
import type { AccountV1, ProfileEntryView, ProfilesView } from "$shared/ipc/bindings";
import { accountFixture, usageFixture } from "$shared/testing/account";
import ProfileFootLook from "./ProfileFootLook.svelte";

const native = vi.hoisted(() => ({
  profiles: null as ProfilesView | null,
  account: null as AccountV1 | null,
}));
vi.mock("$shared/ipc/bindings", async () => {
  const known: Record<string, (...args: never[]) => Promise<unknown>> = {
    profilesGet: async () => ({ status: "ok", data: native.profiles }),
    accountGet: async () => ({ account: native.account, fault: null, retry_after: null }),
    runCommand: async () => ({ accepted: true, operation_id: null }),
    toolsMenuPopup: async () => true,
    settingGet: async () => null,
    keymapEntries: async () => [],
  };
  return {
    commands: new Proxy(known, {
      get: (target, key) =>
        typeof key === "string" ? (target[key] ?? (async () => null)) : undefined,
    }),
  };
});

vi.mock("$domain/keymap", () => ({
  keymap: {
    all: () => [
      { id: "browser.settings", accelerator: "CmdOrCtrl+," },
      { id: "window.newPrivate", accelerator: "CmdOrCtrl+Shift+N" },
      { id: "tool.notes", accelerator: "CmdOrCtrl+Shift+E" },
      { id: "tool.history", accelerator: "CmdOrCtrl+Y" },
      { id: "tool.downloads", accelerator: "CmdOrCtrl+Shift+J" },
    ],
  },
}));

const photo = vi.hoisted(() => ({ url: "" }));
vi.mock("$domain/profiles/avatar", async (original) => {
  const actual = await original<typeof import("$domain/profiles/avatar")>();
  return {
    ...actual,
    drawn: (profile: Parameters<typeof actual.drawn>[0]) => {
      const view = actual.drawn(profile);
      return view.style === "photo" ? { ...view, src: photo.url } : view;
    },
  };
});

function paintPhoto(): string {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 256;
  const context = canvas.getContext("2d")!;
  const sky = context.createLinearGradient(0, 0, 0, 256);
  sky.addColorStop(0, "#f6b48f");
  sky.addColorStop(0.55, "#e9877a");
  sky.addColorStop(1, "#5b4a6b");
  context.fillStyle = sky;
  context.fillRect(0, 0, 256, 256);
  context.fillStyle = "#ffe2b8";
  context.beginPath();
  context.arc(150, 120, 34, 0, Math.PI * 2);
  context.fill();
  context.fillStyle = "#3a3350";
  context.beginPath();
  context.moveTo(0, 190);
  context.quadraticCurveTo(80, 140, 150, 180);
  context.quadraticCurveTo(210, 210, 256, 170);
  context.lineTo(256, 256);
  context.lineTo(0, 256);
  context.fill();
  return canvas.toDataURL("image/png");
}

const ID = (n: number) => `01J${String(n).padStart(23, "0")}`;
const entry = (
  n: number,
  name: string,
  avatar: Partial<ProfileEntryView["avatar"]>,
): ProfileEntryView => ({
  id: ID(n),
  name,
  kind: n === 1 ? "default" : "named",
  avatar: { style: "initials", color: 1, symbol: 0, revision: 1, ...avatar },
});
const profiles = [
  entry(1, "Ada Lovelace", { style: "photo", revision: 3 }),
  entry(2, "Work", { style: "symbol", color: 1, symbol: 0 }),
  entry(3, "Side Project", { style: "initials", color: 8 }),
];

const settle = (ms = 400) => new Promise((done) => setTimeout(done, ms));
const shots = "../../../../../target/look/profiles";

async function shoot(target: HTMLElement, name: string) {
  for (const theme of ["dark", "light"]) {
    document.documentElement.dataset.theme = theme;
    await settle(200);
    await page.elementLocator(target).screenshot({ path: `${shots}/${name}-${theme}.png` });
  }
  document.documentElement.dataset.theme = "dark";
}

const signedIn = accountFixture({
  signed_in: true,
  email: "ada.lovelace@analytical-engine.example",
  plan: "pro",
  usage: usageFixture(),
});

/** Opens the full-width menu (first) or the rail's (last), settled. */
async function open(screen: Awaited<ReturnType<typeof render>>, which: "first" | "last") {
  const triggers = screen.getByRole("button", { name: /^Tools/u });
  await (which === "first" ? triggers.first() : triggers.last()).click();
  const stack = [...screen.container.querySelectorAll<HTMLElement>(".shelf-stack")].at(
    which === "first" ? 0 : -1,
  )!;
  await expect.poll(() => stack.inert).toBe(false);
  await expect.poll(() => getComputedStyle(stack).opacity).toBe("1");
  return stack;
}

test("the sidebar foot wears the profile's photo, symbol or nothing", async () => {
  photo.url = paintPhoto();
  await page.viewport(560, 740);
  native.account = accountFixture();
  native.profiles = { current: ID(1), private: false, profiles };
  const screen = await render(ProfileFootLook);
  await settle();
  await shoot(screen.container, "foot-photo");
  await screen.unmount();

  native.profiles = { current: ID(2), private: false, profiles };
  const symbol = await render(ProfileFootLook);
  await settle();
  await shoot(symbol.container, "foot-symbol");
  await symbol.unmount();

  native.profiles = {
    current: ID(1),
    private: false,
    profiles: [entry(1, "Personal", { revision: 0 }), ...profiles.slice(1)],
  };
  const unchosen = await render(ProfileFootLook);
  await settle();
  await shoot(unchosen.container, "foot-unchosen");
  await unchosen.unmount();
});

test(
  "the foot menu signed out and in, private, at rail width and in a narrow column",
  { timeout: 120_000 },
  async () => {
    photo.url = paintPhoto();
    await page.viewport(560, 740);
    native.profiles = { current: ID(1), private: false, profiles };

    native.account = accountFixture();
    let screen = await render(ProfileFootLook);
    await settle();
    await open(screen, "first");
    await shoot(screen.container, "menu-signed-out");
    await screen.unmount();

    native.account = signedIn;
    screen = await render(ProfileFootLook);
    await settle();
    const stack = await open(screen, "first");
    await expect.element(page.getByRole("menuitem", { name: /^Account/u })).toBeVisible();
    // Never past the column: the menu fits the sidebar it rises from.
    expect(stack.getBoundingClientRect().width).toBeLessThanOrEqual(260 - 8);
    await shoot(screen.container, "menu-signed-in");
    await screen.unmount();

    screen = await render(ProfileFootLook);
    await settle();
    const rail = await open(screen, "last");
    expect(rail.getBoundingClientRect().width).toBe(48);
    await shoot(screen.container, "menu-rail");
    await screen.unmount();

    native.account = accountFixture({
      signed_in: true,
      email: "ada@example.com",
      plan: "pro",
      usage: usageFixture({ used_percent: 91 }),
    });
    screen = await render(ProfileFootLook, { width: 200 });
    await settle();
    const narrow = await open(screen, "first");
    expect(narrow.getBoundingClientRect().width).toBeLessThanOrEqual(200 - 8);
    await shoot(screen.container, "menu-narrow");
    await screen.unmount();

    native.account = signedIn;
    native.profiles = { current: ID(1), private: true, profiles };
    screen = await render(ProfileFootLook);
    await settle();
    await open(screen, "first");
    expect(page.getByRole("menuitem", { name: /^Account/u }).query()).toBeNull();
    expect(page.getByRole("menuitemradio").query()).toBeNull();
    await shoot(screen.container, "menu-private");
    await screen.unmount();
  },
);
