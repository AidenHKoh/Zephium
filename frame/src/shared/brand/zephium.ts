import type { IconSvgElement } from "@hugeicons/svelte";

const round = { strokeLinecap: "round", strokeLinejoin: "round" } as const;

/** Zephium's own mark as a line glyph: two offset bars that step past each
 *  other around a diamond, drawn like the icons beside it. */
export const ZephiumMark: IconSvgElement = [
  ["path", { d: "M3.25 10.9H6.2C8.6 10.9 9.2 6.8 11.6 6.8H20.75", ...round, key: "0" }],
  ["path", { d: "M3.25 17.1H12.2C14.6 17.1 14.9 13.6 16.8 13.6H20.75", ...round, key: "1" }],
  [
    "path",
    {
      d: "M11.64 10.6L13.1 12.06L11.64 13.52L10.18 12.06Z",
      fill: "currentColor",
      ...round,
      key: "2",
    },
  ],
];
