<script lang="ts">
  import { UserIcon } from "@hugeicons/core-free-icons";
  import * as m from "$shared/i18n/messages";
  import Icon from "$shared/ui/Icon";
  import ProfileAvatar, { type ProfileAvatarView } from "$shared/ui/ProfileAvatar";

  /**
   * The account at the foot of Browse and Work: apart from the profile above
   * it, since the account is the same in every profile. Both feet draw it, so
   * what it says is decided once.
   */
  let {
    signedIn,
    email = null,
    plan = null,
    used = null,
    tone = "calm",
    mark = null,
    step = 0,
    compact = false,
    onclick,
  }: {
    signedIn: boolean;
    email?: string | null;
    plan?: string | null;
    /** Whole percent of the included usage spent, when known. */
    used?: number | null;
    tone?: "calm" | "high" | "full";
    /** The account's initials as a drawn avatar. */
    mark?: ProfileAvatarView | null;
    step?: number;
    compact?: boolean;
    onclick: () => void;
  } = $props();

  const standing = $derived(
    [plan, used != null ? m.ai_cloud_used({ percent: used }) : null].filter(Boolean).join(" · "),
  );
  const label = $derived(signedIn ? m.account_menu() : m.settings_identity_sign_in());
  const hint = $derived(signedIn ? [email, standing].filter(Boolean).join(" · ") : null);
  const fill = $derived(used == null ? null : Math.min(100, Math.max(0, used)));
</script>

<button
  type="button"
  role="menuitem"
  class="account-card"
  data-signed-in={signedIn || undefined}
  data-compact={compact || undefined}
  data-tone={tone}
  style:--step={step}
  title={compact ? [label, hint].filter(Boolean).join(" · ") : undefined}
  aria-label={compact || !signedIn ? label : `${label} — ${email ?? ""}`}
  {onclick}
>
  {#if !signedIn}
    {#if compact}<Icon icon={UserIcon} size={16} />{:else}<span class="out">{label}</span>{/if}
  {:else}
    {#if mark}<ProfileAvatar view={mark} size={compact ? 28 : 30} />{/if}
    {#if !compact}<span class="text">
        <span class="email">{email}</span>
        {#if standing}<span class="standing">{standing}</span>{/if}
        {#if fill != null}<span class="bar" aria-hidden="true"
            ><span style:inline-size="{fill}%"></span></span
          >{/if}
      </span>{/if}
  {/if}
</button>

<style>
  .account-card {
    display: flex;
    align-items: center;
    gap: 10px;
    inline-size: 100%;
    box-sizing: border-box;
    min-block-size: 36px;
    margin-block: 2px;
    padding: 6px 10px;
    border: 0;
    border-radius: calc(var(--radius-menu) - var(--menu-pad));
    background: var(--color-fill);
    box-shadow: var(--shine-top);
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-body);
    text-align: start;
    cursor: default;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      scale var(--motion-base) var(--ease-spring);
  }

  .account-card:hover,
  .account-card:focus-visible {
    outline: none;
    background: var(--row-active);
  }

  .account-card:focus-visible {
    box-shadow: inset 0 0 0 1.5px var(--color-text);
  }

  .account-card:active {
    scale: 0.985;
    background: var(--row-pressed);
  }

  .account-card:not([data-signed-in]) {
    justify-content: center;
    min-block-size: 34px;
  }

  .out {
    font-weight: 600;
    letter-spacing: -0.005em;
  }

  .account-card[data-compact] {
    justify-content: center;
    padding-inline: 0;
  }

  .text {
    display: grid;
    flex: 1;
    gap: 1px;
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
    overflow: hidden;
    color: var(--color-faint);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: color var(--motion-instant) var(--ease-smooth);
  }

  .account-card:hover .standing,
  .account-card:focus-visible .standing {
    color: var(--color-muted);
  }

  .account-card[data-tone="high"] .standing {
    color: var(--color-warning, var(--color-muted));
  }

  .account-card[data-tone="full"] .standing {
    color: var(--color-danger);
  }

  .bar {
    overflow: hidden;
    block-size: 3px;
    margin-block-start: 5px;
    border-radius: var(--radius-capsule);
    background: var(--color-track);
  }

  .bar span {
    display: block;
    block-size: 100%;
    border-radius: inherit;
    background: var(--color-text);
    transition: inline-size var(--motion-slow) var(--ease-out);
  }

  .account-card[data-tone="high"] .bar span {
    background: var(--color-warning);
  }

  .account-card[data-tone="full"] .bar span {
    background: var(--color-danger);
  }

  /* Settles in behind the rows below it, so the menu unfolds from its foot. */
  :global([data-open="true"]) .account-card {
    animation: account-card-in var(--motion-slow) var(--ease-out) both;
    animation-delay: calc(var(--step) * 14ms);
  }

  /* stylelint-disable-next-line keyframes-name-pattern -- Svelte's global prefix. */
  @keyframes -global-account-card-in {
    from {
      opacity: 0;
      translate: 0 6px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .account-card,
    .standing,
    .bar span {
      transition: none;
    }

    :global([data-open="true"]) .account-card {
      animation: none;
    }
  }
</style>
