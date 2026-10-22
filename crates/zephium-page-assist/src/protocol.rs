//! The strict line protocol the model answers in, parsed as the stream
//! arrives. Anything that is not a protocol line is dropped, so prose,
//! markdown and echoed page text never reach the panel.
//!
//! ```text
//! = <short answer> {r12}
//! - <supporting point> {r12,r40}
//! # <label> :: <value> {r3}
//! ! <caveat or not on the page>
//! > <action proposal>
//! @css … @end   (style turns: the proposed stylesheet)
//! ```
use crate::ipc::PageAssistPartKindV1 as PartKind;
use crate::text;

/// Longest protocol line, in characters; longer lines are dropped whole.
pub const MAX_LINE_CHARS: usize = 600;
/// Most parts one answer may hold.
pub const MAX_PARTS: usize = 16;
/// Longest short answer, in characters.
pub const MAX_ANSWER_CHARS: usize = 300;
/// Longest figure label, in characters.
pub const MAX_LABEL_CHARS: usize = 60;
/// Most action proposals one answer may hold.
pub const MAX_ACTIONS: usize = 16;
/// Largest stylesheet a style turn may propose, in bytes.
pub const MAX_CSS_BYTES: usize = 16 * 1024;

/// One parsed answer block, before its citations are numbered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedPart {
    /// Which block this is.
    pub kind: PartKind,
    /// Display text, cleaned and bounded.
    pub text: String,
    /// A figure's label.
    pub label: Option<String>,
    /// Passage numbers the model cited, in order, unchecked.
    pub fragments: Vec<u16>,
}

/// What a line produced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Output {
    /// A whole answer block.
    Part(ParsedPart),
    /// The text of one `>` action proposal.
    Action(String),
}

/// Incremental parser for one model answer.
#[derive(Debug, Default)]
pub struct Parser {
    line: String,
    line_chars: usize,
    discarding: bool,
    answered: bool,
    parts: usize,
    held: Vec<ParsedPart>,
    actions: usize,
    in_css: bool,
    css: String,
    css_overflow: bool,
}

impl Parser {
    /// Feeds streamed text; returns blocks completed by it.
    pub fn push(&mut self, delta: &str) -> Vec<Output> {
        let mut output = Vec::new();
        for character in delta.chars() {
            if character == '\n' || character == '\r' {
                self.end_line(&mut output);
                continue;
            }
            if self.discarding {
                continue;
            }
            self.line_chars += 1;
            let limit = if self.in_css {
                MAX_CSS_BYTES
            } else {
                MAX_LINE_CHARS
            };
            if self.line_chars > limit {
                self.discarding = true;
                self.line.clear();
                continue;
            }
            self.line.push(character);
        }
        output
    }

    /// Ends the answer: parses the last line and, when no short answer came,
    /// promotes the first point to it.
    pub fn finish(&mut self) -> Vec<Output> {
        let mut output = Vec::new();
        self.end_line(&mut output);
        if self.in_css {
            self.css_overflow = true;
        }
        if !self.answered {
            self.answered = true;
            if let Some(first) = self.held.iter().position(|p| p.kind == PartKind::Point) {
                let mut answer = self.held.remove(first);
                answer.kind = PartKind::Answer;
                answer.text = text::bounded(&answer.text, MAX_ANSWER_CHARS);
                output.push(Output::Part(answer));
            }
            output.extend(self.held.drain(..).map(Output::Part));
        }
        output
    }

    /// The proposed stylesheet, when a complete `@css … @end` block arrived.
    pub fn css(&self) -> Option<&str> {
        (!self.css_overflow && !self.css.is_empty()).then_some(self.css.as_str())
    }

    fn end_line(&mut self, output: &mut Vec<Output>) {
        let discarded = std::mem::take(&mut self.discarding);
        let line = std::mem::take(&mut self.line);
        self.line_chars = 0;
        if self.in_css {
            if discarded {
                self.css_overflow = true;
            } else if line.trim() == "@end" {
                self.in_css = false;
            } else if self.css.len() + line.len() + 1 > MAX_CSS_BYTES {
                self.css_overflow = true;
            } else if !self.css_overflow {
                self.css.push_str(&line);
                self.css.push('\n');
            }
            return;
        }
        if discarded {
            return;
        }
        let line = line.trim();
        if line == "@css" {
            if !self.css.is_empty() {
                // A second stylesheet is never merged into the first.
                self.css_overflow = true;
            }
            self.in_css = true;
            return;
        }
        let Some((marker, body)) = line.split_at_checked(1) else {
            return;
        };
        if !body.starts_with(' ') {
            return;
        }
        match marker {
            ">" => {
                let action = text::clean(body);
                if !action.is_empty() && self.actions < MAX_ACTIONS {
                    self.actions += 1;
                    output.push(Output::Action(action));
                }
            }
            "=" | "-" | "#" | "!" => {
                let Some(part) = parse_part(marker, body) else {
                    return;
                };
                self.accept(part, output);
            }
            _ => {}
        }
    }

    fn accept(&mut self, mut part: ParsedPart, output: &mut Vec<Output>) {
        if self.parts >= MAX_PARTS {
            return;
        }
        if part.kind == PartKind::Answer && self.answered {
            part.kind = PartKind::Point;
        }
        self.parts += 1;
        if part.kind == PartKind::Answer {
            self.answered = true;
            output.push(Output::Part(part));
            output.extend(self.held.drain(..).map(Output::Part));
        } else if self.answered {
            output.push(Output::Part(part));
        } else {
            self.held.push(part);
        }
    }
}

fn parse_part(marker: &str, body: &str) -> Option<ParsedPart> {
    let (body, fragments) = take_citations(body);
    let kind = match marker {
        "=" => PartKind::Answer,
        "-" => PartKind::Point,
        "#" => PartKind::Figure,
        _ => PartKind::Caveat,
    };
    let (label, value) = if kind == PartKind::Figure {
        let (label, value) = body.split_once("::")?;
        let label = text::clean(label);
        if label.is_empty() {
            return None;
        }
        (Some(text::bounded(&label, MAX_LABEL_CHARS)), value)
    } else {
        (None, body.as_str())
    };
    let shown = text::clean(value);
    if shown.is_empty() {
        return None;
    }
    let text = if kind == PartKind::Answer {
        text::bounded(&shown, MAX_ANSWER_CHARS)
    } else {
        shown
    };
    Some(ParsedPart {
        kind,
        text,
        label,
        fragments: if kind == PartKind::Caveat {
            Vec::new()
        } else {
            fragments
        },
    })
}

/// Removes every `{r1,r2}` group, returning the text and the ids in order.
/// Braces that are not a citation group stay as text.
fn take_citations(body: &str) -> (String, Vec<u16>) {
    let mut text = String::with_capacity(body.len());
    let mut ids = Vec::new();
    let mut rest = body;
    while let Some(open) = rest.find('{') {
        text.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => match citation_group(&after[..close]) {
                Some(group) => {
                    ids.extend(group);
                    rest = &after[close + 1..];
                }
                None => {
                    text.push('{');
                    rest = after;
                }
            },
            None => {
                text.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    text.push_str(rest);
    (text, ids)
}

fn citation_group(inner: &str) -> Option<Vec<u16>> {
    let mut ids = Vec::new();
    for token in inner.split(',') {
        let token = token.trim();
        let digits = token
            .strip_prefix("@r")
            .or_else(|| token.strip_prefix('r'))?;
        if digits.is_empty()
            || digits.len() > 5
            || (digits.len() > 1 && digits.starts_with('0'))
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let id = digits.parse::<u16>().ok().filter(|id| *id > 0)?;
        if ids.len() < 16 {
            ids.push(id);
        }
    }
    (!ids.is_empty()).then_some(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn parse(answer: &str) -> (Vec<Output>, Parser) {
        let mut parser = Parser::default();
        let mut output = parser.push(answer);
        output.extend(parser.finish());
        (output, parser)
    }

    fn part(kind: PartKind, text: &str, fragments: &[u16]) -> Output {
        Output::Part(ParsedPart {
            kind,
            text: text.into(),
            label: None,
            fragments: fragments.to_vec(),
        })
    }

    #[test]
    fn whole_answer_parses_in_protocol_order() {
        let (output, _) = parse(
            "Sure! Here is the answer:\n= It ships in May. {r12}\n- Returns are free {r3,r40}\n# Price :: $12/month {r7}\n! Not on this page: shipping to Canada.\n> click r5\n",
        );
        assert_eq!(
            output,
            vec![
                part(PartKind::Answer, "It ships in May.", &[12]),
                part(PartKind::Point, "Returns are free", &[3, 40]),
                Output::Part(ParsedPart {
                    kind: PartKind::Figure,
                    text: "$12/month".into(),
                    label: Some("Price".into()),
                    fragments: vec![7],
                }),
                part(
                    PartKind::Caveat,
                    "Not on this page: shipping to Canada.",
                    &[]
                ),
                Output::Action("click r5".into()),
            ]
        );
    }

    #[test]
    fn points_before_the_answer_wait_for_it() {
        let mut parser = Parser::default();
        assert!(parser.push("- first {r1}\n- second\n").is_empty());
        let output = parser.push("= the answer\n");
        assert_eq!(
            output,
            vec![
                part(PartKind::Answer, "the answer", &[]),
                part(PartKind::Point, "first", &[1]),
                part(PartKind::Point, "second", &[]),
            ]
        );
    }

    #[test]
    fn without_an_answer_the_first_point_is_promoted() {
        let (output, _) = parse("! caveat\n- first {r1}\n- second");
        assert_eq!(
            output,
            vec![
                part(PartKind::Answer, "first", &[1]),
                part(PartKind::Caveat, "caveat", &[]),
                part(PartKind::Point, "second", &[]),
            ]
        );
        assert!(parse("just prose, no protocol").0.is_empty());
    }

    #[test]
    fn malformed_and_hostile_lines_are_dropped() {
        for line in [
            "=no space",
            "-no space",
            "# label without separator",
            "#  :: value without label",
            "=   ",
            "* markdown bullet",
            "1. numbered",
            "<script>alert(1)</script>",
        ] {
            let (output, _) = parse(line);
            assert!(output.is_empty(), "{line:?} produced {output:?}");
        }
        let long = format!("= {}", "a".repeat(MAX_LINE_CHARS));
        assert!(parse(&long).0.is_empty());
        let (output, _) = parse(&format!("= {}\n= ok", "a".repeat(MAX_LINE_CHARS)));
        assert_eq!(output, vec![part(PartKind::Answer, "ok", &[])]);
    }

    #[test]
    fn citations_parse_strictly() {
        for (line, text, ids) in [
            ("- a {r1}", "a", vec![1]),
            ("- a {@r2, r3}", "a", vec![2, 3]),
            ("- a {r01}", "a {r01}", vec![]),
            ("- a {r0}", "a {r0}", vec![]),
            ("- a {r70000}", "a {r70000}", vec![]),
            ("- set {x} here {r4}", "set {x} here", vec![4]),
            ("- {r1} lead and tail {r2}", "lead and tail", vec![1, 2]),
            ("- open { brace", "open { brace", vec![]),
        ] {
            let (output, _) = parse(line);
            let Output::Part(parsed) = &output[0] else {
                panic!("{line}");
            };
            assert_eq!(parsed.text, text, "{line}");
            assert_eq!(parsed.fragments, ids, "{line}");
        }
        let (output, _) = parse("! not on page {r3}");
        assert_eq!(output, vec![part(PartKind::Caveat, "not on page", &[])]);
    }

    #[test]
    fn limits_hold() {
        let many: String = (0..40).map(|i| format!("- point {i}\n")).collect();
        let (output, _) = parse(&format!("= a\n{many}"));
        assert_eq!(output.len(), MAX_PARTS);
        let (output, _) = parse(&format!("= {}", "word ".repeat(100)));
        let Output::Part(answer) = &output[0] else {
            panic!()
        };
        assert!(answer.text.chars().count() <= MAX_ANSWER_CHARS);
        let duplicate = parse("= one\n= two").0;
        assert_eq!(duplicate[1], part(PartKind::Point, "two", &[]));
    }

    #[test]
    fn stylesheet_block_is_collected_once_and_bounded() {
        let (output, parser) = parse("= Darker {r1}\n@css\nbody { color: white }\n@end\n- note");
        assert_eq!(output.len(), 2);
        assert_eq!(parser.css(), Some("body { color: white }\n"));
        let (_, unterminated) = parse("@css\nbody{}\n");
        assert_eq!(unterminated.css(), None);
        let (_, twice) = parse("@css\na{}\n@end\n@css\nb{}\n@end");
        assert_eq!(twice.css(), None);
        let big = format!("@css\n{}\n@end", "a{color:red}\n".repeat(2000));
        assert_eq!(parse(&big).1.css(), None);
    }

    proptest! {
        #[test]
        fn any_split_of_the_stream_parses_the_same(
            answer in "(= [a-z ]{1,20}( \\{r[1-9]\\})?\n|- [a-z ]{1,20}( \\{r[1-9],r[1-9]\\})?\n|! [a-z]{1,10}\n|> click r[1-9]\n|[a-z ]{0,10}\n){0,12}",
            cut in 0usize..400,
        ) {
            let whole = parse(&answer).0;
            let cut = cut.min(answer.len());
            let mut parser = Parser::default();
            let mut split = parser.push(&answer[..cut]);
            split.extend(parser.push(&answer[cut..]));
            split.extend(parser.finish());
            prop_assert_eq!(whole, split);
        }

        #[test]
        fn arbitrary_input_never_breaks_the_bounds(input in "\\PC{0,3000}") {
            let (output, parser) = parse(&input);
            let parts = output.iter().filter(|o| matches!(o, Output::Part(_))).count();
            prop_assert!(parts <= MAX_PARTS);
            prop_assert!(output.iter().filter(|o| matches!(o, Output::Action(_))).count() <= MAX_ACTIONS);
            for item in &output {
                if let Output::Part(part) = item {
                    prop_assert!(!part.text.is_empty());
                    prop_assert!(part.text.chars().count() <= MAX_LINE_CHARS);
                    prop_assert!(!part.text.chars().any(char::is_control));
                    prop_assert!(part.fragments.len() <= MAX_LINE_CHARS);
                }
            }
            prop_assert!(parser.css().is_none_or(|css| css.len() <= MAX_CSS_BYTES));
        }
    }
}
