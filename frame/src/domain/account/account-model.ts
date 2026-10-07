import type { AccountOfferV1, AccountPriceV1 } from "$shared/ipc/bindings";

/** One or two letters for the avatar, from the name or else the address. */
export function initials(email: string | null | undefined, name?: string | null): string {
  const words = (name ?? "").trim().split(/\s+/u).filter(Boolean);
  if (words.length >= 2) return (words[0]![0]! + words.at(-1)![0]!).toUpperCase();
  if (words.length === 1) return words[0]![0]!.toUpperCase();
  const local = (email ?? "").split("@")[0] ?? "";
  const letter = [...local].find((c) => /\p{L}|\p{N}/u.test(c));
  return letter ? letter.toUpperCase() : "";
}

/** A plan id as the person reads it: `pro` → `Pro`. */
export function planName(plan: string | null | undefined): string | null {
  const id = plan?.trim();
  if (!id) return null;
  return id
    .split(/[-_\s]+/u)
    .filter(Boolean)
    .map((word) => word[0]!.toUpperCase() + word.slice(1))
    .join(" ");
}

/** The provider's price in the person's locale, or null if it cannot be shown truthfully. */
export function priceLabel(
  price: AccountPriceV1 | null | undefined,
  locale?: string,
): string | null {
  if (!price || !/^[A-Za-z]{3}$/u.test(price.currency)) return null;
  try {
    const format = new Intl.NumberFormat(locale, {
      style: "currency",
      currency: price.currency.toUpperCase(),
    });
    const digits = format.resolvedOptions().maximumFractionDigits ?? 2;
    const amount = price.amount_minor / 10 ** digits;
    return new Intl.NumberFormat(locale, {
      style: "currency",
      currency: price.currency.toUpperCase(),
      minimumFractionDigits: Number.isInteger(amount) ? 0 : digits,
      maximumFractionDigits: digits,
    }).format(amount);
  } catch {
    return null;
  }
}

/** A day, short and local, with the year only when it is not this one. */
export function shortDate(iso: string | null | undefined, now = new Date(), locale?: string) {
  if (!iso) return null;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return null;
  const sameYear = date.getFullYear() === now.getFullYear();
  return new Intl.DateTimeFormat(locale, {
    month: "short",
    day: "numeric",
    ...(sameYear ? {} : { year: "numeric" }),
  }).format(date);
}

/** How close the allowance is to running out, for the meter's colour. */
export function usageTone(percent: number | null | undefined): "calm" | "high" | "full" {
  if (percent == null) return "calm";
  if (percent >= 100) return "full";
  return percent >= 80 ? "high" : "calm";
}

export function offerKey(offer: AccountOfferV1): string {
  return offer.kind === "plan" ? `plan:${offer.plan}:${offer.interval}` : `topup:${offer.id}`;
}
