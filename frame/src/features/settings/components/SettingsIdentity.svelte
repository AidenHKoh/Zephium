<script lang="ts">
  import { onMount } from "svelte";
  import { accountSession as session, initials, planName } from "$domain/account";
  import * as m from "$shared/i18n/messages";
  import { identityAvatar, profiles } from "$domain/profiles";
  import ProfileAvatar from "$shared/ui/ProfileAvatar";
  import * as settings from "../lib/settings-state.svelte";

  onMount(() => {
    void session.retain();
    void profiles.retain();
    return () => {
      session.release();
      profiles.release();
    };
  });

  let account = $derived(session.account());
  let signedIn = $derived(account?.signed_in === true);
  let current = $derived(settings.section() === "account" && !settings.query());
  let title = $derived(
    signedIn ? (account?.name ?? account?.email ?? "") : m.settings_identity_sign_in(),
  );
  let detail = $derived.by(() => {
    if (!signedIn) return m.settings_identity_sign_in_desc();
    const plan = planName(account?.plan);
    if (plan) return m.account_plan_line({ plan });
    return account?.name ? (account.email ?? "") : m.settings_identity_account();
  });
</script>

{#if !session.unavailable()}
  <!-- Holds its height while the first answer is read, so the list below
       never jumps. -->
  <div class="identity-slot">
    {#if account}<button
        type="button"
        class="settings-nav-item identity"
        data-plate
        aria-current={current ? "page" : undefined}
        onclick={() => settings.select("account")}
        >{#if signedIn}<ProfileAvatar
            view={identityAvatar(profiles.current(), initials(account.email, account.name))}
          />{:else}<img class="mark" src="/zephium-icon.png" alt="" />{/if}<span class="copy"
          ><strong>{title}</strong><small>{detail}</small></span
        ></button
      >{/if}
  </div>
{/if}

<style>
  .identity-slot {
    min-block-size: 52px;
    margin-block-end: 10px;
  }

  button.identity {
    gap: 11px;
    min-block-size: 52px;
    padding: 9px 10px;
  }

  .mark {
    flex: none;
    inline-size: 32px;
    block-size: 32px;
  }

  .copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-inline-size: 0;
  }

  strong,
  small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  strong {
    color: var(--color-text);
    font-size: 13px;
    font-weight: 550;
    line-height: 17px;
    letter-spacing: -0.01em;
  }

  small {
    color: var(--color-label-secondary);
    font-size: 11px;
    line-height: 15px;
  }
</style>
