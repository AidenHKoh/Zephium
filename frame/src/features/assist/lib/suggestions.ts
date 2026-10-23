import * as m from "$shared/i18n/messages";
import type { PageAssistSuggestionV1 } from "$shared/ipc/bindings";

const LABELS: Record<PageAssistSuggestionV1, () => string> = {
  summarize: m.assist_suggest_summarize,
  key_points: m.assist_suggest_key_points,
  key_numbers: m.assist_suggest_key_numbers,
  catch: m.assist_suggest_catch,
  explain: m.assist_suggest_explain,
  steps: m.assist_suggest_steps,
  compare: m.assist_suggest_compare,
  picks: m.assist_suggest_picks,
  redesign: m.assist_suggest_redesign,
  darker: m.assist_suggest_darker,
  readable: m.assist_suggest_readable,
};

export const suggestionLabel = (suggestion: PageAssistSuggestionV1) => LABELS[suggestion]();

export const isRestyle = (suggestion: PageAssistSuggestionV1) =>
  suggestion === "redesign" || suggestion === "darker" || suggestion === "readable";
