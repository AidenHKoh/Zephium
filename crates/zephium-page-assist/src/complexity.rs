//! Recognises, before any request, asks that need other pages or a long
//! chain of steps, so the panel offers "Run in Work" instead of trying.
//! Conservative: a missed case still ends in the model's `> work` or the
//! step cap; a false offer only costs the person a rephrase.
use crate::ipc::{PageAssistIntentV1 as Intent, PageAssistOfferV1 as Offer};

const MULTI_PAGE: &[&str] = &[
    "other sites",
    "other websites",
    "another site",
    "other stores",
    "across sites",
    "across the web",
    "search the web",
    "search online",
    "search google",
    "google it",
    "look it up",
    "every page",
    "all pages",
    "each page",
    "all the pages",
    "other tabs",
    "my other tab",
    "go through all",
    "check all the links",
    "open each",
    "open every",
    "open all",
    "visit each",
    "compare with other",
    "compare prices elsewhere",
    "cheaper elsewhere",
    "find me a",
    "find a cheaper",
];

const CHAINED: &[&str] = &[
    "and then",
    "after that",
    "then also",
    "for each",
    "for every",
    "one by one",
    "all of them",
    "every item",
    "each item",
    "each result",
    "every result",
    "sign up",
    "sign me up",
    "create an account",
    "check out",
    "checkout",
    "book a",
    "book me",
    "apply for",
    "fill out the whole",
    "fill in everything",
];

/// The offer to make instead of answering, if any.
pub fn upfront_offer(intent: Intent, question: &str) -> Option<Offer> {
    if matches!(
        intent,
        Intent::Summarize | Intent::Style | Intent::Selection
    ) {
        return None;
    }
    let lower = question.to_lowercase();
    if mentions_other_address(&lower) || MULTI_PAGE.iter().any(|phrase| lower.contains(phrase)) {
        return Some(Offer::MultiPage);
    }
    if intent == Intent::Act && CHAINED.iter().any(|phrase| lower.contains(phrase)) {
        return Some(Offer::Complex);
    }
    None
}

fn mentions_other_address(lower: &str) -> bool {
    lower.contains("http://") || lower.contains("https://") || lower.contains("www.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_follow_the_question() {
        let cases = [
            (Intent::Ask, "What does this page say about refunds?", None),
            (
                Intent::Ask,
                "Is this cheaper elsewhere?",
                Some(Offer::MultiPage),
            ),
            (
                Intent::Ask,
                "Compare with other stores",
                Some(Offer::MultiPage),
            ),
            (
                Intent::Ask,
                "what's on https://other.example",
                Some(Offer::MultiPage),
            ),
            (Intent::Act, "Sort by price", None),
            (
                Intent::Act,
                "Fill the form and then submit it and then download",
                Some(Offer::Complex),
            ),
            (
                Intent::Act,
                "Book a table for two on Friday",
                Some(Offer::Complex),
            ),
            (
                Intent::Act,
                "Open each result and copy the prices",
                Some(Offer::MultiPage),
            ),
            (Intent::Ask, "Sort by price and then show me", None),
            (Intent::Summarize, "every page", None),
            (Intent::Style, "make all pages dark", None),
        ];
        for (intent, question, expected) in cases {
            assert_eq!(upfront_offer(intent, question), expected, "{question}");
        }
    }
}
