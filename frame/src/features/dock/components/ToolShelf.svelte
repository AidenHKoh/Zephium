<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { onMount } from "svelte";
  import { HelpCircleIcon, Settings02Icon, UserIcon } from "@hugeicons/core-free-icons";
  import { commands } from "$shared/ipc/bindings";
  import * as tools from "$session/tools.svelte";
  import type { ToolKind } from "$session/tools.svelte";
  import { requestNew } from "$session/profile-sheet.svelte";
  import { accountFoot, accountSession } from "$domain/account";
  import { preferences } from "$domain/preferences";
  import { tabs } from "$domain/tabs";
  import { profiles } from "$domain/profiles";
  import Disclosure from "$shared/ui/Disclosure";
  import FootMenu, { type FootRow } from "$shared/ui/FootMenu";
  import Icon from "$shared/ui/Icon";
  import ProfileAvatar from "$shared/ui/ProfileAvatar";
  import { RECORD_TOOLS, SHELF_TOOLS, toolPresentation } from "../lib/dock-tools";
  import NoticePip from "./NoticePip.svelte";
  import { keymap } from "$domain/keymap";
  import { acceleratorKeys } from "$shared/lib/accelerator";
  import { IS_MAC, IS_WINDOWS } from "$shared/platform";

  let { compact = false }: { compact?: boolean } = $props();
  onMount(() => {
    void profiles.retain();
    void accountSession.retain();
    return () => {
      profiles.release();
      accountSession.release();
    };
  });
  const name = $derived(tabs.profile()?.name ?? profiles.current()?.name ?? "");
  const privately = $derived(profiles.inPrivate() || tabs.profile()?.kind === "incognito");
  const foot = accountFoot({ private: () => privately });
  // Each row says its key, so the menu teaches the way past itself.
  function keysFor(command: string) {
    const accelerator = keymap.all().find((entry) => entry.id === command)?.accelerator;
    return accelerator ? acceleratorKeys(accelerator, IS_MAC).join(IS_MAC ? "" : "+") : null;
  }
  const run = (id: string) => void commands.runCommand(id).catch(() => {});
  function pick(kind: ToolKind) {
    if (tools.activeTool() === kind) tools.close();
    else tools.open(kind);
  }
  // A tool whose feature is off is not offered here either.
  const groups = $derived.by((): FootRow[][] => {
    const toolRows = (kinds: readonly ToolKind[]) =>
      kinds.map((kind): FootRow => {
        const view = toolPresentation(kind);
        return {
          id: kind,
          icon: view.icon,
          label: view.label(),
          keys: keysFor(`tool.${kind}`),
          current: tools.activeTool() === kind,
          run: () => pick(kind),
        };
      });
    const settings: FootRow[] = [
      {
        id: "settings",
        icon: Settings02Icon,
        label: m.command_browser_settings(),
        keys: keysFor("browser.settings"),
        run: () => run("browser.settings"),
      },
    ];
    // Windows has no menu bar to hold Help, so it stands beneath Settings.
    if (IS_WINDOWS)
      settings.push({
        id: "help",
        icon: HelpCircleIcon,
        label: m.native_help(),
        run: () => run("help.menu"),
      });
    return [
      toolRows(SHELF_TOOLS.filter((kind) => preferences.enabled(kind))),
      toolRows(RECORD_TOOLS),
      settings,
    ];
  });
  function newProfile() {
    requestNew();
    run("settings.profiles");
  }
  // The full native menu stays one gesture away, where a Mac user reaches for
  // everything else a control can do.
  function nativeMenu(event: MouseEvent) {
    event.preventDefault();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    void commands.toolsMenuPopup(rect.left, rect.top);
  }
</script>

<!--
  The foot of the column is you: the profile this window browses as, opening
  onto the menu Work's foot opens too. A click opens it rising out of the
  avatar; nothing opens on hover. At rail width the menu is glyphs and faces,
  each named by its tooltip.
-->
<div
  class="shelf"
  data-compact={compact}
  data-morph="profile"
  role="presentation"
  oncontextmenu={nativeMenu}
>
  <Disclosure
    label={name ? `${m.dock_tools()} — ${name}` : m.dock_tools()}
    menu
    triggerClass={["case", tools.activeTool() !== null && "case-lit"].filter(Boolean).join(" ")}
    panelClass="shelf-stack foot-menu-panel sidebar-softens"
  >
    {#snippet trigger()}{#if foot.face}<ProfileAvatar
          view={foot.face}
          size={compact ? 36 : 40}
        />{:else}<Icon
          icon={UserIcon}
          size={compact ? 16 : 17}
          strokeWidth={1.7}
        />{/if}{#if compact}<NoticePip />{/if}{/snippet}
    <FootMenu
      account={foot.available && !privately ? foot : null}
      profiles={foot.profiles}
      privateWindow={privately
        ? null
        : { keys: keysFor("window.newPrivate"), run: () => run("window.newPrivate") }}
      {groups}
      {compact}
      onaccount={() => run("settings.account")}
      onswitch={(id) => void profiles.switchTo(id)}
      onnewprofile={newProfile}
      onmanage={() => run("settings.profiles")}
    />
  </Disclosure>
</div>

<style>
  .shelf {
    position: relative;
    z-index: 20;
    flex: none;
  }

  /* You, at the head of the row: lit the way the current tab is, with light
     caught only along its top edge, and round where every kept site is
     square, so it reads as yours rather than as one more site. */
  .shelf :global(.case) {
    position: relative;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    margin-inline: 2px;
    border: 0;
    border-radius: var(--radius-capsule);
    background: var(--row-active);
    box-shadow: var(--shine-top);
    color: var(--color-label-secondary);
    cursor: default;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out),
      scale var(--motion-slow) var(--ease-spring);
  }

  .shelf :global(.case:hover),
  .shelf :global(.case[data-state="open"]),
  .shelf :global(.case.case-lit) {
    background: var(--row-pressed);
    color: var(--color-text);
  }

  .shelf :global(.case:active) {
    scale: 0.94;
    transition-duration: var(--motion-instant);
  }

  /* A chosen picture fills the lit circle; its light still catches the top. */
  .shelf :global(.case .profile-avatar) {
    pointer-events: none;
  }

  .shelf :global(.case:has(.profile-avatar)::after) {
    position: absolute;
    border-radius: inherit;
    box-shadow: var(--shine-top);
    content: "";
    inset: 0;
    pointer-events: none;
  }

  /* The menu rises out of the case, clear of it, one width whatever it
     holds, and never wider than the column: a short window scrolls it. */
  .shelf :global(.shelf-stack) {
    bottom: calc(100% + 8px);
    width: min(256px, calc(var(--sidebar-width, 320px) - 8px));
    min-width: min(232px, calc(var(--sidebar-width, 320px) - 8px));
    max-height: calc(100vh - 120px);
  }

  /* At rail width: a square plate like every other rail item, and a menu of
     glyphs and faces just wide enough for them. */
  .shelf[data-compact="true"] :global(.case) {
    width: 36px;
    height: 36px;
    margin-inline: 0;
  }

  /* Centred on the 40px case: four points either side of it. The rail's marks
     and header lie behind it, so it is near-solid rather than glass. */
  .shelf[data-compact="true"] :global(.shelf-stack) {
    background: color-mix(in srgb, var(--color-float) 96%, transparent);
    inset-inline-start: -4px;
    min-width: 0;
    width: 48px;
  }

  @media (forced-colors: active) {
    .shelf :global(.case.case-lit) {
      border: 1px solid Highlight;
    }
  }
</style>
