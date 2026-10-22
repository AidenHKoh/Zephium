//! Light page actions the model may propose, one per `>` line. The grammar
//! is closed: a verb, a passage of the current read and, for typing and
//! choosing, a quoted value. What each step may do is decided later by Rust
//! from the live control, never from these words.
use crate::ipc::PageAssistOfferV1 as Offer;

/// Most steps one turn may run.
pub const MAX_STEPS: usize = 8;
/// Longest value a step may type or choose, in characters.
pub const MAX_VALUE_CHARS: usize = 256;

/// What a step does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verb {
    /// Press a button or link.
    Click,
    /// Type into a field.
    Fill,
    /// Choose an option.
    Select,
    /// Tick a checkbox.
    Check,
    /// Untick a checkbox.
    Uncheck,
    /// Bring a passage into view.
    Scroll,
    /// Open a collapsed section.
    Expand,
    /// Follow a link on the same site.
    Open,
}

/// Where a scroll goes when it names no passage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// One screen up.
    Up,
    /// One screen down.
    Down,
}

/// One proposed step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    /// What it does.
    pub verb: Verb,
    /// The passage it acts on, as `rN`.
    pub target: Option<u16>,
    /// A page scroll with no passage.
    pub direction: Option<Direction>,
    /// What to type or choose.
    pub value: Option<String>,
}

/// One `>` line, understood.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Proposal {
    /// A step on this page.
    Step(Step),
    /// The model judged the request too big for the panel.
    Work,
}

/// Parses one action line; anything outside the grammar is refused.
pub fn parse(line: &str) -> Option<Proposal> {
    let line = line.trim();
    let (verb, rest) = line.split_once(' ').unwrap_or((line, ""));
    let verb = verb.to_ascii_lowercase();
    if verb == "work" {
        return Some(Proposal::Work);
    }
    let verb = match verb.as_str() {
        "click" | "press" => Verb::Click,
        "fill" | "type" => Verb::Fill,
        "select" | "choose" => Verb::Select,
        "check" => Verb::Check,
        "uncheck" => Verb::Uncheck,
        "scroll" => Verb::Scroll,
        "expand" => Verb::Expand,
        "open" => Verb::Open,
        _ => return None,
    };
    let rest = rest.trim();
    if verb == Verb::Scroll {
        let direction = match rest.to_ascii_lowercase().as_str() {
            "up" => Some(Direction::Up),
            "down" => Some(Direction::Down),
            _ => None,
        };
        if direction.is_some() {
            return Some(Proposal::Step(Step {
                verb,
                target: None,
                direction,
                value: None,
            }));
        }
    }
    let (target, rest) = rest.split_once(' ').unwrap_or((rest, ""));
    let target = fragment(target)?;
    let rest = rest.trim();
    let value = match verb {
        Verb::Fill | Verb::Select => Some(quoted(rest)?),
        _ if rest.is_empty() => None,
        _ => return None,
    };
    Some(Proposal::Step(Step {
        verb,
        target: Some(target),
        direction: None,
        value,
    }))
}

fn fragment(token: &str) -> Option<u16> {
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
    digits.parse::<u16>().ok().filter(|id| *id > 0)
}

fn quoted(text: &str) -> Option<String> {
    let inner = text.strip_prefix('"')?.strip_suffix('"')?;
    if inner.contains('"')
        || inner.chars().any(char::is_control)
        || inner.chars().count() > MAX_VALUE_CHARS
    {
        return None;
    }
    Some(inner.to_owned())
}

/// The steps of a turn, or the reason to offer Work instead.
pub fn plan(proposals: &[Proposal]) -> Result<Vec<Step>, Offer> {
    if proposals.iter().any(|p| matches!(p, Proposal::Work)) {
        return Err(Offer::Complex);
    }
    let steps: Vec<Step> = proposals
        .iter()
        .filter_map(|proposal| match proposal {
            Proposal::Step(step) => Some(step.clone()),
            Proposal::Work => None,
        })
        .collect();
    if steps.len() > MAX_STEPS {
        return Err(Offer::Complex);
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(verb: Verb, target: u16, value: Option<&str>) -> Option<Proposal> {
        Some(Proposal::Step(Step {
            verb,
            target: Some(target),
            direction: None,
            value: value.map(str::to_owned),
        }))
    }

    #[test]
    fn grammar_is_closed() {
        let cases = [
            ("click r5", step(Verb::Click, 5, None)),
            ("Press @r12", step(Verb::Click, 12, None)),
            (
                "fill r3 \"Ada Lovelace\"",
                step(Verb::Fill, 3, Some("Ada Lovelace")),
            ),
            (
                "select r9 \"Price: low to high\"",
                step(Verb::Select, 9, Some("Price: low to high")),
            ),
            ("expand r2", step(Verb::Expand, 2, None)),
            ("open r40", step(Verb::Open, 40, None)),
            ("check r1", step(Verb::Check, 1, None)),
            (
                "scroll down",
                Some(Proposal::Step(Step {
                    verb: Verb::Scroll,
                    target: None,
                    direction: Some(Direction::Down),
                    value: None,
                })),
            ),
            ("work", Some(Proposal::Work)),
            ("click", None),
            ("click r0", None),
            ("click #submit", None),
            ("click r5 now", None),
            ("fill r3 unquoted", None),
            ("fill r3 \"has \" quote\"", None),
            ("fill r3", None),
            ("navigate https://evil.example", None),
            ("eval r1", None),
            ("submit r1", None),
        ];
        for (line, expected) in cases {
            assert_eq!(parse(line), expected, "{line}");
        }
        let long = format!("fill r1 \"{}\"", "a".repeat(MAX_VALUE_CHARS + 1));
        assert_eq!(parse(&long), None);
    }

    #[test]
    fn plans_cap_at_eight_steps_and_honour_work() {
        let click = parse("click r1").unwrap();
        assert_eq!(plan(&vec![click.clone(); 8]).map(|s| s.len()), Ok(8));
        assert_eq!(plan(&vec![click.clone(); 9]), Err(Offer::Complex));
        assert_eq!(plan(&[click, Proposal::Work]), Err(Offer::Complex));
    }
}
