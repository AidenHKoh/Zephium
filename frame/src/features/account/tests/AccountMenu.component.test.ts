import { expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import { render } from "vitest-browser-svelte";
import type { AccountV1, ProfileEntryView, ProfilesView } from "$shared/ipc/bindings";
import { accountFixture, usageFixture } from "$shared/testing/account";
import AccountMenuStage from "./AccountMenuStage.svelte";

const native = vi.hoisted(() => ({
  account: null as unknown as AccountV1,
  profiles: null as unknown as ProfilesView,
  command: vi.fn(async (_id: string) => ({ operation_id: "op", accepted: true })),
  switch: vi.fn(async (_id: string) => ({ status: "ok", data: null })),
}));

vi.mock("$shared/ipc/bindings", async () => {
  const { mockBindings } = await import("$shared/testing/bindings");
  return mockBindings({
    accountGet: vi.fn(async () => ({ account: native.account, fault: null, retry_after: null })),
    profilesGet: vi.fn(async () => ({ status: "ok" as const, data: native.profiles })),
    profilesSwitch: native.switch as never,
    runCommand: native.command as never,
  });
});

vi.mock("$domain/keymap", () => ({
  keymap: {
    all: () => [
      { id: "browser.settings", accelerator: "CmdOrCtrl+," },
      { id: "window.newPrivate", accelerator: "CmdOrCtrl+Shift+N" },
    ],
  },
}));

const ID = (n: number) => `01J${String(n).padStart(23, "0")}`;
const entry = (n: number, name: string, color: number): ProfileEntryView => ({
  id: ID(n),
  name,
  kind: n === 1 ? "default" : "named",
  avatar: { style: "initials", color, symbol: 0, revision: 1 },
});
const solo = [entry(1, "Ada Lovelace", 1)];
const three = [solo[0]!, entry(2, "Work", 4), entry(3, "Side Project", 8)];

const shots = "../../../../../target/look/account-menu";
const settle = (ms = 500) => new Promise((done) => setTimeout(done, ms));

async function shoot(name: string) {
  for (const theme of ["dark", "light"]) {
    document.documentElement.dataset.theme = theme;
    await settle();
    await page.screenshot({ path: `${shots}/${name}-${theme}.png` });
  }
  document.documentElement.dataset.theme = "dark";
}

async function open(
  account: AccountV1,
  profiles: ProfileEntryView[],
  name = "Account",
  privately = false,
) {
  native.account = account;
  native.profiles = { current: ID(1), private: privately, profiles };
  const screen = await render(AccountMenuStage);
  await settle(200);
  await screen.getByRole("button", { name, exact: true }).click();
  await expect.element(page.getByRole("menuitem", { name: /^Settings/u })).toBeVisible();
  await expect.poll(() => document.querySelector<HTMLElement>(".work-menu")?.inert).toBe(false);
  return screen;
}

const signedIn = accountFixture({
  signed_in: true,
  email: "ada@example.com",
  plan: "pro",
  usage: usageFixture(),
});

const label = (item: Element) =>
  item.getAttribute("aria-label") ?? item.querySelector(".foot-label")?.textContent?.trim();

test("the menu leads with the account, then profiles, a private window, models and settings", async () => {
  await page.viewport(520, 680);
  native.command.mockClear();
  const screen = await open(signedIn, three);

  const items = [...document.querySelectorAll(".work-menu [role^='menuitem']")];
  expect(items.map(label)).toEqual([
    "Account — ada@example.com",
    "Ada Lovelace",
    "Work",
    "Side Project",
    "New Profile…",
    "Manage Profiles…",
    "New Incognito Window",
    "Models",
    "Settings",
  ]);
  await expect
    .element(page.getByRole("menuitem", { name: /^Account/u }))
    .toHaveTextContent(/ada@example\.com.*Pro.*42% used/u);
  await expect
    .element(page.getByRole("menuitemradio", { name: "Ada Lovelace" }))
    .toHaveAttribute("aria-checked", "true");
  await expect
    .element(page.getByRole("menuitemradio", { name: "Work" }))
    .toHaveAttribute("aria-checked", "false");
  expect(page.getByText("History").query()).toBeNull();
  expect(page.getByText("Downloads").query()).toBeNull();

  const trigger = screen.getByRole("button", { name: "Account", exact: true });
  for (const [name, command] of [
    [/^Account/u, "settings.account"],
    ["Manage Profiles…", "settings.profiles"],
    ["New Profile…", "settings.profiles"],
    ["New Incognito Window", "window.newPrivate"],
    ["Models", "settings.ai"],
    [/^Settings/u, "browser.settings"],
  ] as const) {
    await page.getByRole("menuitem", { name }).click();
    expect(native.command).toHaveBeenLastCalledWith(command);
    await trigger.click();
  }
});

test("another profile is one click away, and the current one stays put", async () => {
  await page.viewport(520, 680);
  native.switch.mockClear();
  const screen = await open(signedIn, three);
  await page.getByRole("menuitemradio", { name: "Side Project" }).click();
  expect(native.switch).toHaveBeenCalledWith(ID(3));
  await screen.getByRole("button", { name: "Account", exact: true }).click();
  await page.getByRole("menuitemradio", { name: "Ada Lovelace" }).click();
  expect(native.switch).toHaveBeenCalledTimes(1);
});

test("signed out offers one way in, and a private window shows neither account nor profiles", async () => {
  await page.viewport(520, 680);
  const out = await open(accountFixture(), solo);
  await expect
    .element(page.getByRole("menuitem", { name: "Sign in to Zephium", exact: true }))
    .toBeVisible();
  expect(page.getByRole("menuitem", { name: /^Account/u }).query()).toBeNull();
  await out.unmount();

  await open(signedIn, three, "Account", true);
  expect(page.getByRole("menuitem", { name: /^Account|^Sign in/u }).query()).toBeNull();
  expect(page.getByRole("menuitemradio").query()).toBeNull();
  expect(page.getByRole("menuitem", { name: "New Incognito Window" }).query()).toBeNull();
});

test("the menu is part of the keyboard", async () => {
  await page.viewport(520, 680);
  const { userEvent } = await import("vitest/browser");
  await open(signedIn, three);
  const account = page.getByRole("menuitem", { name: /^Account/u });
  await expect.element(account).toHaveFocus();
  await userEvent.keyboard("{ArrowDown}");
  await expect.element(page.getByRole("menuitemradio", { name: "Ada Lovelace" })).toHaveFocus();
  await userEvent.keyboard("{ArrowDown}{ArrowDown}");
  await expect.element(page.getByRole("menuitemradio", { name: "Side Project" })).toHaveFocus();
  await userEvent.keyboard("{End}");
  await expect.element(page.getByRole("menuitem", { name: /^Settings/u })).toHaveFocus();
  await userEvent.keyboard("{ArrowDown}");
  await expect.element(account).toHaveFocus();
  await userEvent.keyboard("{ArrowUp}");
  await expect.element(page.getByRole("menuitem", { name: /^Settings/u })).toHaveFocus();
  native.switch.mockClear();
  await userEvent.keyboard("{Home}{ArrowDown}{ArrowDown}{Enter}");
  expect(native.switch).toHaveBeenCalledWith(ID(2));
  await expect
    .element(page.getByRole("button", { name: "Account", exact: true }))
    .toHaveAttribute("aria-expanded", "false");
  await page.getByRole("button", { name: "Account", exact: true }).click();
  await userEvent.keyboard("{Escape}");
  await expect
    .element(page.getByRole("button", { name: "Account", exact: true }))
    .toHaveAttribute("aria-expanded", "false");
});

test(
  "what the menu looks like, signed in and out, with one profile and three",
  { timeout: 90_000 },
  async () => {
    await page.viewport(520, 680);
    for (const [name, account, profiles] of [
      ["in-three", signedIn, three],
      ["in-one", signedIn, solo],
      ["out-three", accountFixture(), three],
      ["out-one", accountFixture(), solo],
      [
        "usage-high",
        accountFixture({
          signed_in: true,
          email: "ada@example.com",
          plan: "pro",
          usage: usageFixture({ used_percent: 91 }),
        }),
        three,
      ],
      [
        "usage-full",
        accountFixture({
          signed_in: true,
          email: "ada@example.com",
          plan: "pro",
          usage: usageFixture({ used_percent: 100 }),
        }),
        three,
      ],
    ] as const) {
      const screen = await open(account, [...profiles]);
      await shoot(name);
      await screen.unmount();
    }
  },
);
