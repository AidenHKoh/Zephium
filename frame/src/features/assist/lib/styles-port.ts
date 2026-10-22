import type { PageStyleV1 } from "$shared/ipc/bindings";

/** Every site the profile has restyled, as Settings lists them. */
export type StyledSites = { site: string; styles: PageStyleV1[] }[];

/**
 * The commands `page_styles_all(profile)`, `page_style_toggle_for(profile, site,
 * style, enabled)` and `page_style_remove_for(profile, site, style)` are not in the
 * contract yet; until they are, Settings reports the list as unreachable.
 */
const missing = (): never => {
  throw new Error("page styles are listed per tab only");
};

export const stylesPort = {
  list: async (_profile: string): Promise<StyledSites> => missing(),
  toggle: async (
    _profile: string,
    _site: string,
    _style: string,
    _enabled: boolean,
  ): Promise<StyledSites> => missing(),
  remove: async (_profile: string, _site: string, _style: string): Promise<StyledSites> =>
    missing(),
};
