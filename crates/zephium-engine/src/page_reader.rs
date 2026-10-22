//! The Ask panel's live Browse-tab reader: what the shell may ask of a tab's
//! document and the shapes of the answers. Reads, the selection, a marked
//! quote and the style outline all run in a private world of the tab's
//! current document through closed invocations; nothing page-derived is
//! ever executed, and every answer is decoded as hostile data.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zephium_core::ids::ItemId;

/// An sRGB colour.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// What a region of the page is, as the outline judged it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PageOutlineKind {
    /// A page header or banner.
    Header,
    /// Navigation.
    Navigation,
    /// The main landmark.
    Main,
    /// An article.
    Article,
    /// A sidebar.
    Aside,
    /// A page footer.
    Footer,
    /// A heading.
    Heading,
    /// Repeated items such as cards or comments.
    Content,
    /// A button or field.
    Control,
    /// Anything else.
    Other,
}

/// One region of the page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageOutlineNode {
    /// `tag`, one optional `#id`, up to three `.class` tokens.
    pub selector: String,
    /// What it is.
    pub kind: PageOutlineKind,
    /// How many elements match.
    pub count: u16,
    /// Its background, when opaque.
    pub background: Option<Rgb>,
    /// Its text colour.
    pub color: Option<Rgb>,
    /// Its font size.
    pub font_size_px: Option<u16>,
}

/// The page's structure and palette, for a style turn. Untrusted: the shell
/// admits it before use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageOutline {
    /// Regions, most important first.
    pub nodes: Vec<PageOutlineNode>,
    /// The page's background.
    pub background: Option<Rgb>,
    /// Its body text colour.
    pub text: Option<Rgb>,
    /// Its link colour.
    pub link: Option<Rgb>,
    /// Its accent.
    pub accent: Option<Rgb>,
    /// Its font families, most used first.
    pub fonts: Vec<String>,
    /// The page is already dark.
    pub dark: bool,
}

/// A form field's kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageFieldKind {
    /// Ordinary text.
    Text,
    /// A search box.
    Search,
    /// A number.
    Number,
    /// A password: its value is never read.
    Password,
}

/// What a passage is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageRole {
    /// A heading of the given level.
    Heading(u8),
    /// Running text.
    Text,
    /// A list item.
    ListItem,
    /// A table cell.
    Cell,
    /// A link; `same_site` when it stays on the page's site.
    Link {
        /// The destination is on the page's own host.
        same_site: bool,
    },
    /// A button.
    Button,
    /// A text field.
    Field(PageFieldKind),
    /// A checkbox or switch.
    Checkbox {
        /// Its current state.
        checked: bool,
    },
    /// A drop-down or list box.
    Select,
    /// Anything else.
    Other,
}

/// One passage of a read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagePassage {
    /// The read fragment it came from (`rN`).
    pub id: u16,
    /// What it is.
    pub role: PageRole,
    /// The visible text or accessible name.
    pub text: String,
    /// `text` is exactly what the page shows, complete.
    pub verbatim: bool,
    /// A field's current value; never set for a password.
    pub value: Option<String>,
    /// 0 the main content, 1 beside it, 2 the page's chrome.
    pub region: u8,
}

/// One read of a tab's document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageReadout {
    /// The committed navigation the read belongs to.
    pub epoch: u64,
    /// The page's title (untrusted).
    pub title: Option<String>,
    /// Passages in reading order.
    pub passages: Vec<PagePassage>,
}

/// Why a reader request did not happen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageReadFailure {
    /// The document is not ready, or predates the reader.
    NotReady,
    /// The tab navigated or closed.
    PageChanged,
    /// The page could not be read.
    Unavailable,
}

/// When the reader may install: the shell's switch and its site policy.
#[derive(Clone)]
pub struct PageReaderPolicy {
    /// Whether an address is on a site the reader never runs on.
    pub sensitive: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    /// The person used a tab while steps held it: mouse, keys or scroll.
    pub human_input: Arc<dyn Fn(ItemId) + Send + Sync>,
}

impl std::fmt::Debug for PageReaderPolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PageReaderPolicy")
    }
}

#[derive(Clone, Copy, Default)]
struct TabState {
    epoch: u64,
    ready: bool,
}

static TABS: Mutex<Option<HashMap<ItemId, TabState>>> = Mutex::new(None);

fn with_tabs<T>(apply: impl FnOnce(&mut HashMap<ItemId, TabState>) -> T) -> T {
    let mut tabs = TABS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    apply(tabs.get_or_insert_with(HashMap::new))
}

/// The committed navigation epoch of `tab`'s document, when it has one.
/// Epochs are process-unique and never reused.
pub fn committed_epoch(tab: ItemId) -> Option<u64> {
    with_tabs(|tabs| tabs.get(&tab).map(|state| state.epoch))
}

/// Whether `tab`'s current document can be read now.
pub fn readable(tab: ItemId) -> bool {
    with_tabs(|tabs| tabs.get(&tab).is_some_and(|state| state.ready))
}

#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
pub(crate) fn publish_commit(tab: ItemId, epoch: u64) {
    with_tabs(|tabs| {
        if tabs.len() >= 4_096 && !tabs.contains_key(&tab) {
            tabs.clear();
        }
        tabs.insert(
            tab,
            TabState {
                epoch,
                ready: false,
            },
        );
    });
}

#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
pub(crate) fn publish_ready(tab: ItemId, epoch: u64, ready: bool) {
    with_tabs(|tabs| {
        if let Some(state) = tabs.get_mut(&tab).filter(|state| state.epoch == epoch) {
            state.ready = ready;
        }
    });
}

#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
pub(crate) fn forget(tab: ItemId) {
    with_tabs(|tabs| {
        tabs.remove(&tab);
    });
    with_reads(|reads| {
        reads.remove(&tab);
    });
}

/// Why a step did not run or did not take.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageActFailure {
    /// The tab navigated or closed, or the step was planned on another page.
    PageChanged,
    /// The control is not one this step can use, or it changed.
    Unsuitable,
    /// A protected field, another site or a sensitive page.
    Refused,
    /// It ran without the expected effect, or could not run.
    Failed,
}

/// The read a step is planned on: its passages' nodes, as observed.
pub struct PageRead {
    epoch: u64,
    observation: zephium_agentic::SemanticObservation,
    frame: zephium_agentic::SemanticFrameJoin,
    references: Vec<(u16, zephium_agentic::SemanticReferenceId)>,
}

impl PageRead {
    /// The committed navigation it belongs to.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// The observation the passages came from.
    pub fn observation(&self) -> &zephium_agentic::SemanticObservation {
        &self.observation
    }

    /// Its main frame's authority.
    pub fn frame(&self) -> &zephium_agentic::SemanticFrameJoin {
        &self.frame
    }

    /// The observed node behind passage `rN`.
    pub fn reference(&self, passage: u16) -> Option<zephium_agentic::SemanticReferenceId> {
        self.references
            .iter()
            .find(|(id, _)| *id == passage)
            .map(|(_, reference)| *reference)
    }
}

static READS: Mutex<Option<HashMap<ItemId, Arc<PageRead>>>> = Mutex::new(None);

fn with_reads<T>(apply: impl FnOnce(&mut HashMap<ItemId, Arc<PageRead>>) -> T) -> T {
    let mut reads = READS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    apply(reads.get_or_insert_with(HashMap::new))
}

/// The latest read of `tab`'s document, which steps are planned on.
pub fn latest_read(tab: ItemId) -> Option<Arc<PageRead>> {
    with_reads(|reads| reads.get(&tab).cloned())
}

pub(crate) fn publish_read(tab: ItemId, read: PageRead) {
    with_reads(|reads| {
        if reads.len() >= 512 && !reads.contains_key(&tab) {
            reads.clear();
        }
        reads.insert(tab, Arc::new(read));
    });
}

#[cfg(feature = "agentic-browser")]
pub(crate) mod read {
    //! A runtime snapshot projected into passages through the semantic read
    //! policy: public content only, the standard budget, verbatim only when
    //! exact, and never a password's value.

    use zephium_agentic::{
        read_semantic_observation, ContextCapabilities, ContextCapability, ContextId,
        ContextIdentity, ContextJoin, ContextKind, ContextOperationId, ContextRegistry,
        ContextRunId, ContextSettlement, FrameId, SemanticCaptureInstant, SemanticFrameJoin,
        SemanticFrameTrust, SemanticInvocationId, SemanticObservationAssembler,
        SemanticObservationBudget, SemanticObservationId, SemanticObservationRequest,
        SemanticOrigin, SemanticReadAuthority, SemanticReadBudget, SemanticReadContent,
        SemanticReadField, SemanticReadSensitivityLimit, SemanticRole, SemanticRuntimeBudget,
        SemanticRuntimeInvocation, SemanticSnapshot, SemanticSnapshotGeneration, SemanticState,
    };

    use super::{PageFieldKind, PagePassage, PageReadFailure, PageRole};

    /// One request in flight: its observation request and frame authority,
    /// kept to assemble the answer.
    pub(crate) struct PendingRead {
        request: SemanticObservationRequest,
        frame: SemanticFrameJoin,
        site: Option<String>,
    }

    /// A private, never-shared context identity for one reader. It carries
    /// no capability beyond typed bookkeeping for observation decoding.
    pub(crate) fn reader_context() -> Option<ContextJoin> {
        let identity = ContextIdentity::new(
            ContextId::generate(),
            ContextRunId::generate(),
            1_u128.into(),
            ContextKind::Owned,
        );
        let mut registry = ContextRegistry::new();
        registry
            .reserve(
                identity,
                ContextCapabilities::try_new(ContextKind::Owned, &[ContextCapability::Observe])
                    .ok()?,
            )
            .ok()?;
        let operation = registry
            .begin_context(identity.id(), ContextOperationId::new(1)?)
            .ok()?;
        registry
            .settle_construction(identity.id(), operation, ContextSettlement::Applied)
            .ok()?;
        registry.join(identity.id()).ok()
    }

    /// One whole-page observation of the main frame at `url`.
    pub(crate) fn invocation(
        context: ContextJoin,
        url: &str,
        sequence: u64,
    ) -> Option<(SemanticRuntimeInvocation, PendingRead)> {
        let origin = SemanticOrigin::parse(url).ok()?;
        let frame = SemanticFrameJoin::try_new(
            context,
            FrameId::MAIN,
            context.frame_generation(),
            origin,
            SemanticFrameTrust::SameOrigin,
        )
        .ok()?;
        let request = SemanticObservationRequest::initial(
            SemanticObservationId::new(sequence)?,
            context,
            SemanticObservationBudget::WHOLE_PAGE,
        );
        let invocation = zephium_agentic::encode_semantic_runtime_invocation(
            &request,
            frame.clone(),
            SemanticInvocationId::new(sequence)?,
            SemanticSnapshotGeneration::new(sequence)?,
            SemanticRuntimeBudget::WHOLE_PAGE,
        )
        .ok()?;
        let site = zephium_agentic::ContextNavigationTarget::parse(url)
            .ok()
            .and_then(|target| zephium_agentic::registrable_site(&target));
        Some((
            invocation,
            PendingRead {
                request,
                frame,
                site,
            },
        ))
    }

    fn same_site(site: Option<&str>, target: &zephium_agentic::ContextNavigationTarget) -> bool {
        site.is_some_and(|site| zephium_agentic::registrable_site(target).as_deref() == Some(site))
    }

    /// Projects one runtime snapshot into passages in reading order.
    /// Passages, and the read kept for the steps that may follow it.
    pub(crate) fn passages(
        pending: PendingRead,
        snapshot: SemanticSnapshot,
        captured_at_ms: u64,
        epoch: u64,
    ) -> Result<(Vec<PagePassage>, super::PageRead), PageReadFailure> {
        let observation = SemanticObservationAssembler::new(pending.request, snapshot)
            .and_then(|assembler| assembler.finish())
            .map_err(|_| PageReadFailure::Unavailable)?;
        let read = read_semantic_observation(
            &observation,
            SemanticReadAuthority::Initial,
            SemanticCaptureInstant::from_millis(captured_at_ms),
            SemanticReadSensitivityLimit::PublicOnly,
            SemanticReadBudget::STANDARD,
        )
        .map_err(|_| PageReadFailure::Unavailable)?;
        let mut passages: Vec<(PagePassage, zephium_agentic::SemanticReferenceId)> = Vec::new();
        for fragment in read.fragments() {
            let reference = fragment.provenance().reference();
            let Ok(node) = observation.resolve_node(reference, &pending.frame) else {
                continue;
            };
            let role = match node.role() {
                SemanticRole::Heading => {
                    PageRole::Heading(node.heading_level().map_or(2, |level| level.get()))
                }
                SemanticRole::Paragraph | SemanticRole::Document | SemanticRole::Status => {
                    PageRole::Text
                }
                SemanticRole::ListItem => PageRole::ListItem,
                SemanticRole::Cell | SemanticRole::CellHeader => PageRole::Cell,
                SemanticRole::Link => PageRole::Link {
                    same_site: node
                        .link_destination()
                        .is_some_and(|target| same_site(pending.site.as_deref(), target)),
                },
                SemanticRole::Button | SemanticRole::MenuItem | SemanticRole::Tab => {
                    PageRole::Button
                }
                SemanticRole::Textbox => PageRole::Field(PageFieldKind::Text),
                SemanticRole::Searchbox => PageRole::Field(PageFieldKind::Search),
                SemanticRole::Spinbutton => PageRole::Field(PageFieldKind::Number),
                SemanticRole::Password => PageRole::Field(PageFieldKind::Password),
                SemanticRole::Checkbox | SemanticRole::Radio => PageRole::Checkbox {
                    checked: node.states().contains(SemanticState::Checked),
                },
                SemanticRole::Combobox | SemanticRole::Listbox => PageRole::Select,
                _ => PageRole::Other,
            };
            let field = matches!(role, PageRole::Field(_));
            let protected = matches!(role, PageRole::Field(PageFieldKind::Password));
            let previous = passages
                .last_mut()
                .filter(|(_, last)| *last == reference)
                .map(|(passage, _)| passage);
            match (fragment.field(), fragment.content()) {
                (
                    SemanticReadField::VisibleText | SemanticReadField::AccessibleName,
                    SemanticReadContent::Text(text),
                ) => {
                    if let Some(previous) = previous.filter(|p| p.text.is_empty()) {
                        previous.text = text.as_str().to_owned();
                        previous.verbatim = fragment.verbatim_text().is_some();
                        continue;
                    }
                    passages.push((
                        PagePassage {
                            id: fragment.id().get(),
                            role,
                            text: text.as_str().to_owned(),
                            verbatim: fragment.verbatim_text().is_some(),
                            value: None,
                            region: fragment.region(),
                        },
                        reference,
                    ));
                }
                (SemanticReadField::TextValue, SemanticReadContent::ValuePreview(preview))
                    if field && !protected =>
                {
                    let value = preview.text().to_owned();
                    match previous {
                        Some(previous) => previous.value = Some(value),
                        None => passages.push((
                            PagePassage {
                                id: fragment.id().get(),
                                role,
                                text: String::new(),
                                verbatim: false,
                                value: Some(value),
                                region: fragment.region(),
                            },
                            reference,
                        )),
                    }
                }
                (SemanticReadField::BooleanValue, SemanticReadContent::Boolean(checked)) => {
                    if let Some(previous) = previous {
                        if matches!(previous.role, PageRole::Checkbox { .. }) {
                            previous.role = PageRole::Checkbox { checked };
                        }
                    }
                }
                _ => {}
            }
        }
        let passages: Vec<(PagePassage, zephium_agentic::SemanticReferenceId)> = passages
            .into_iter()
            .filter(|(passage, _)| !passage.text.is_empty() || passage.value.is_some())
            .collect();
        let references = passages
            .iter()
            .map(|(passage, reference)| (passage.id, *reference))
            .collect();
        drop(read);
        Ok((
            passages.into_iter().map(|(passage, _)| passage).collect(),
            super::PageRead {
                epoch,
                observation,
                frame: pending.frame,
                references,
            },
        ))
    }

    /// One person-directed step bound to the read it was planned on and
    /// prepared against `current`, the page as it is right now.
    pub(crate) fn person_action(
        stored: &super::PageRead,
        current: &SemanticSnapshot,
        proposal: zephium_agentic::SemanticActionProposal,
        attempt: u64,
        requested_at_ms: u64,
    ) -> Result<
        (
            zephium_agentic::PersonDirectedAction,
            zephium_agentic::SemanticActionNativeRequest,
        ),
        super::PageActFailure,
    > {
        use super::PageActFailure;
        let batch = zephium_agentic::SemanticActionBatch::bind(
            zephium_agentic::SemanticActionBatchId::new(attempt).ok_or(PageActFailure::Failed)?,
            &stored.observation,
            std::slice::from_ref(&stored.frame),
            vec![proposal],
        )
        .map_err(|_| PageActFailure::Unsuitable)?;
        let prepared = batch
            .actions()
            .first()
            .ok_or(PageActFailure::Failed)?
            .prepare(current)
            .map_err(|_| PageActFailure::Unsuitable)?;
        zephium_agentic::PersonDirectedAction::prepare(
            &prepared,
            zephium_agentic::SemanticActionAttemptId::new(attempt).ok_or(PageActFailure::Failed)?,
            zephium_agentic::SemanticActionExecutionInstant::from_millis(requested_at_ms),
        )
        .map_err(|_| PageActFailure::Failed)
    }

    /// Why a prepared step did not take.
    pub(crate) fn act_failure(
        failure: zephium_agentic::SemanticActionFailure,
    ) -> super::PageActFailure {
        use zephium_agentic::SemanticActionFailure as Failure;
        match failure {
            Failure::CredentialBoundary | Failure::BlockedOrigin => super::PageActFailure::Refused,
            Failure::TargetChanged
            | Failure::TargetDisabled
            | Failure::TargetOccluded
            | Failure::UnsupportedInteraction => super::PageActFailure::Unsuitable,
            Failure::StaleReference => super::PageActFailure::PageChanged,
            _ => super::PageActFailure::Failed,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use zephium_agentic::{decode_semantic_snapshot, SemanticDecodeContext};

        fn snapshot(pending: &PendingRead, invocation: u64, wire: &str) -> SemanticSnapshot {
            decode_semantic_snapshot(
                SemanticDecodeContext::new(
                    SemanticInvocationId::new(invocation).expect("invocation"),
                    pending.frame.clone(),
                    SemanticSnapshotGeneration::new(invocation).expect("generation"),
                ),
                wire.as_bytes(),
            )
            .expect("snapshot")
        }

        #[test]
        fn passages_follow_the_public_read_and_withhold_passwords() {
            let context = reader_context().expect("context");
            let (invocation, pending) =
                super::invocation(context, "https://news.example.com/story?id=1", 3)
                    .expect("invocation");
            assert!(!invocation.as_str().is_empty());
            let wire = r#"{"v":1,"i":3,"g":3,"c":"complete","n":[
                {"k":1,"r":"document","o":16},
                {"k":2,"p":0,"r":"heading","l":1,"n":"Rivers rise"},
                {"k":3,"p":0,"r":"paragraph","t":"The river rose two metres overnight."},
                {"k":4,"p":0,"r":"password","n":"Password","q":"secret","v":{"k":"redacted"}},
                {"k":5,"p":0,"r":"textbox","n":"Email","q":"sensitive"}
            ]}"#;
            let (passages, kept) =
                passages(pending, snapshot_for(&invocation, wire), 1, 7).expect("read");
            assert_eq!(kept.epoch(), 7);
            assert!(passages.iter().all(|p| kept.reference(p.id).is_some()));
            let texts: Vec<&str> = passages.iter().map(|p| p.text.as_str()).collect();
            assert!(texts.contains(&"Rivers rise"), "{texts:?}");
            assert!(texts.contains(&"The river rose two metres overnight."));
            assert!(passages.iter().all(|p| p.value.is_none()));
            assert!(
                !texts.contains(&"Email"),
                "sensitive fields stay out of a public read"
            );
            let heading = passages
                .iter()
                .find(|p| p.text == "Rivers rise")
                .expect("heading");
            assert_eq!(heading.role, PageRole::Heading(1));
            assert!(heading.verbatim);
        }

        fn snapshot_for(invocation: &SemanticRuntimeInvocation, wire: &str) -> SemanticSnapshot {
            invocation.decode_result(wire.as_bytes()).expect("decode")
        }

        #[test]
        fn links_know_their_site_and_internal_pages_are_never_read() {
            let site = Some("example.co.uk");
            let url =
                |value: &str| zephium_agentic::ContextNavigationTarget::parse(value).expect("url");
            assert!(same_site(site, &url("https://example.co.uk/a")));
            assert!(same_site(site, &url("https://shop.example.co.uk/")));
            assert!(!same_site(site, &url("https://other.co.uk/")));
            let context = reader_context().expect("context");
            for refused in ["about:blank", "file:///etc/hosts", "zephium://settings"] {
                assert!(
                    super::invocation(context, refused, 1).is_none(),
                    "{refused}"
                );
            }
            let _ = snapshot;
        }
    }
}
