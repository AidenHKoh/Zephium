//! Release-excluded counts stripped before the shipping observation decoder.
use std::sync::atomic::{AtomicBool, Ordering};
static ENABLED: AtomicBool = AtomicBool::new(false);
pub(super) fn configure(public: bool, account: bool) {
    ENABLED.store(public && !account, Ordering::Release);
}
pub(super) fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

pub(super) fn source(original: &str) -> Option<String> {
    let replacements = [
        ("const snapshots=new Map(), style=", "let md=[0,0,0,0];const snapshots=new Map(), style="),
        ("function evidence(records, invocation, generation, helpers) {", "function evidence(records, invocation, generation, helpers) {md=[0,0,0,0];"),
        ("    let work=0;\n    const boundaries", "    let work=0;md[1]=candidates.length;\n    const exhausted=()=>{md[0]=2;md[2]=Math.min(work,16385);return [];};\n    const boundaries"),
        ("if(work>16384 || !images.length)return [];", "if(work>16384)return exhausted();if(!images.length){md[0]=1;return []; }"),
        ("    remember(invocation,generation,records,rows);", "    md[2]=Math.min(work,16385);md[3]=rows.length;remember(invocation,generation,records,rows);"),
        ("Object.freeze({evidence,invoke})", "Object.freeze({evidence,invoke,diagnostic:()=>md})"),
        ("  function mediaEvidence(records, request) {", "  let mediaDiagnostic=[0,0,0,0];\n  function mediaEvidence(records, request) {"),
        ("try { return globalThis.__zephiumRecordMediaV1.evidence(records,", "try { const result=globalThis.__zephiumRecordMediaV1.evidence(records,"),
        ("imageNaturalWidthGetter,pageTitleText}); } catch (_) { return []; }", "imageNaturalWidthGetter,pageTitleText});mediaDiagnostic=globalThis.__zephiumRecordMediaV1.diagnostic();return result; } catch (_) {mediaDiagnostic=[3,0,0,0]; return []; }"),
        ("if (utf8Length(enriched,request.b.w+1) <= request.b.w) value = enriched;", "if (utf8Length(enriched,request.b.w+1) <= request.b.w) value = enriched;else mediaDiagnostic[0]=4;"),
        ("    return encoded.value;", "    return 'MD1:'+mediaDiagnostic.join(',')+'\\n'+encoded.value;"),
    ];
    let mut source = original.to_owned();
    for (from, to) in replacements {
        if source.matches(from).count() != 1 {
            return None;
        }
        source = source.replacen(from, to, 1);
    }
    let start = source.find("  function evidence(")?;
    let end = source[start..].find("\n  function target(")? + start;
    let body = &source[start..end];
    let compact = "if(++work>16384)return [];";
    let spaced = "if (++work > 16384) return [];";
    let final_check = "if(work>16384)return [];";
    if body.matches(compact).count() != 2
        || body.matches(spaced).count() != 2
        || body.matches(final_check).count() != 1
    {
        return None;
    }
    let body = body
        .replace(compact, "if(++work>16384)return exhausted();")
        .replace(spaced, "if (++work > 16384) return exhausted();")
        .replace(final_check, "if(work>16384)return exhausted();");
    source.replace_range(start..end, &body);
    (source.len() <= original.len() + 4096).then_some(source)
}

/// Borrows precisely the original snapshot tail; invalid envelopes remain invalid
/// shipping input. No parsed counter can become a semantic node or receipt.
pub(super) fn split(bytes: &[u8]) -> Option<([u32; 4], &[u8])> {
    let rest = bytes.strip_prefix(b"MD1:")?;
    let end = rest.iter().take(48).position(|b| *b == b'\n')?;
    let header = std::str::from_utf8(&rest[..end]).ok()?;
    let values = header.split(',').collect::<Vec<_>>();
    if values.len() != 4 {
        return None;
    }
    let mut counts = [0; 4];
    for (value, out) in values.into_iter().zip(&mut counts) {
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *out = value.parse().ok()?;
    }
    if counts[0] > 4 || counts[1] > 1536 || counts[2] > 16385 || counts[3] > 48 {
        return None;
    }
    let payload = &rest[end + 1..];
    payload.starts_with(b"{").then_some((counts, payload))
}
#[allow(clippy::print_stderr)]
pub(super) fn normalize(bytes: &[u8]) -> &[u8] {
    if enabled() {
        if let Some((counts, payload)) = split(bytes) {
            eprintln!(
                "record-media-native: status={} candidates={} work={} rows={} content=redacted",
                counts[0], counts[1], counts[2], counts[3]
            );
            return payload;
        }
    }
    bytes
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_bounded_closed_counts_are_stripped_and_payload_is_exact() {
        let original =
            b"{\"v\":1,\"i\":7,\"g\":9,\"c\":\"complete\",\"n\":[{\"k\":1,\"r\":\"document\"}]}";
        for status in 0..=4 {
            let mut wrapped = format!("MD1:{status},24,16385,48\n").into_bytes();
            wrapped.extend_from_slice(original);
            let (counts, tail) = split(&wrapped).unwrap();
            assert_eq!(tail, original);
            assert!(std::ptr::eq(
                tail.as_ptr(),
                wrapped[wrapped.len() - original.len()..].as_ptr()
            ));
            assert_eq!(counts, [status, 24, 16385, 48]);
        }
        for invalid in [
            "MD1:5,0,0,0\n{}",
            "MD1:0,1537,0,0\n{}",
            "MD1:0,0,16386,0\n{}",
            "MD1:0,0,0,49\n{}",
            "MD1:0,0,0,0,0\n{}",
            "MD1:secret,0,0,0\n{}",
            "MD1:0,0,0,0\nE1:unsupported_scope",
        ] {
            assert!(split(invalid.as_bytes()).is_none());
        }
    }
    #[test]
    fn account_activity_disables_activation_and_normalization() {
        let bytes = b"MD1:0,1,2,3\n{}";
        for (public, account) in [(false, false), (true, false), (false, true), (true, true)] {
            configure(public, account);
            assert_eq!(enabled(), public && !account);
            assert_eq!(
                normalize(bytes),
                if public && !account {
                    b"{}".as_slice()
                } else {
                    bytes.as_slice()
                }
            );
        }
        configure(false, false);
    }
    #[test]
    fn exact_source_anchors_fail_closed_without_changing_shipping_program() {
        let original = zephium_agentic::SEMANTIC_RUNTIME_PROGRAM.installation_source();
        let instrumented = source(original).unwrap();
        assert!(instrumented.contains("MD1:"));
        assert!(!original.contains("MD1:"));
        assert!(source(&original.replace(
            "function mediaEvidence(records, request)",
            "function mediaEvidenceChanged(records, request)"
        ))
        .is_none());
        assert!(!instrumented.contains("console.log"));
    }
}
