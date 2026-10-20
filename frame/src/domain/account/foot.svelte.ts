import * as accountSession from "./account.svelte";
import { initials, planName, usageTone } from "./account-model";
import * as m from "$shared/i18n/messages";
import { drawn, identityAvatar, profiles } from "$domain/profiles";

/**
 * What the account shows at the foot of Browse and Work: one reading of the
 * session for the trigger's face and the menu's account row, so the two feet
 * cannot drift. Call it during component initialisation.
 */
export function accountFoot(options: { private?: () => boolean } = {}) {
  const account = $derived(accountSession.account());
  const signedIn = $derived(account?.signed_in === true);
  const used = $derived(signedIn ? (account?.usage?.used_percent ?? null) : null);
  const tone = $derived(usageTone(used));
  const standing = $derived.by(() => {
    if (!signedIn || !account) return null;
    const parts = [
      planName(account.plan),
      used != null ? m.ai_cloud_used({ percent: used }) : null,
    ].filter(Boolean);
    return parts.join(" · ") || account.email || null;
  });
  /** The picture the profile chose; until then the account's initials once signed in. */
  const face = $derived.by((): ReturnType<typeof drawn> | null => {
    if (options.private?.()) return null;
    const current = profiles.current();
    if (current && current.avatar.revision > 0) return drawn(current);
    return signedIn ? identityAvatar(null, initials(account?.email, account?.name)) : null;
  });

  return {
    get available() {
      return account?.available === true;
    },
    get signedIn() {
      return signedIn;
    },
    get tone() {
      return tone;
    },
    get standing() {
      return standing;
    },
    get face() {
      return face;
    },
  };
}
