import * as m from "$shared/i18n/messages";

const VENDORS: Record<string, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
  google: "Google",
  zephium: "Zephium",
  openrouter: "OpenRouter",
};

/**
 * Who supplied a cited page, as the evidence names it: OpenAI's own search,
 * or the chosen model's provider searching natively (its catalog id says
 * which, `anthropic/claude-…`).
 */
export function searchProvider(
  provider: string,
  model: string,
): { provider: string; model: string } {
  const [vendor, ...rest] = model.split("/");
  const name = rest.length ? rest.join("/") : model;
  if (provider === "open_ai")
    return { provider: m.work_provider_web_search({ provider: "OpenAI" }), model: name };
  const known = vendor && rest.length ? VENDORS[vendor] : undefined;
  return { provider: known ?? m.work_provider_model(), model: name };
}
