/**
 * Host traits the chrome must branch on. Kept in one place so a component
 * never re-derives the platform from the user agent.
 *
 * macOS keeps AppKit's traffic lights through an overlay title bar; Windows
 * draws its controls in a title band above the page. Linux uses sidebar
 * controls.
 */
const userAgent = navigator.userAgent;
export const IS_MAC = userAgent.includes("Mac");
export const IS_WINDOWS = userAgent.includes("Windows");
