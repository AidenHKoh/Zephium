//! Bounded native DOM associations for rendered record media. Metadata only:
//! these historical references grant neither fetch nor pixel-capture authority.
use crate::semantic_wire::SemanticDecodeError;
use crate::{SemanticReferenceId, SemanticRole, SemanticSensitivity, SemanticSnapshot};
use serde::Deserialize;

/// Hard per-observation candidate ceiling; final read output uses at most twelve.
pub const MAX_SEMANTIC_MEDIA: usize = 48;
/// Maximum structural text references associated with one candidate.
pub const MAX_SEMANTIC_MEDIA_RELATIONS: usize = 8;

/// The rendered primitive or actual observed platform poster.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticMediaKind {
    /// An img element, including the browser-selected picture/srcset source.
    Image,
    /// A computed CSS image background.
    Background,
    /// A video's actual poster source; never inferred from a video URL.
    Poster,
    /// Native SVG pixels.
    Svg,
    /// Native canvas pixels.
    Canvas,
    /// A page's explicitly declared image metadata, usable only for that page.
    PageImage,
    /// Another explicit rendered image role without a retrievable source.
    Graphic,
    /// A bounded explicit DOM record, used only when no usable picture exists.
    #[serde(rename = "record")]
    RecordContainer,
}

/// Historical structural association in one exact native semantic snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticMediaEvidence {
    reference: SemanticReferenceId,
    kind: SemanticMediaKind,
    related: Vec<SemanticReferenceId>,
    loaded: bool,
}
impl SemanticMediaEvidence {
    /// Candidate's original opaque reference, never a new executable capability.
    pub const fn reference(&self) -> SemanticReferenceId {
        self.reference
    }
    /// Actual rendered source kind.
    pub const fn kind(&self) -> SemanticMediaKind {
        self.kind
    }
    /// Exact text/heading/link nodes sharing the nearest bounded DOM card.
    pub fn related(&self) -> &[SemanticReferenceId] {
        &self.related
    }
    /// Whether a native img had decoded when observed; other primitives still
    /// require native fetch/capture validation before producing bytes.
    pub const fn loaded(&self) -> bool {
        self.loaded
    }
    pub(crate) fn retain_references(&mut self, refs: &[SemanticReferenceId]) {
        self.related.retain(|reference| refs.contains(reference));
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawMedia {
    k: u64,
    t: SemanticMediaKind,
    r: Vec<u64>,
    l: bool,
}
pub(crate) fn decode(
    raw: Vec<RawMedia>,
    snapshot: &SemanticSnapshot,
) -> Result<Vec<SemanticMediaEvidence>, SemanticDecodeError> {
    if raw.len() > MAX_SEMANTIC_MEDIA {
        return Err(SemanticDecodeError::NodeContract);
    }
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for raw in raw {
        if !seen.insert(raw.k)
            || raw.r.len() > MAX_SEMANTIC_MEDIA_RELATIONS
            || raw
                .r
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != raw.r.len()
        {
            return Err(SemanticDecodeError::NodeContract);
        }
        let node = snapshot
            .nodes()
            .iter()
            .find(|node| node.key().get() == raw.k)
            .ok_or(SemanticDecodeError::NodeIdentity)?;
        if if raw.t == SemanticMediaKind::RecordContainer {
            !matches!(node.role(), SemanticRole::Document | SemanticRole::ListItem)
        } else {
            node.role() != SemanticRole::Image
        } {
            return Err(SemanticDecodeError::NodeContract);
        }
        let mut related = Vec::new();
        for key in raw.r {
            let source = snapshot
                .nodes()
                .iter()
                .find(|node| node.key().get() == key)
                .ok_or(SemanticDecodeError::NodeIdentity)?;
            if !matches!(
                source.role(),
                SemanticRole::Heading
                    | SemanticRole::Link
                    | SemanticRole::Paragraph
                    | SemanticRole::Document
                    | SemanticRole::ListItem
            ) {
                return Err(SemanticDecodeError::NodeContract);
            }
            if source.sensitivity() != SemanticSensitivity::Secret {
                related.push(source.reference());
            }
        }
        if node.sensitivity() == SemanticSensitivity::Secret {
            continue;
        }
        if raw.t != SemanticMediaKind::PageImage
            && node
                .geometry()
                .is_none_or(|rect| rect.width() < 64 || rect.height() < 48)
        {
            continue;
        }
        out.push(SemanticMediaEvidence {
            reference: node.reference(),
            kind: raw.t,
            related,
            loaded: raw.l,
        });
    }
    Ok(out)
}

/// Exact native candidate associated with an admitted source record. This is
/// historical data; the live retained owner must separately admit and revalidate it.
#[derive(Clone)]
pub struct SemanticRecordMediaPlan {
    pub(crate) crop: Option<SemanticRecordCropPermit>,
    pub(crate) record: u16,
    pub(crate) source: [u8; 32],
    pub(crate) frame: crate::SemanticFrameJoin,
    pub(crate) invocation: crate::SemanticInvocationId,
    pub(crate) snapshot: crate::SemanticSnapshotGeneration,
    pub(crate) key: u64,
    pub(crate) name_key: u64,
}
impl SemanticRecordMediaPlan {
    /// Canonical flattened source-row position, independent of model image labels.
    pub const fn record(&self) -> u16 {
        self.record
    }
    /// Exact admitted extraction and record identity fingerprint.
    pub const fn source(&self) -> [u8; 32] {
        self.source
    }
    /// Original frame/document identity, never a replacement native grant.
    pub fn frame(&self) -> &crate::SemanticFrameJoin {
        &self.frame
    }
    /// The isolated runtime accepts only these original numeric coordinates;
    /// no URL, script, cookie, or model-generated selector crosses this boundary.
    pub fn native_invocation(&self) -> String {
        format!(
            "M1:{{\"i\":{},\"g\":{},\"k\":{},\"n\":{}}}",
            self.invocation.get(),
            self.snapshot.get(),
            self.key,
            self.name_key
        )
    }
}
impl std::fmt::Debug for SemanticRecordMediaPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SemanticRecordMediaPlan")
            .field("record", &self.record)
            .field("source", &"[redacted]")
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct SemanticRecordCropPermit {
    pub(crate) context: crate::ContextJoin,
    pub(crate) account: crate::AgentContextAccountBinding,
    pub(crate) expires: std::time::Instant,
    pub(crate) remaining: std::sync::Arc<std::sync::atomic::AtomicU8>,
    spent: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl SemanticRecordMediaPlan {
    /// Local canvas capture is disabled unless the original live run carried
    /// its existing screenshot permission into this exact source plan.
    pub fn local_crop_permitted(&self, now: std::time::Instant) -> bool {
        self.crop.as_ref().is_some_and(|permit| {
            permit.context == self.frame.context()
                && permit.account.context() == permit.context
                && now < permit.expires
                && (permit.spent.load(std::sync::atomic::Ordering::Acquire)
                    || permit.remaining.load(std::sync::atomic::Ordering::Acquire) > 0)
        })
    }
}

impl SemanticRecordCropPermit {
    pub(crate) fn admit(
        context: crate::ContextJoin,
        original: crate::AgentContextAccountBinding,
        current: crate::AgentContextAccountBinding,
        permitted: bool,
        now: std::time::Instant,
        expires: std::time::Instant,
    ) -> Option<Self> {
        (permitted
            && original == current
            && current.context() == context
            && expires > now
            && expires <= now + std::time::Duration::from_secs(8))
        .then_some(Self {
            context,
            account: current,
            expires,
            remaining: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(12)),
            spent: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }
}

impl SemanticRecordCropPermit {
    pub(crate) fn for_record(&self) -> Self {
        Self {
            spent: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ..self.clone()
        }
    }
}
impl SemanticRecordMediaPlan {
    /// Reserves exactly one local capture from the original run's remaining
    /// native budget. Clones share the reservation; failure never refunds it.
    pub fn begin_local_crop(&self, now: std::time::Instant) -> bool {
        use std::sync::atomic::Ordering;
        if !self.local_crop_permitted(now) {
            return false;
        }
        let Some(permit) = &self.crop else {
            return false;
        };
        if permit
            .spent
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        if permit
            .remaining
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |left| {
                left.checked_sub(1)
            })
            .is_err()
        {
            permit.spent.store(false, Ordering::Release);
            return false;
        }
        true
    }
    /// Publication requires both the original permission and its exact capture reservation.
    pub fn local_crop_reserved(&self, now: std::time::Instant) -> bool {
        self.local_crop_permitted(now)
            && self
                .crop
                .as_ref()
                .is_some_and(|permit| permit.spent.load(std::sync::atomic::Ordering::Acquire))
    }
}

/// Closed source-stage counts, never source text, identity, URLs or pixel bytes.
/// These values diagnose optional image absence and confer no capture authority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SemanticRecordMediaDiagnostics {
    /// Explicit image nodes in the current semantic observation.
    pub observed_images: usize,
    /// Image nodes with native geometry meeting the unchanged minimum dimensions.
    pub eligible_image_geometry: usize,
    /// Structural candidates retained across admitted source observations.
    pub structural_candidates: usize,
    /// Candidates whose bounded native relation list is full.
    pub saturated_candidates: usize,
    /// Name source roles: heading, link, paragraph, document, list item, image, other.
    pub identity_roles: [usize; 7],
    /// Name source fields: accessible name, visible text, image source, other.
    pub identity_fields: [usize; 4],
    /// Exact image-node self provenance, independently of labels or URLs.
    pub self_image_pairs: usize,
    /// Admitted source rows considered for optional enrichment.
    pub records: usize,
    /// Name provenance fragments considered across those rows.
    pub identity_sources: usize,
    /// Candidate/source pairs from the exact same capture.
    pub same_capture_pairs: usize,
    /// Exact native structural relations to those name sources.
    pub related_pairs: usize,
}
