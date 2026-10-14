/** "New Space…" or "Edit Space…" asked for from a menu, waiting for the
 *  switcher, with the release of a column expanded only to show it. */
type Request = { kind: "new" | "edit"; release: (() => void) | null };

let request = $state.raw<Request | null>(null);
let handled = 0;

export const pending = () => request;
export function ask(kind: Request["kind"], release: (() => void) | null = null) {
  request?.release?.();
  request = { kind, release };
}
export function take(): Request | null {
  const taken = request;
  request = null;
  return taken;
}
/** Each native command is handled once, by whichever shape of the column has it. */
export function claim(seq: number): boolean {
  if (seq <= handled) return false;
  handled = seq;
  return true;
}
