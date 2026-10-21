import * as accountSession from "./account.svelte";
import { initials, planName, usageTone } from "./account-model";
import { drawn, identityAvatar, profiles } from "$domain/profiles";

/**
 * What the foot of Browse and Work shows: the profile's picture for the
 * trigger, and in the menu the account (the same in every profile) and the
 * profiles to switch between. One reading of both, so the two feet cannot drift. Call it during
 * component initialisation.
 */
export function accountFoot(options: { private?: () => boolean } = {}) {
  const account = $derived(accountSession.account());
  const signedIn = $derived(account?.signed_in === true);
  const used = $derived(signedIn ? (account?.usage?.used_percent ?? null) : null);
  /** Only the picture the profile chose; the account never stands in for it. */
  const face = $derived.by((): ReturnType<typeof drawn> | null => {
    if (options.private?.()) return null;
    const current = profiles.current();
    return current && current.avatar.revision > 0 ? drawn(current) : null;
  });
  const mark = $derived(
    signedIn ? identityAvatar(null, initials(account?.email, account?.name)) : null,
  );
  /** Every profile a window could browse as, once known; a private window switches to none. */
  const roster = $derived.by(() => {
    if (options.private?.()) return null;
    const here = profiles.current()?.id;
    return profiles.all().map((profile) => ({
      id: profile.id,
      name: profile.name,
      view: drawn(profile),
      current: profile.id === here,
    }));
  });

  return {
    get available() {
      return account?.available === true;
    },
    get signedIn() {
      return signedIn;
    },
    get email() {
      return account?.email ?? null;
    },
    get plan() {
      return signedIn ? planName(account?.plan) : null;
    },
    get used() {
      return used;
    },
    get tone() {
      return usageTone(used);
    },
    get mark() {
      return mark;
    },
    get face() {
      return face;
    },
    get profiles() {
      return roster;
    },
  };
}
