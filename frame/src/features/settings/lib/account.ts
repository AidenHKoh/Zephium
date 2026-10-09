import type { AccountFaultV1 } from "$shared/ipc/bindings";
import * as m from "$shared/i18n/messages";

export const TERMS_URL = "https://zephium.app/terms";
export const PRIVACY_URL = "https://zephium.app/privacy";

/** What a fault means to the person, in one sentence. */
export function explainAccountFault(fault: AccountFaultV1, retryAfter: number | null): string {
  switch (fault) {
    case "offline":
      return m.account_fault_offline();
    case "signed_out":
      return m.account_fault_signed_out();
    case "rate_limited":
      return m.account_fault_rate_limited({ seconds: Math.max(1, retryAfter ?? 60) });
    case "invalid_code":
      return m.account_fault_invalid_code();
    case "signup_closed":
      return m.account_fault_signup_closed();
    case "unavailable":
      return m.account_fault_unavailable();
    case "no_billing_account":
      return m.account_fault_no_billing();
    case "server":
      return m.account_fault_server();
    case "vault":
      return m.account_fault_vault();
    case "invalid":
      return m.account_fault_invalid();
    case "reauth_required":
      return m.account_fault_reauth();
    case "email_taken":
      return m.account_fault_email_taken();
    case "not_found":
      return m.account_fault_not_found();
  }
}

export function plausibleEmail(text: string): boolean {
  const email = text.trim();
  const at = email.lastIndexOf("@");
  if (at < 1 || email.length > 254 || /\s/u.test(email)) return false;
  const domain = email.slice(at + 1);
  return domain.includes(".") && !domain.startsWith(".") && !domain.endsWith(".");
}

/** The six digits a person typed or pasted, ignoring spaces and dashes. */
export function codeDigits(text: string): string {
  return text.replace(/[\s-]/gu, "").replace(/\D/gu, "").slice(0, 6);
}
