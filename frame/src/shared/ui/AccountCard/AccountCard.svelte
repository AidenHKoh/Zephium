<script lang="ts">
  import { UserIcon } from "@hugeicons/core-free-icons";
  import * as m from "$shared/i18n/messages";
  import Icon from "$shared/ui/Icon";
  import ProfileAvatar, { type ProfileAvatarView } from "$shared/ui/ProfileAvatar";

  /**
   * The Zephium account at the head of the foot menu, apart from the profiles
   * below it, since the account is the same in every profile.
   */
  let {
    signedIn,
    email = null,
    plan = null,
    used = null,
    tone = "calm",
    mark = null,
    compact = false,
    onclick,
  }: {
    signedIn: boolean;
    email?: string | null;
    plan?: string | null;
    /** Percent of the included usage spent, when known. */
    used?: number | null;
    tone?: "calm" | "high" | "full";
    /** The account's initials as a drawn avatar. */
    mark?: ProfileAvatarView | null;
    compact?: boolean;
    onclick: () => void;
  } = $props();

  const percent = $derived(used == null ? null : Math.min(100, Math.max(0, Math.round(used))));
  const usage = $derived(percent == null ? null : m.ai_cloud_used({ percent }));
  const label = $derived(
    signedIn ? `${m.account_menu()} — ${email ?? ""}`.trimEnd() : m.menu_sign_in(),
  );
  const hint = $derived(
    signedIn ? [m.account_menu(), email, plan, usage].filter(Boolean).join(" · ") : label,
  );
</script>

{#if !signedIn}
  <button
    type="button"
    role="menuitem"
    class="account-card account-out"
    data-compact={compact || undefined}
    title={compact ? hint : undefined}
    aria-label={label}
    {onclick}
  >
    {#if compact}<Icon icon={UserIcon} size={16} />{:else}{label}{/if}
  </button>
{:else}
  <button
    type="button"
    role="menuitem"
    class="account-card"
    data-compact={compact || undefined}
    data-tone={tone}
    title={compact ? hint : undefined}
    aria-label={label}
    {onclick}
  >
    {#if mark}<ProfileAvatar view={mark} size={compact ? 24 : 32} />{/if}
    {#if compact}{#if tone !== "calm"}<span class="pip" aria-hidden="true"></span>{/if}
    {:else}<span class="text">
        <span class="email">{email}</span>
        {#if plan || usage}<span class="standing">
            {#if plan}<span class="plan">{plan}</span>{/if}
            {#if usage}<span class="used">{usage}</span>{/if}
          </span>{/if}
        {#if percent != null}<span class="bar" aria-hidden="true"
            ><span style:inline-size="{percent}%"></span></span
          >{/if}
      </span>{/if}
  </button>
{/if}

<style>
  .account-card {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    inline-size: 100%;
    box-sizing: border-box;
    padding: 10px;
    border: 0;
    border-radius: calc(var(--radius-menu) - var(--menu-pad));
    background: var(--color-fill);
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-body);
    line-height: 18px;
    text-align: start;
    cursor: default;
    outline: none;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      scale var(--motion-fast) var(--ease-out);
  }

  .account-card:hover,
  .account-card:focus-visible {
    background: var(--row-active);
  }

  .account-card:active {
    scale: 0.985;
    background: var(--row-pressed);
  }

  /* Signed out, the card is one call to action: the secondary plate, centred. */
  .account-out {
    justify-content: center;
    min-block-size: var(--foot-row, 32px);
    padding-block: 7px;
    background: var(--row-active);
    box-shadow: var(--shine-top);
    font-weight: 600;
    letter-spacing: -0.005em;
  }

  .account-out:hover,
  .account-out:focus-visible {
    background: var(--row-pressed);
  }

  .account-card:focus-visible {
    box-shadow:
      var(--shine-top),
      inset 0 0 0 1px var(--color-ring);
  }

  .text {
    display: grid;
    flex: 1;
    gap: 2px;
    min-inline-size: 0;
  }

  .email {
    overflow: hidden;
    font-weight: 600;
    letter-spacing: -0.005em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .standing {
    display: flex;
    align-items: baseline;
    gap: 8px;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 16px;
  }

  .plan {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .used {
    flex: none;
    margin-inline-start: auto;
    color: var(--color-faint);
    font-variant-numeric: tabular-nums;
  }

  .account-card[data-tone="high"] .used {
    color: var(--color-warning);
  }

  .account-card[data-tone="full"] .used {
    color: var(--color-danger);
  }

  .bar {
    overflow: hidden;
    block-size: 3px;
    margin-block-start: 4px;
    border-radius: var(--radius-capsule);
    background: var(--color-track);
  }

  .bar span {
    display: block;
    block-size: 100%;
    border-radius: inherit;
    background: var(--color-muted);
    transition: inline-size var(--motion-slow) var(--ease-out);
  }

  .account-card[data-tone="high"] .bar span {
    background: var(--color-warning);
  }

  .account-card[data-tone="full"] .bar span {
    background: var(--color-danger);
  }

  /* At rail width the card is the initials alone, its standing in the tooltip. */
  .account-card[data-compact] {
    justify-content: center;
    min-block-size: var(--foot-row, 32px);
    padding: 0;
    background: transparent;
    box-shadow: none;
  }

  .account-card[data-compact]:hover,
  .account-card[data-compact]:focus-visible {
    background: var(--row-active);
  }

  .pip {
    position: absolute;
    inset-block-start: 4px;
    inset-inline-end: 5px;
    inline-size: 7px;
    block-size: 7px;
    border-radius: 50%;
    background: var(--color-warning);
    box-shadow: 0 0 0 2px var(--color-float);
  }

  .account-card[data-tone="full"] .pip {
    background: var(--color-danger);
  }

  @media (forced-colors: active) {
    .account-card {
      border: 1px solid ButtonText;
    }
  }
</style>
