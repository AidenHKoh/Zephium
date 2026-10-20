<script lang="ts">
  import { AccountSetting01Icon, UserIcon } from "@hugeicons/core-free-icons";
  import * as m from "$shared/i18n/messages";
  import Icon from "$shared/ui/Icon";

  /**
   * The menu row onto Settings → Account, or onto signing in. Browse's and
   * Work's feet both draw it, so what it says is decided once.
   */
  let {
    signedIn,
    standing = null,
    tone = "calm",
    step = 0,
    compact = false,
    onclick,
  }: {
    signedIn: boolean;
    /** Plan and usage, or the email: what stands behind the account. */
    standing?: string | null;
    tone?: "calm" | "high" | "full";
    step?: number;
    compact?: boolean;
    onclick: () => void;
  } = $props();

  const label = $derived(signedIn ? m.account_menu() : m.ai_cloud_sign_in());
  const caption = $derived(compact ? null : standing);
</script>

<button
  type="button"
  role="menuitem"
  class="ui-menu-item account-row"
  data-compact={compact || undefined}
  style:--step={step}
  title={compact ? [label, standing].filter(Boolean).join(" · ") : undefined}
  aria-label={compact ? label : undefined}
  {onclick}
>
  <span class="ui-menu-icon"
    ><Icon icon={signedIn ? AccountSetting01Icon : UserIcon} size={16} /></span
  >
  {#if !compact}<span class="label">{label}</span>{/if}
  {#if caption}<span class="caption" data-tone={tone}>{caption}</span>{/if}
</button>

<style>
  .account-row {
    inline-size: 100%;
    min-block-size: 32px;
    padding-block: 4px;
    border: 0;
    background: transparent;
    font: inherit;
    font-size: var(--text-body);
    text-align: start;
  }

  .account-row[data-compact] {
    justify-content: center;
    padding-inline: 0;
  }

  .label {
    white-space: nowrap;
  }

  /* A caption, not a keycap: it belongs to the row and says what stands behind it. */
  .caption {
    overflow: hidden;
    margin-inline-start: auto;
    padding-inline-start: 24px;
    color: var(--color-faint);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: color var(--motion-instant) var(--ease-smooth);
  }

  .caption[data-tone="high"] {
    color: var(--color-warning, var(--color-muted));
  }

  .caption[data-tone="full"] {
    color: var(--color-danger);
  }

  .account-row:hover .caption,
  .account-row:focus-visible .caption {
    color: var(--color-muted);
  }

  .account-row:hover,
  .account-row:focus-visible {
    outline: none;
    background: var(--row-active);
  }

  .account-row:active {
    background: var(--row-pressed);
  }

  /* Each row settles in behind the one below it, so the menu unfolds from its foot. */
  :global([data-open="true"]) .account-row {
    animation: account-foot-in var(--motion-slow) var(--ease-out) both;
    animation-delay: calc(var(--step) * 14ms);
  }

  /* stylelint-disable-next-line keyframes-name-pattern -- Svelte's global prefix. */
  @keyframes -global-account-foot-in {
    from {
      opacity: 0;
      translate: 0 6px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .caption {
      transition: none;
    }

    :global([data-open="true"]) .account-row {
      animation: none;
    }
  }
</style>
