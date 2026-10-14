import type { SpaceThemeView } from "$shared/ipc/bindings";
import { reducedMotion } from "$shared/lib/motion";

/** The `ui.theme` and `ui.theme-custom` values a space's own theme stands in for. */
export function spaceThemeValues(
  own: SpaceThemeView | null,
  app: { theme: string; custom: string },
) {
  if (own === null) return app;
  return own.kind === "preset"
    ? { theme: own.id, custom: app.custom }
    : { theme: "custom", custom: own.seeds };
}

let shownSpace: string | null = null;
let shownTheme = "";

/**
 * Applies the window's theme. Moving to a space that wears another one
 * crossfades the whole window instead of snapping; a change made in
 * Appearance applies as it always has.
 */
export function applySpaceTheme(space: string | null, key: string, apply: () => void) {
  const crossfade =
    shownSpace !== null &&
    space !== shownSpace &&
    key !== shownTheme &&
    !reducedMotion() &&
    typeof document.startViewTransition === "function";
  shownSpace = space;
  shownTheme = key;
  if (crossfade) document.startViewTransition(apply);
  else apply();
}
