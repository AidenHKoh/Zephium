//! Fixed passive human-page survey. No caller-supplied program or page text.
const SOURCE: &str = include_str!("../host/work_human_wall.js");
pub(crate) fn program() -> String {
    format!("(() => {{ const code =\n{SOURCE}\n; return typeof code === 'number' && Number.isInteger(code) && code >= 0 && code <= 24 ? JSON.stringify(code) : null; }})()")
}
pub(crate) fn decode(wire: &str) -> Option<String> {
    let code = wire.parse::<u8>().ok()?;
    (code <= 24 && wire == code.to_string()).then(|| wire.to_owned())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn human_survey_is_fixed_bounded_and_returns_only_closed_codes() {
        assert!(program().len() <= 32 * 1024);
        assert!(program().contains(SOURCE));
        for code in 0..=24 {
            assert_eq!(decode(&code.to_string()), Some(code.to_string()));
        }
        for wire in [
            "false",
            "true",
            "00",
            " 0",
            "0.0",
            "25",
            "null",
            "private text",
        ] {
            assert!(decode(wire).is_none());
        }
    }
}
