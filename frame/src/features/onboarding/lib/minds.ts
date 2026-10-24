import type { WorkLocalServerV1 } from "$shared/ipc/bindings";

/** Where Work's model can come from, as onboarding offers it. */
export type Mind = "zephium" | "key" | "local";

/** What the browser opens once it is up, for a setup onboarding cannot finish. */
export type Then = "account" | "models";

/** A running local server is connected here, so it needs nothing after;
 *  every other choice is finished in its own settings page. */
export function followUp(mind: Mind | null, server: WorkLocalServerV1 | null): Then | null {
  if (mind === "zephium") return "account";
  if (mind === "key") return "models";
  if (mind === "local") return server ? null : "models";
  return null;
}
