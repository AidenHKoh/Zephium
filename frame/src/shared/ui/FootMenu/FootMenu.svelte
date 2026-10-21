<script lang="ts" module>
  import type { IconSvgElement } from "@hugeicons/svelte";
  import type { ProfileAvatarView } from "$shared/ui/ProfileAvatar";

  /** Display-only: one destination row, already labelled by the caller. */
  export type FootRow = {
    id: string;
    icon: IconSvgElement;
    label: string;
    keys?: string | null;
    /** Its panel is the one open now. */
    current?: boolean;
    run: () => void;
  };

  /** Display-only: a profile as the switcher lists it. */
  export type FootProfile = {
    id: string;
    name: string;
    view: ProfileAvatarView;
    current: boolean;
  };

  /** Display-only: the account as the card shows it. */
  export type FootAccount = {
    signedIn: boolean;
    email: string | null;
    plan: string | null;
    used: number | null;
    tone: "calm" | "high" | "full";
    mark: ProfileAvatarView | null;
  };
</script>

<script lang="ts">
  import {
    Add01Icon,
    IncognitoIcon,
    Tick02Icon,
    UserMultiple02Icon,
  } from "@hugeicons/core-free-icons";
  import * as m from "$shared/i18n/messages";
  import AccountCard from "$shared/ui/AccountCard";
  import Icon from "$shared/ui/Icon";
  import ProfileAvatar from "$shared/ui/ProfileAvatar";
  import "./panel.css";

  let {
    account,
    profiles,
    privateWindow,
    groups,
    compact = false,
    onaccount,
    onswitch,
    onnewprofile,
    onmanage,
  }: {
    /** Null hides the card: no service, or a private window. */
    account: FootAccount | null;
    /** Null hides the switcher, as a private window does. */
    profiles: FootProfile[] | null;
    /** Null where a private window cannot be opened from here. */
    privateWindow: { keys: string | null; run: () => void } | null;
    /** Destinations, a hairline between groups; Settings closes the last. */
    groups: FootRow[][];
    compact?: boolean;
    onaccount: () => void;
    onswitch: (id: string) => void;
    onnewprofile: () => void;
    onmanage: () => void;
  } = $props();

  const named = (label: string, keys: string | null | undefined) =>
    keys ? `${label}  ${keys}` : label;
  const visible = $derived(groups.filter((group) => group.length > 0));
</script>

{#snippet row(
  icon: IconSvgElement,
  label: string,
  keys: string | null | undefined,
  run: () => void,
  extra: { tool?: boolean; current?: boolean } = {},
)}
  <button
    type="button"
    role="menuitem"
    class={["ui-menu-item", "foot-row", extra.tool && "foot-tool"]}
    title={compact ? named(label, keys) : undefined}
    aria-label={compact ? label : undefined}
    aria-current={extra.current || undefined}
    aria-keyshortcuts={keys ?? undefined}
    onclick={run}
  >
    <span class="ui-menu-icon foot-lead"><Icon {icon} size={16} /></span>
    {#if !compact}<span class="foot-label">{label}</span>{#if keys}<span
          class="foot-keys"
          aria-hidden="true">{keys}</span
        >{/if}{/if}
  </button>
{/snippet}

<!--
  The one menu at the foot of Browse and of Work, top to bottom: the Zephium
  account, the profiles this window can browse as, a private window, then the
  destinations, Settings last and nearest the trigger. Every row shares one
  height and one leading column, so the eye runs straight down it.
-->
<div class="foot-menu" data-compact={compact || undefined} role="presentation">
  {#if account}<div class="foot-section foot-account" role="presentation">
      <AccountCard
        signedIn={account.signedIn}
        email={account.email}
        plan={account.plan}
        used={account.used}
        tone={account.tone}
        mark={account.mark}
        {compact}
        onclick={onaccount}
      />
    </div>{/if}
  {#if profiles}{#if account}<div class="foot-rule" role="separator"></div>{/if}
    <div class="foot-section" role="group" aria-label={m.menu_profiles()}>
      {#if !compact}<div class="foot-heading" aria-hidden="true">{m.menu_profiles()}</div>{/if}
      {#each profiles as profile (profile.id)}
        <button
          type="button"
          role="menuitemradio"
          class="ui-menu-item foot-row foot-profile"
          aria-checked={profile.current}
          aria-label={profile.name}
          title={compact ? profile.name : undefined}
          onclick={() => {
            if (!profile.current) onswitch(profile.id);
          }}
        >
          <span class="foot-lead"
            ><ProfileAvatar view={profile.view} size={compact ? 24 : 20} /></span
          >
          {#if !compact}<span class="foot-label">{profile.name}</span>{#if profile.current}<span
                class="foot-check"
                aria-hidden="true"><Icon icon={Tick02Icon} size={14} strokeWidth={2} /></span
              >{/if}{/if}
        </button>
      {/each}
      {@render row(Add01Icon, m.profiles_menu_new(), null, onnewprofile)}
      {@render row(UserMultiple02Icon, m.browse_profiles_manage(), null, onmanage)}
    </div>{/if}
  {#if privateWindow}{#if account || profiles}<div class="foot-rule" role="separator"></div>{/if}
    <div class="foot-section" role="presentation">
      {@render row(
        IncognitoIcon,
        m.browse_incognito_window(),
        privateWindow.keys,
        privateWindow.run,
      )}
    </div>{/if}
  {#each visible as group, at (at)}
    {#if at > 0 || account || profiles || privateWindow}<div
        class="foot-rule"
        role="separator"
      ></div>{/if}
    <div class="foot-section" role="presentation">
      {#each group as item (item.id)}
        {@render row(item.icon, item.label, item.keys, item.run, {
          tool: true,
          current: item.current,
        })}
      {/each}
    </div>
  {/each}
</div>

<style>
  .foot-menu {
    --foot-row: 32px;

    display: flex;
    container-type: inline-size;
    flex-direction: column;
    min-inline-size: 0;
  }

  .foot-section {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .foot-rule {
    flex: none;
    block-size: 1px;
    margin: 5px var(--menu-item-inset);
    background: var(--color-border);
  }

  /* A caption, not a row: it names the rows under it and takes no focus. */
  .foot-heading {
    padding: 4px var(--menu-item-inset) 3px;
    color: var(--color-faint);
    font-size: var(--text-caption);
    font-weight: 600;
    letter-spacing: 0.02em;
    line-height: 14px;
    user-select: none;
  }

  .foot-row {
    inline-size: 100%;
    min-block-size: var(--foot-row);
    padding-block: 4px;
    border: 0;
    background: transparent;
    font: inherit;
    font-size: var(--text-body);
    text-align: start;
  }

  .foot-row:hover,
  .foot-row:focus-visible {
    background: var(--row-active);
  }

  .foot-row:active {
    background: var(--row-pressed);
  }

  /* One leading column for glyphs and faces alike, so every label starts on
     the same line whatever stands before it. */
  .foot-lead {
    display: grid;
    flex: none;
    inline-size: 20px;
    place-items: center;
  }

  .foot-row:is(:hover, :focus-visible) .ui-menu-icon {
    color: var(--color-text);
  }

  .foot-row[aria-current="true"] .ui-menu-icon {
    color: var(--color-accent);
  }

  .foot-label {
    flex: 1;
    overflow: hidden;
    min-inline-size: 0;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .foot-check {
    display: grid;
    flex: none;
    color: var(--color-text);
    place-items: center;
  }

  /* A caption, not a keycap: the key belongs to the row and steps forward
     with it when the row is pointed at. */
  .foot-keys {
    flex: none;
    padding-inline-start: 12px;
    color: var(--color-faint);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
    transition: color var(--motion-instant) var(--ease-smooth);
  }

  .foot-row:is(:hover, :focus-visible) .foot-keys {
    color: var(--color-muted);
  }

  /* In a narrow column the names keep the room and the keys give it up. */
  @container (inline-size < 236px) {
    .foot-keys {
      display: none;
    }
  }

  /* At rail width: glyphs and faces centred in the same square plates, each
     named by its tooltip; the heading gives way to the hairline above. */
  .foot-menu[data-compact] .foot-row {
    justify-content: center;
    padding-inline: 0;
  }

  .foot-menu[data-compact] .foot-lead {
    inline-size: 24px;
  }

  .foot-menu[data-compact] .foot-profile[aria-checked="true"] .foot-lead :global(.profile-avatar) {
    box-shadow:
      0 0 0 1.5px var(--color-float),
      0 0 0 3px var(--color-muted);
  }

  .foot-menu[data-compact] .foot-rule {
    margin-inline: 8px;
  }

  @media (forced-colors: active) {
    .foot-rule {
      background: CanvasText;
    }

    .foot-row:is(:hover, :focus-visible) {
      background: Highlight;
      color: HighlightText;
    }
  }
</style>
