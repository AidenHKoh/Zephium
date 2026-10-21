<script lang="ts">
  import { onMount } from "svelte";
  import { CubeIcon, Settings02Icon, UserIcon } from "@hugeicons/core-free-icons";
  import { accountFoot, accountSession } from "$domain/account";
  import { keymap } from "$domain/keymap";
  import { profiles } from "$domain/profiles";
  import { tabs } from "$domain/tabs";
  import { requestNew } from "$session/profile-sheet.svelte";
  import { commands } from "$shared/ipc/bindings";
  import * as m from "$shared/i18n/messages";
  import { acceleratorKeys } from "$shared/lib/accelerator";
  import { IS_MAC } from "$shared/platform";
  import Disclosure from "$shared/ui/Disclosure";
  import FootMenu, { type FootRow } from "$shared/ui/FootMenu";
  import Icon from "$shared/ui/Icon";
  import ProfileAvatar from "$shared/ui/ProfileAvatar";
  import "$shared/ui/Shine/shine.css";

  /** The profile's name until the window's own profile is known. */
  let { name = "" }: { name?: string } = $props();

  onMount(() => {
    void accountSession.retain();
    void profiles.retain();
    return () => {
      accountSession.release();
      profiles.release();
    };
  });

  const privately = $derived(profiles.inPrivate() || tabs.profile()?.kind === "incognito");
  const foot = accountFoot({ private: () => privately });
  const who = $derived(tabs.profile()?.name || profiles.current()?.name || name);
  const tone = $derived(foot.tone);

  function keysFor(command: string) {
    const accelerator = keymap.all().find((entry) => entry.id === command)?.accelerator;
    return accelerator ? acceleratorKeys(accelerator, IS_MAC).join(IS_MAC ? "" : "+") : null;
  }
  const run = (id: string) => void commands.runCommand(id).catch(() => {});
  const groups = $derived<FootRow[][]>([
    [{ id: "models", icon: CubeIcon, label: m.settings_models(), run: () => run("settings.ai") }],
    [
      {
        id: "settings",
        icon: Settings02Icon,
        label: m.command_browser_settings(),
        keys: keysFor("browser.settings"),
        run: () => run("browser.settings"),
      },
    ],
  ]);
</script>

<!--
  Work's foot is the same figure as Browse's: you, as a lit circle, opening onto
  the same menu, where Work's own destination, its models, stands in for
  Browse's tools. It is the rail's own light menu rather than the page-wide
  one, since it loads with Work into a chrome that pays for every dependency
  at launch.
-->
<Disclosure
  label={m.account_menu()}
  triggerTitle={who || undefined}
  menu
  triggerClass="work-case"
  panelClass="work-menu foot-menu-panel ui-shine-sheet sidebar-softens"
>
  {#snippet trigger()}{#if foot.face}<ProfileAvatar view={foot.face} size={36} />{:else}<Icon
        icon={UserIcon}
        size={16}
        strokeWidth={1.7}
      />{/if}{#if tone !== "calm"}<span class="pip" data-tone={tone} aria-hidden="true"
      ></span>{/if}{/snippet}
  <FootMenu
    account={foot.available && !privately ? foot : null}
    profiles={foot.profiles}
    privateWindow={privately
      ? null
      : { keys: keysFor("window.newPrivate"), run: () => run("window.newPrivate") }}
    {groups}
    onaccount={() => run("settings.account")}
    onswitch={(id) => void profiles.switchTo(id)}
    onnewprofile={() => {
      requestNew();
      run("settings.profiles");
    }}
    onmanage={() => run("settings.profiles")}
  />
</Disclosure>

<style>
  /* Up out of the trigger, clear of it and of the window's foot, the way the
     menu rises from Browse's: fully on screen at any rail or column width. */
  :global(.work-menu.ui-disclosure-panel) {
    bottom: calc(100% + 12px);
    inset-inline-start: 0;
    inline-size: min(256px, calc(100vw - 16px));
    min-inline-size: min(232px, calc(100vw - 16px));
    max-inline-size: calc(100vw - 16px);
    max-height: calc(100vh - 140px);
  }

  /* The profile as the tab list's own foot draws it: lit like the current tab,
     with light caught only along its top edge. */
  :global(.work-case) {
    position: relative;
    display: grid;
    inline-size: 36px;
    block-size: 36px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-capsule);
    background: var(--row-active);
    box-shadow: var(--shine-top);
    color: var(--color-label-secondary);
    place-items: center;
    cursor: default;
    pointer-events: auto;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out),
      scale var(--motion-slow) var(--ease-spring);
  }

  :global(.work-case:hover),
  :global(.work-case[data-state="open"]) {
    background: var(--row-pressed);
    color: var(--color-text);
  }

  :global(.work-case:active) {
    scale: 0.94;
    transition-duration: var(--motion-instant);
  }

  :global(.work-case:focus-visible) {
    outline: 2px solid var(--color-ring);
    outline-offset: 1px;
  }

  :global(.work-case .profile-avatar) {
    pointer-events: none;
  }

  :global(.work-case:has(.profile-avatar)::after) {
    position: absolute;
    border-radius: inherit;
    box-shadow: var(--shine-top);
    content: "";
    inset: 0;
    pointer-events: none;
  }

  /* The agent stops when the included usage runs out, so the figure says so from the rail. */
  .pip {
    position: absolute;
    inset-block-end: 0;
    inset-inline-end: 0;
    inline-size: 9px;
    block-size: 9px;
    border-radius: 50%;
    background: var(--color-warning, var(--color-soft-lemon));
    box-shadow: 0 0 0 2px var(--color-chrome);
  }

  .pip[data-tone="full"] {
    background: var(--color-danger);
  }

  @media (forced-colors: active) {
    :global(.work-case[data-state="open"]) {
      border: 1px solid Highlight;
    }
  }
</style>
