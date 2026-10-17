import * as m from "$shared/i18n/messages";

export const WEBSITE = "https://zephium.app";

export const links = [
  { id: "website", url: WEBSITE, label: () => m.about_link_website() },
  { id: "docs", url: `${WEBSITE}/docs`, label: () => m.about_link_docs() },
  { id: "github", url: "https://github.com/zephium-browser/Zephium", label: () => "GitHub" },
  { id: "discord", url: "https://discord.gg/tyveTUyEp7", label: () => "Discord" },
  { id: "youtube", url: "https://www.youtube.com/@crynta", label: () => "YouTube" },
] as const;
