//! Numbered sources for one turn. A chip's excerpt and the text the page
//! highlights both come from the read's verbatim passage, never the model.
use crate::ipc::{PageAssistCiteV1 as Cite, PAGE_ASSIST_MAX_EXCERPT_CHARS};
use crate::page::PageRead;
use crate::text;

/// Most distinct sources one turn may number.
pub const MAX_SOURCES: usize = 24;
/// Most sources one part may cite.
pub const MAX_CITES_PER_PART: usize = 4;
/// Longest text handed to the page highlight, in bytes.
pub const MAX_HIGHLIGHT_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Source {
    fragment: u16,
    excerpt: String,
    highlight: String,
}

/// The turn's sources in the order they were first cited.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CitationBook {
    sources: Vec<Source>,
}

impl CitationBook {
    /// Numbers the passages a part cites. Unknown ids, passages without
    /// verbatim text, repeats and anything past the caps are dropped.
    pub fn cite(&mut self, read: &PageRead, fragments: &[u16]) -> Vec<Cite> {
        let mut cites: Vec<Cite> = Vec::new();
        for &fragment in fragments {
            if cites.len() == MAX_CITES_PER_PART {
                break;
            }
            let Some(n) = self.number(read, fragment) else {
                continue;
            };
            if cites.iter().any(|cite| cite.n == n) {
                continue;
            }
            let excerpt = self.sources[usize::from(n) - 1].excerpt.clone();
            cites.push(Cite { n, excerpt });
        }
        cites
    }

    fn number(&mut self, read: &PageRead, fragment: u16) -> Option<u8> {
        if let Some(index) = self.sources.iter().position(|s| s.fragment == fragment) {
            return u8::try_from(index + 1).ok();
        }
        if self.sources.len() >= MAX_SOURCES {
            return None;
        }
        let verbatim = read.fragments.get(&fragment)?.verbatim.as_deref()?;
        let shown = text::clean(verbatim);
        if shown.is_empty() {
            return None;
        }
        let highlight = text::verbatim_prefix(verbatim.trim(), MAX_HIGHLIGHT_BYTES);
        if highlight.is_empty() || highlight.chars().any(char::is_control) {
            return None;
        }
        self.sources.push(Source {
            fragment,
            excerpt: text::bounded(&shown, PAGE_ASSIST_MAX_EXCERPT_CHARS),
            highlight: highlight.to_owned(),
        });
        u8::try_from(self.sources.len()).ok()
    }

    /// The page's own words for chip `n`, to scroll to and mark.
    pub fn highlight(&self, n: u8) -> Option<&str> {
        let index = usize::from(n).checked_sub(1)?;
        self.sources
            .get(index)
            .map(|source| source.highlight.as_str())
    }

    /// Sources numbered so far.
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Whether nothing was cited.
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::{Fragment, PageTraits};
    use std::collections::BTreeMap;

    pub(crate) fn read(passages: &[(u16, Option<&str>)]) -> PageRead {
        PageRead {
            site: "example.com".into(),
            title: None,
            payload: String::new(),
            fragments: passages
                .iter()
                .map(|(id, text)| {
                    (
                        *id,
                        Fragment {
                            verbatim: text.map(str::to_owned),
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>(),
            traits: PageTraits::default(),
            fingerprint: 1,
        }
    }

    #[test]
    fn numbers_follow_first_citation_and_drop_what_is_not_verbatim() {
        let page = read(&[
            (3, Some("Shipping starts in May.")),
            (7, Some("Returns are free for 30 days.")),
            (9, None),
            (11, Some("  \u{202E}  ")),
        ]);
        let mut book = CitationBook::default();
        let first = book.cite(&page, &[7, 3, 7, 99, 9, 11]);
        assert_eq!(
            first.iter().map(|c| c.n).collect::<Vec<_>>(),
            vec![1, 2],
            "unknown, unverbatim and blank passages are dropped"
        );
        assert_eq!(first[0].excerpt, "Returns are free for 30 days.");
        let second = book.cite(&page, &[3]);
        assert_eq!(second[0].n, 2);
        assert_eq!(book.highlight(1), Some("Returns are free for 30 days."));
        assert_eq!(book.highlight(0), None);
        assert_eq!(book.highlight(3), None);
    }

    #[test]
    fn excerpts_and_highlights_are_bounded_and_highlight_stays_verbatim() {
        let long = "word ".repeat(200);
        let page = read(&[(1, Some(long.as_str()))]);
        let mut book = CitationBook::default();
        let cite = book.cite(&page, &[1]);
        assert!(cite[0].excerpt.chars().count() <= PAGE_ASSIST_MAX_EXCERPT_CHARS);
        let highlight = book.highlight(1).unwrap();
        assert!(highlight.len() <= MAX_HIGHLIGHT_BYTES);
        assert!(long.starts_with(highlight));
    }

    #[test]
    fn caps_hold() {
        let passages: Vec<(u16, Option<String>)> = (1..=40)
            .map(|i| (i, Some(format!("passage {i}"))))
            .collect();
        let borrowed: Vec<(u16, Option<&str>)> =
            passages.iter().map(|(i, t)| (*i, t.as_deref())).collect();
        let page = read(&borrowed);
        let mut book = CitationBook::default();
        let ids: Vec<u16> = (1..=40).collect();
        assert_eq!(book.cite(&page, &ids).len(), MAX_CITES_PER_PART);
        for chunk in ids.chunks(4) {
            book.cite(&page, chunk);
        }
        assert_eq!(book.len(), MAX_SOURCES);
    }
}
