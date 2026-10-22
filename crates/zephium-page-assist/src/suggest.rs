//! The one-click starts offered for a page, chosen from its structure.
use crate::ipc::{PageAssistIntentV1 as Intent, PageAssistSuggestionV1 as Suggestion};
use crate::page::{PageKind, PageTraits};

/// Most suggestions shown at once.
pub const MAX_SUGGESTIONS: usize = 4;

/// Suggestions for a page, most useful first.
pub fn suggestions(traits: &PageTraits) -> Vec<Suggestion> {
    let mut chosen = Vec::with_capacity(MAX_SUGGESTIONS);
    let long = traits.words >= 150;
    if long {
        chosen.push(Suggestion::Summarize);
    }
    match traits.kind {
        PageKind::HowTo if traits.steps >= 3 => chosen.push(Suggestion::Steps),
        PageKind::Product if traits.prices >= 2 => chosen.push(Suggestion::Compare),
        PageKind::Docs => chosen.push(Suggestion::Explain),
        PageKind::Article if traits.words >= 600 => chosen.push(Suggestion::KeyPoints),
        _ => {}
    }
    if traits.prices > 0 || matches!(traits.kind, PageKind::Product) {
        chosen.push(Suggestion::Catch);
    }
    if traits.numbers >= 5 || traits.table_rows >= 3 {
        chosen.push(Suggestion::KeyNumbers);
    }
    if long && !chosen.contains(&Suggestion::KeyPoints) && chosen.len() < 3 {
        chosen.push(Suggestion::KeyPoints);
    }
    if traits.words >= 1500 {
        chosen.push(Suggestion::Readable);
    }
    chosen.push(Suggestion::Darker);
    chosen.dedup();
    chosen.truncate(MAX_SUGGESTIONS);
    chosen
}

/// The turn a suggestion starts.
pub fn intent_of(suggestion: Suggestion) -> Intent {
    match suggestion {
        Suggestion::Summarize => Intent::Summarize,
        Suggestion::Darker | Suggestion::Readable => Intent::Style,
        Suggestion::KeyPoints
        | Suggestion::KeyNumbers
        | Suggestion::Catch
        | Suggestion::Explain
        | Suggestion::Steps
        | Suggestion::Compare => Intent::Ask,
    }
}

/// What the suggestion asks, in the model's words. Fixed text; the person's
/// question is empty for a suggestion.
pub fn request_text(suggestion: Suggestion) -> &'static str {
    match suggestion {
        Suggestion::Summarize => "Summarize this page.",
        Suggestion::KeyPoints => "What are the key points of this page?",
        Suggestion::KeyNumbers => {
            "List the most important numbers on this page as figures, each with its label."
        }
        Suggestion::Catch => {
            "What's the catch? Point out fine print, conditions, fees, limits or anything easy to miss."
        }
        Suggestion::Explain => "Explain this page in plain language.",
        Suggestion::Steps => "List the steps this page describes, in order.",
        Suggestion::Compare => "Compare the options this page offers and say who each suits.",
        Suggestion::Darker => "Make this site dark: dark backgrounds, light readable text.",
        Suggestion::Readable => {
            "Make this site easier to read: larger text, comfortable line length and spacing."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_follow_the_page() {
        let cases = [
            (PageTraits::default(), vec![Suggestion::Darker]),
            (
                PageTraits {
                    kind: PageKind::Article,
                    words: 2000,
                    ..Default::default()
                },
                vec![
                    Suggestion::Summarize,
                    Suggestion::KeyPoints,
                    Suggestion::Readable,
                    Suggestion::Darker,
                ],
            ),
            (
                PageTraits {
                    kind: PageKind::Product,
                    words: 400,
                    prices: 3,
                    numbers: 9,
                    ..Default::default()
                },
                vec![
                    Suggestion::Summarize,
                    Suggestion::Compare,
                    Suggestion::Catch,
                    Suggestion::KeyNumbers,
                ],
            ),
            (
                PageTraits {
                    kind: PageKind::HowTo,
                    words: 300,
                    steps: 6,
                    ..Default::default()
                },
                vec![
                    Suggestion::Summarize,
                    Suggestion::Steps,
                    Suggestion::KeyPoints,
                    Suggestion::Darker,
                ],
            ),
        ];
        for (traits, expected) in cases {
            assert_eq!(suggestions(&traits), expected, "{traits:?}");
        }
    }

    #[test]
    fn every_suggestion_has_an_intent_and_fixed_text() {
        for suggestion in [
            Suggestion::Summarize,
            Suggestion::KeyPoints,
            Suggestion::KeyNumbers,
            Suggestion::Catch,
            Suggestion::Explain,
            Suggestion::Steps,
            Suggestion::Compare,
            Suggestion::Darker,
            Suggestion::Readable,
        ] {
            assert!(!request_text(suggestion).is_empty());
            let style = matches!(suggestion, Suggestion::Darker | Suggestion::Readable);
            assert_eq!(intent_of(suggestion) == Intent::Style, style);
        }
    }
}
