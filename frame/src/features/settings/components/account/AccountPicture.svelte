<script lang="ts">
  import { untrack } from "svelte";
  import { drawn, profiles, type AvatarDraft } from "$domain/profiles";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import AccountDialog from "./AccountDialog.svelte";
  import AvatarPicker, { type AvatarChoice } from "../profiles/AvatarPicker.svelte";

  /**
   * The picture is the browser profile's own, the one the sidebar shows, so
   * choosing it here writes what the profile sheet writes.
   */
  let { open = $bindable(false) }: { open?: boolean } = $props();

  const profile = $derived(profiles.current());
  let choice = $state<AvatarChoice>({ style: "initials", color: 1, symbol: 0, photo: null });
  let busy = $state(false);
  let failed = $state(false);
  const saved = $derived(profile ? drawn(profile).src : null);

  // Each opening starts from the profile as it is.
  $effect(() => {
    if (!open) return;
    untrack(() => {
      failed = false;
      const avatar = profile?.avatar;
      choice = avatar
        ? { style: avatar.style, color: avatar.color, symbol: avatar.symbol, photo: null }
        : { style: "initials", color: 1, symbol: 0, photo: null };
    });
  });

  const draft = (): AvatarDraft => ({
    style: choice.style,
    color: choice.color,
    symbol: choice.symbol,
    photo: choice.style === "photo" ? choice.photo : null,
  });

  /** A kept photo has no token; it is saved already and needs no write. */
  const changed = $derived(
    !profile ||
      profile.avatar.revision === 0 ||
      choice.photo !== null ||
      (choice.style !== "photo" &&
        (choice.style !== profile.avatar.style ||
          choice.color !== profile.avatar.color ||
          (choice.style === "symbol" && choice.symbol !== profile.avatar.symbol))),
  );

  async function submit() {
    if (busy || !profile) return;
    if (!changed) {
      open = false;
      return;
    }
    busy = true;
    failed = false;
    const ok = await profiles.setAvatar(profile.id, draft());
    busy = false;
    if (ok) open = false;
    else failed = true;
  }
</script>

<AccountDialog
  bind:open
  title={m.account_picture_title()}
  description={m.account_picture_desc()}
  width={460}
>
  <form
    class="body"
    onsubmit={(event) => {
      event.preventDefault();
      void submit();
    }}
  >
    <AvatarPicker bind:value={choice} name={profile?.name ?? ""} {saved} />
    {#if failed}<p class="failed" role="alert">{m.profiles_failed()}</p>{/if}
    <footer>
      <Button onclick={() => (open = false)} disabled={busy}>{m.action_cancel()}</Button>
      <Button type="submit" variant="primary" pending={busy}>{m.profiles_save()}</Button>
    </footer>
  </form>
</AccountDialog>

<style>
  .body {
    display: grid;
    gap: 22px;
  }

  .body :global(footer) {
    margin-block-start: 6px;
  }

  .failed {
    margin: 0;
    color: var(--color-danger);
    font-size: var(--text-label);
    line-height: 1.5;
  }
</style>
