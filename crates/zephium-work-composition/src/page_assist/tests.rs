use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::AtomicUsize;
use std::time::{Duration, Instant};

use zephium_agentic::lead::{LeadClient, LeadSecret, LeadTarget};
use zephium_app::work_models::WorkCallSource;
use zephium_core::work::model::WorkModelClient;
use zephium_core::work::model::{
    WorkModelEntry, WorkModelError, WorkModelEvent, WorkModelFuture, WorkModelOutcome,
    WorkModelPart, WorkModelProvider, WorkModelRef, WorkModelRequest, WorkModelStop,
    WorkModelSupports, WorkModelUsage, WorkModelWire,
};

use super::*;

const TAB: &str = "tab-1";
const PASSWORD: &str = "hunter2-correct-horse-battery";

#[derive(Default)]
struct PageState {
    facts: Option<TabFacts>,
    readout: Option<Readout>,
    selection: Option<String>,
    highlights: Vec<String>,
    steps: HashMap<u16, ResolvedStep>,
    acted: Vec<StepRequest>,
    applied: Vec<(String, String)>,
    reads: usize,
}

#[derive(Default)]
struct FakePage(Mutex<PageState>);

impl FakePage {
    fn lock(&self) -> MutexGuard<'_, PageState> {
        self.0.lock().unwrap()
    }
}

impl PagePort for FakePage {
    fn supported(&self) -> bool {
        true
    }
    fn facts(&self, tab: &str) -> Option<TabFacts> {
        (tab == TAB).then(|| self.lock().facts.clone()).flatten()
    }
    fn read<'a>(
        &'a self,
        _tab: &'a str,
        _epoch: u64,
    ) -> BoxFuture<'a, Result<Readout, ReadFailure>> {
        let mut state = self.lock();
        state.reads += 1;
        let readout = state.readout.clone().ok_or(ReadFailure::Unavailable);
        Box::pin(async move { readout })
    }
    fn selection<'a>(&'a self, _tab: &'a str, _epoch: u64) -> BoxFuture<'a, Option<String>> {
        let selection = self.lock().selection.clone();
        Box::pin(async move { selection })
    }
    fn highlight(&self, _tab: &str, _epoch: u64, text: &str) -> bool {
        self.lock().highlights.push(text.to_owned());
        true
    }
    fn resolve(
        &self,
        _tab: &str,
        _epoch: u64,
        step: &StepRequest,
    ) -> Result<ResolvedStep, StepFailure> {
        step.target
            .and_then(|target| self.lock().steps.get(&target).cloned())
            .ok_or(StepFailure::Unsuitable)
    }
    fn act<'a>(
        &'a self,
        _tab: &'a str,
        _epoch: u64,
        step: &'a StepRequest,
    ) -> BoxFuture<'a, Result<(), StepFailure>> {
        self.lock().acted.push(step.clone());
        Box::pin(async { Ok(()) })
    }
    fn apply_styles(&self, _profile: &ProfileId, site: &str, css: &str) {
        self.lock().applied.push((site.to_owned(), css.to_owned()));
    }
}

/// A scripted model: streams `deltas`, then ends with `result`.
#[derive(Clone)]
struct Script {
    deltas: Vec<String>,
    result: Result<(), WorkModelError>,
    /// Waits forever after streaming, until the call is dropped.
    hang: bool,
}

struct FakeClient {
    script: Script,
    requests: Arc<Mutex<Vec<String>>>,
    dropped: Arc<AtomicBool>,
}

struct DropFlag(Arc<AtomicBool>);
impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn request_text(request: &WorkModelRequest) -> String {
    let mut text: String = request
        .system
        .iter()
        .map(|block| block.text.clone())
        .collect();
    for message in &request.messages {
        let parts = match message {
            zephium_core::work::model::WorkModelMessage::User(parts)
            | zephium_core::work::model::WorkModelMessage::Assistant(parts) => parts,
            zephium_core::work::model::WorkModelMessage::ToolResults(_) => continue,
        };
        for part in parts {
            if let WorkModelPart::Text(part) = part {
                text.push_str(part);
                text.push('\n');
            }
        }
    }
    text
}

impl WorkModelClient for FakeClient {
    fn call<'a>(
        &'a self,
        request: WorkModelRequest,
        events: &'a (dyn Fn(WorkModelEvent) + Send + Sync),
    ) -> WorkModelFuture<'a> {
        self.requests.lock().unwrap().push(request_text(&request));
        let script = self.script.clone();
        let flag = DropFlag(self.dropped.clone());
        Box::pin(async move {
            let _flag = flag;
            for delta in script.deltas {
                events(WorkModelEvent::Text(delta));
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            if script.hang {
                std::future::pending::<()>().await;
            }
            script.result.map(|()| WorkModelOutcome {
                stop: WorkModelStop::EndTurn,
                usage: WorkModelUsage {
                    input_tokens: 1_000,
                    output_tokens: 100,
                    cost_micros: Some(4_000),
                    ..WorkModelUsage::default()
                },
                assistant: Vec::new(),
            })
        })
    }
}

fn entry(provider: WorkModelProvider, wire: WorkModelWire) -> WorkModelEntry {
    WorkModelEntry {
        id: "test/model".into(),
        model: WorkModelRef {
            provider,
            wire,
            model: "model".into(),
        },
        display_name: "Luna".into(),
        roles: Vec::new(),
        recommended: false,
        context_window: 128_000,
        max_output: 4_000,
        supports: WorkModelSupports {
            tools: false,
            vision: false,
            prompt_cache: true,
            reasoning: false,
            native_search: false,
        },
        price: None,
    }
}

struct FakeModels {
    entry: WorkModelEntry,
    client: Mutex<Option<Arc<dyn WorkModelClient>>>,
    resolved: AtomicUsize,
}

impl ModelSource for FakeModels {
    fn entry<'a>(&'a self, _profile: &'a ProfileId) -> BoxFuture<'a, Option<WorkModelEntry>> {
        let entry = self.entry.clone();
        Box::pin(async move { Some(entry) })
    }
    fn resolve<'a>(
        &'a self,
        _profile: &'a ProfileId,
        _source: WorkCallSource,
    ) -> BoxFuture<'a, Result<ResolvedModel, WorkModelError>> {
        self.resolved.fetch_add(1, Ordering::AcqRel);
        let client = self.client.lock().unwrap().clone();
        let entry = self.entry.clone();
        Box::pin(async move {
            Ok(ResolvedModel {
                entry,
                client: client.ok_or(WorkModelError::MissingKey)?,
            })
        })
    }
}

#[derive(Default)]
struct Sink(Mutex<Vec<PageAssistChangedV1>>);
impl EventSink for Sink {
    fn emit(&self, event: PageAssistChangedV1) {
        self.0.lock().unwrap().push(event);
    }
}

struct RealClock(Instant);
impl Clock for RealClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

struct Switch(AtomicBool);
impl Settings for Switch {
    fn ai_enabled(&self, _profile: &ProfileId) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

struct Work;
impl WorkHandoff for Work {
    fn begin<'a>(
        &'a self,
        _profile: &'a ProfileId,
        question: String,
        url: String,
    ) -> BoxFuture<'a, Option<String>> {
        Box::pin(
            async move { (!question.is_empty() && !url.is_empty()).then(|| "work-1".to_owned()) },
        )
    }
}

#[derive(Default)]
struct Styles(Mutex<Vec<StoredStyle>>);
impl StyleStore for Styles {
    fn load(&self, _profile: &ProfileId) -> Vec<StoredStyle> {
        self.0.lock().unwrap().clone()
    }
    fn save(&self, _profile: &ProfileId, styles: &[StoredStyle]) {
        *self.0.lock().unwrap() = styles.to_vec();
    }
}

struct Harness {
    service: PageAssist,
    page: Arc<FakePage>,
    models: Arc<FakeModels>,
    sink: Arc<Sink>,
    ai: Arc<Switch>,
    styles: Arc<Styles>,
    requests: Arc<Mutex<Vec<String>>>,
    dropped: Arc<AtomicBool>,
}

fn passage(id: u16, role: Role, text: &str) -> Passage {
    Passage {
        id,
        role,
        text: text.into(),
        verbatim: true,
        value: None,
        region: 0,
    }
}

fn readout() -> Readout {
    Readout {
        epoch: 1,
        title: Some("Pricing".into()),
        passages: vec![
            passage(1, Role::Heading(1), "Pricing"),
            passage(
                2,
                Role::Text,
                "The Pro plan costs $12 per month, billed yearly.",
            ),
            passage(
                3,
                Role::Text,
                "Cancel anytime within 30 days for a full refund.",
            ),
            Passage {
                value: Some(PASSWORD.into()),
                ..passage(4, Role::Field(FieldKind::Password), "Password")
            },
            passage(5, Role::Button, "Place order"),
            passage(6, Role::Select, "Sort by"),
            passage(7, Role::Link { same_site: false }, "Partner site"),
        ],
    }
}

fn harness(provider: WorkModelProvider, script: Option<Script>) -> Harness {
    let page = Arc::new(FakePage::default());
    {
        let mut state = page.lock();
        state.facts = Some(TabFacts {
            profile: ProfileId::from(7),
            url: Some("https://shop.example.com/pricing".into()),
            private: false,
            viewer: false,
            loading: false,
            epoch: 1,
        });
        state.readout = Some(readout());
    }
    let requests = Arc::new(Mutex::new(Vec::new()));
    let dropped = Arc::new(AtomicBool::new(false));
    let client: Option<Arc<dyn WorkModelClient>> = script.map(|script| {
        Arc::new(FakeClient {
            script,
            requests: requests.clone(),
            dropped: dropped.clone(),
        }) as Arc<dyn WorkModelClient>
    });
    let models = Arc::new(FakeModels {
        entry: entry(provider, WorkModelWire::ChatCompletions),
        client: Mutex::new(client),
        resolved: AtomicUsize::new(0),
    });
    let sink = Arc::new(Sink::default());
    let ai = Arc::new(Switch(AtomicBool::new(true)));
    let styles = Arc::new(Styles::default());
    let service = PageAssist::new(Ports {
        page: page.clone(),
        models: models.clone(),
        events: sink.clone(),
        clock: Arc::new(RealClock(Instant::now())),
        settings: ai.clone(),
        work: Arc::new(Work),
        styles: styles.clone(),
    });
    Harness {
        service,
        page,
        models,
        sink,
        ai,
        styles,
        requests,
        dropped,
    }
}

fn script(answer: &str) -> Script {
    // Split mid-line to prove the parser rejoins the stream.
    let middle = answer.len() / 2;
    let middle = (0..=middle)
        .rev()
        .find(|i| answer.is_char_boundary(*i))
        .unwrap_or(0);
    Script {
        deltas: vec![answer[..middle].to_owned(), answer[middle..].to_owned()],
        result: Ok(()),
        hang: false,
    }
}

fn ask(intent: PageAssistIntentV1, text: &str) -> PageAssistAskV1 {
    PageAssistAskV1 {
        tab: TAB.into(),
        text: text.into(),
        intent,
        suggestion: None,
    }
}

impl Harness {
    fn events(&self) -> Vec<PageAssistEventV1> {
        self.sink
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|event| event.kind.clone())
            .collect()
    }

    fn last(&self) -> PageAssistEventV1 {
        self.events().last().cloned().unwrap()
    }

    async fn run(&self, request: PageAssistAskV1) -> u32 {
        let (turn, task) = self.service.ask(request).await.expect("admitted");
        task.await;
        turn
    }

    fn parts(&self) -> Vec<PageAssistPartV1> {
        self.events()
            .into_iter()
            .filter_map(|event| match event {
                PageAssistEventV1::Part { part } => Some(part),
                _ => None,
            })
            .collect()
    }
}

async fn wait_for(harness: &Harness, done: impl Fn(&[PageAssistEventV1]) -> bool) {
    for _ in 0..400 {
        if done(&harness.events()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out: {:?}", harness.events());
}

#[tokio::test]
async fn answers_with_validated_citations_in_order() {
    let h = harness(
        WorkModelProvider::OpenAi,
        Some(script(
            "Here you go\n- Refunds within 30 days {r3}\n= Pro costs $12 a month. {r2,r99}\n# Price :: $12/month {r2}\n! Not on this page: taxes.\n",
        )),
    );
    let turn = h
        .run(ask(PageAssistIntentV1::Ask, "How much is Pro?"))
        .await;
    let events = h.events();
    assert!(
        matches!(events[0], PageAssistEventV1::Started { ref turn } if turn.question == "How much is Pro?")
    );
    assert_eq!(
        events[1],
        PageAssistEventV1::Status {
            phase: PageAssistPhaseV1::Reading
        }
    );
    assert_eq!(
        events[2],
        PageAssistEventV1::Status {
            phase: PageAssistPhaseV1::Read { passages: 7 }
        }
    );
    assert_eq!(
        events[3],
        PageAssistEventV1::Status {
            phase: PageAssistPhaseV1::Answering
        }
    );
    let parts = h.parts();
    assert_eq!(parts[0].kind, PageAssistPartKindV1::Answer);
    assert_eq!(parts[0].cites.len(), 1, "r99 is not in the read");
    assert_eq!(
        parts[0].cites[0].excerpt,
        "The Pro plan costs $12 per month, billed yearly."
    );
    assert_eq!(parts[1].kind, PageAssistPartKindV1::Point);
    assert_eq!(parts[1].cites[0].n, 2);
    assert_eq!(parts[2].label.as_deref(), Some("Price"));
    assert_eq!(parts[2].cites[0].n, 1, "the same passage keeps its number");
    assert!(parts[3].cites.is_empty());
    assert!(events.len() <= 15);
    assert_eq!(
        h.last(),
        PageAssistEventV1::Done {
            cost: Some(PageAssistCostV1::Byok {
                model: "Luna".into(),
                provider: "OpenAI".into(),
                usd: Some("0.004".into())
            })
        }
    );
    let seqs: Vec<u32> = h.sink.0.lock().unwrap().iter().map(|e| e.seq).collect();
    assert!(seqs.windows(2).all(|pair| pair[1] == pair[0] + 1));
    assert!(h.service.cite(TAB, turn, 1));
    assert_eq!(
        h.page.lock().highlights,
        vec!["The Pro plan costs $12 per month, billed yearly."]
    );
    let status = h.service.status(TAB).await;
    let Some(PageAssistEntryV1::Turn { turn: view }) = status.entries.first() else {
        panic!()
    };
    assert_eq!(view.state, PageAssistTurnStateV1::Done);
    assert_eq!(view.parts, parts);
}

#[tokio::test]
async fn the_password_never_reaches_the_request_and_page_text_stays_fenced() {
    let h = harness(WorkModelProvider::OpenAi, Some(script("= ok {r2}")));
    h.run(ask(PageAssistIntentV1::Ask, "What is on this page?"))
        .await;
    let requests = h.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].contains(PASSWORD));
    assert!(requests[0].contains("@r4 field:password \"Password\"\n"));
}

#[tokio::test]
async fn refusals_send_nothing() {
    type Setup = Box<dyn Fn(&Harness)>;
    let cases: Vec<(Setup, PageAssistFaultV1)> = vec![
        (
            Box::new(|h: &Harness| h.ai.0.store(false, Ordering::Release)),
            PageAssistFaultV1::AiOff,
        ),
        (
            Box::new(|h: &Harness| h.page.lock().facts.as_mut().unwrap().private = true),
            PageAssistFaultV1::Private,
        ),
        (
            Box::new(|h: &Harness| {
                h.page.lock().facts.as_mut().unwrap().url = Some("file:///tmp/a.html".into())
            }),
            PageAssistFaultV1::NotWeb,
        ),
        (
            Box::new(|h: &Harness| h.page.lock().facts.as_mut().unwrap().url = None),
            PageAssistFaultV1::NotWeb,
        ),
        (
            Box::new(|h: &Harness| h.page.lock().facts.as_mut().unwrap().viewer = true),
            PageAssistFaultV1::NotWeb,
        ),
        (
            Box::new(|h: &Harness| {
                h.page.lock().facts.as_mut().unwrap().url = Some("https://www.chase.com/".into())
            }),
            PageAssistFaultV1::Sensitive,
        ),
        (
            Box::new(|h: &Harness| h.page.lock().facts.as_mut().unwrap().loading = true),
            PageAssistFaultV1::Loading,
        ),
    ];
    for (setup, expected) in cases {
        let h = harness(WorkModelProvider::OpenAi, Some(script("= ok")));
        setup(&h);
        let refused = h
            .service
            .ask(ask(PageAssistIntentV1::Ask, "hi"))
            .await
            .err();
        assert_eq!(refused, Some(expected));
        assert_eq!(h.page.lock().reads, 0, "{expected:?} read the page");
        assert_eq!(h.models.resolved.load(Ordering::Acquire), 0);
        assert!(h.requests.lock().unwrap().is_empty());
        assert!(h.events().is_empty());
    }
    let h = harness(WorkModelProvider::OpenAi, Some(script("= ok")));
    let too_long = "a".repeat(PAGE_ASSIST_MAX_QUESTION_BYTES + 1);
    assert_eq!(
        h.service
            .ask(ask(PageAssistIntentV1::Ask, &too_long))
            .await
            .err(),
        Some(PageAssistFaultV1::Invalid)
    );
    assert_eq!(
        h.service
            .ask(ask(PageAssistIntentV1::Ask, "  "))
            .await
            .err(),
        Some(PageAssistFaultV1::Invalid)
    );
    assert_eq!(
        h.service
            .ask(ask(PageAssistIntentV1::Selection, "this?"))
            .await
            .err(),
        Some(PageAssistFaultV1::NoSelection)
    );
    assert!(h.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn model_faults_map_to_closed_words() {
    for (error, provider, expected) in [
        (
            WorkModelError::OverBudget,
            WorkModelProvider::OpenAi,
            WorkModelFaultV1::ProviderCredit,
        ),
        (
            WorkModelError::Hosted {
                refusal: zephium_core::work::model::HostedRefusal::DailyLimit,
                retry_after_ms: None,
            },
            WorkModelProvider::Cloud,
            WorkModelFaultV1::DailyLimit,
        ),
        (
            WorkModelError::OverBudget,
            WorkModelProvider::Cloud,
            WorkModelFaultV1::HostedUsage,
        ),
        (
            WorkModelError::RateLimited {
                retry_after_ms: Some(9_000),
            },
            WorkModelProvider::OpenAi,
            WorkModelFaultV1::RateLimited,
        ),
    ] {
        let h = harness(
            provider,
            Some(Script {
                deltas: vec!["= partial".into()],
                result: Err(error),
                hang: false,
            }),
        );
        h.run(ask(PageAssistIntentV1::Ask, "q")).await;
        let PageAssistEventV1::Failed {
            fault: PageAssistFaultV1::Model { fault, .. },
        } = h.last()
        else {
            panic!("{:?}", h.events());
        };
        assert_eq!(fault, expected);
    }
}

#[tokio::test]
async fn stop_mid_stream_cancels_the_call_and_sends_nothing_more() {
    let h = harness(
        WorkModelProvider::OpenAi,
        Some(Script {
            deltas: vec!["= first part\n".into()],
            result: Ok(()),
            hang: true,
        }),
    );
    let (turn, task) = h
        .service
        .ask(ask(PageAssistIntentV1::Ask, "q"))
        .await
        .unwrap();
    let running = tokio::spawn(task);
    wait_for(&h, |events| {
        events
            .iter()
            .any(|e| matches!(e, PageAssistEventV1::Part { .. }))
    })
    .await;
    assert!(h.service.stop(TAB));
    running.await.unwrap();
    assert_eq!(h.last(), PageAssistEventV1::Cancelled);
    assert!(
        h.dropped.load(Ordering::Acquire),
        "dropping the call cancels it"
    );
    assert!(!h.service.stop(TAB));
    let status = h.service.status(TAB).await;
    let Some(PageAssistEntryV1::Turn { turn: view }) = status.entries.first() else {
        panic!()
    };
    assert_eq!(
        (view.turn, view.state),
        (turn, PageAssistTurnStateV1::Cancelled)
    );
}

#[tokio::test]
async fn navigation_mid_answer_ends_the_turn_and_divides_the_thread() {
    let h = harness(
        WorkModelProvider::OpenAi,
        Some(Script {
            deltas: vec!["= first\n".into()],
            result: Ok(()),
            hang: true,
        }),
    );
    let (turn, task) = h
        .service
        .ask(ask(PageAssistIntentV1::Ask, "q"))
        .await
        .unwrap();
    let running = tokio::spawn(task);
    wait_for(&h, |events| {
        events
            .iter()
            .any(|e| matches!(e, PageAssistEventV1::Part { .. }))
    })
    .await;
    h.service.navigated(TAB, Some("https://other.example.org/"));
    running.await.unwrap();
    let events = h.events();
    assert!(events.contains(&PageAssistEventV1::Divider {
        site: "example.org".into(),
        cross_site: true
    }));
    assert_eq!(
        h.last(),
        PageAssistEventV1::Failed {
            fault: PageAssistFaultV1::PageChanged
        }
    );
    assert!(
        !h.service.cite(TAB, turn, 1),
        "citations are inert after navigation"
    );
}

#[tokio::test]
async fn cross_site_navigation_starts_fresh_model_context() {
    let h = harness(WorkModelProvider::OpenAi, Some(script("= one {r2}")));
    h.run(ask(PageAssistIntentV1::Ask, "first question")).await;
    h.run(ask(PageAssistIntentV1::Ask, "second question")).await;
    assert!(h.requests.lock().unwrap()[1].contains("first question"));
    h.service
        .navigated(TAB, Some("https://elsewhere.example.net/"));
    h.page.lock().facts.as_mut().unwrap().url = Some("https://elsewhere.example.net/".into());
    h.run(ask(PageAssistIntentV1::Ask, "third question")).await;
    let requests = h.requests.lock().unwrap();
    assert!(!requests[2].contains("first question"));
    assert!(!requests[2].contains("second question"));
}

#[tokio::test]
async fn summaries_are_cached_by_page() {
    let h = harness(
        WorkModelProvider::OpenAi,
        Some(script("= A summary {r2}\n- point {r3}")),
    );
    let summarize = PageAssistAskV1 {
        suggestion: Some(PageAssistSuggestionV1::Summarize),
        ..ask(PageAssistIntentV1::Summarize, "")
    };
    h.run(summarize.clone()).await;
    let turn = h.run(summarize).await;
    assert_eq!(
        h.requests.lock().unwrap().len(),
        1,
        "the second summary is served from memory"
    );
    assert_eq!(h.last(), PageAssistEventV1::Done { cost: None });
    assert!(h.service.cite(TAB, turn, 1));
}

#[tokio::test]
async fn complex_requests_offer_work_without_a_request() {
    let h = harness(WorkModelProvider::OpenAi, Some(script("= ok")));
    let turn = h
        .run(ask(
            PageAssistIntentV1::Ask,
            "Is this cheaper on other sites?",
        ))
        .await;
    assert!(h.events().contains(&PageAssistEventV1::Offer {
        offer: PageAssistOfferV1::MultiPage
    }));
    assert!(h.requests.lock().unwrap().is_empty());
    let handoff = h.service.handoff(TAB, turn).await;
    assert_eq!(handoff.work.as_deref(), Some("work-1"));
}

#[tokio::test]
async fn commit_steps_wait_for_the_person_and_protected_fields_are_refused() {
    let h = harness(
        WorkModelProvider::OpenAi,
        Some(script(
            "= Sorting, then ordering\n> select r6 \"Price\"\n> click r5\n",
        )),
    );
    {
        let mut page = h.page.lock();
        page.steps.insert(
            6,
            ResolvedStep {
                label: "Choose \"Price\" in \"Sort by\"".into(),
                effect: StepEffect::Draft,
            },
        );
        page.steps.insert(
            5,
            ResolvedStep {
                label: "Click \"Place order\"".into(),
                effect: StepEffect::Commit(WorkConfirmCategoryV1::Purchase),
            },
        );
    }
    let (turn, task) = h
        .service
        .ask(ask(PageAssistIntentV1::Act, "sort by price and order"))
        .await
        .unwrap();
    let running = tokio::spawn(task);
    wait_for(&h, |events| events.iter().any(|e| matches!(e, PageAssistEventV1::Step { step } if step.state == PageAssistStepStateV1::Confirm))).await;
    assert_eq!(h.page.lock().acted.len(), 1, "the commit step has not run");
    assert!(h.service.decide(TAB, turn, PageAssistDecisionV1::Approve));
    running.await.unwrap();
    assert_eq!(h.page.lock().acted.len(), 2);
    assert!(matches!(h.last(), PageAssistEventV1::Done { .. }));

    let h = harness(
        WorkModelProvider::OpenAi,
        Some(script("= Typing\n> fill r4 \"secret\"\n> open r7\n")),
    );
    h.page.lock().steps.insert(
        4,
        ResolvedStep {
            label: "Type".into(),
            effect: StepEffect::Draft,
        },
    );
    h.run(ask(PageAssistIntentV1::Act, "type my password"))
        .await;
    assert!(
        h.page.lock().acted.is_empty(),
        "protected fields and other sites are never acted on"
    );
    assert_eq!(
        h.last(),
        PageAssistEventV1::Failed {
            fault: PageAssistFaultV1::StepFailed
        }
    );
}

#[tokio::test]
async fn human_input_pauses_steps_until_continue() {
    let h = harness(
        WorkModelProvider::OpenAi,
        Some(script("= ok\n> click r5\n> select r6 \"Price\"\n")),
    );
    {
        let mut page = h.page.lock();
        page.steps.insert(
            5,
            ResolvedStep {
                label: "Order".into(),
                effect: StepEffect::Commit(WorkConfirmCategoryV1::Purchase),
            },
        );
        page.steps.insert(
            6,
            ResolvedStep {
                label: "Sort".into(),
                effect: StepEffect::Draft,
            },
        );
    }
    let (turn, task) = h
        .service
        .ask(ask(PageAssistIntentV1::Act, "order and sort"))
        .await
        .unwrap();
    let running = tokio::spawn(task);
    wait_for(&h, |events| events.iter().any(|e| matches!(e, PageAssistEventV1::Step { step } if step.state == PageAssistStepStateV1::Confirm))).await;
    h.service.human_input(TAB);
    assert!(h.service.decide(TAB, turn, PageAssistDecisionV1::Approve));
    wait_for(&h, |events| {
        events
            .iter()
            .any(|e| matches!(e, PageAssistEventV1::Paused { .. }))
    })
    .await;
    assert_eq!(
        h.page.lock().acted.len(),
        1,
        "the next step waits for Continue"
    );
    let reads = h.page.lock().reads;
    assert!(h.service.resume(TAB, turn));
    running.await.unwrap();
    assert_eq!(
        h.page.lock().reads,
        reads + 1,
        "Continue reads the page again"
    );
    assert_eq!(h.page.lock().acted.len(), 2);
    assert!(matches!(h.last(), PageAssistEventV1::Done { .. }));
}

#[tokio::test]
async fn styles_apply_only_after_approval_and_reject_injection() {
    let h = harness(
        WorkModelProvider::OpenAi,
        Some(script(
            "= Darker pages\n@css\nbody{background-color:#111!important;color:#eee}\n@end\n",
        )),
    );
    let turn = {
        let (turn, task) = h
            .service
            .ask(ask(PageAssistIntentV1::Style, "make it dark"))
            .await
            .unwrap();
        let running = tokio::spawn(task);
        wait_for(&h, |events| events.iter().any(|e| matches!(e, PageAssistEventV1::Style { style } if style.state == PageStyleStateV1::Proposed))).await;
        assert!(
            h.page.lock().applied.is_empty(),
            "never without the person's click"
        );
        assert!(h.service.decide(TAB, turn, PageAssistDecisionV1::Approve));
        running.await.unwrap();
        turn
    };
    assert!(turn > 0);
    let applied = h.page.lock().applied.clone();
    assert_eq!(
        applied,
        vec![(
            "example.com".to_owned(),
            "body{background-color:#111!important;color:#eee}\n".to_owned()
        )]
    );
    let listed = h.service.styles(TAB);
    assert_eq!(listed.styles.len(), 1);
    let id = listed.styles[0].id.clone();
    assert_eq!(h.styles.0.lock().unwrap().len(), 1);
    let off = h.service.toggle_style(TAB, &id, false);
    assert!(!off.styles[0].enabled);
    assert_eq!(h.page.lock().applied.last().unwrap().1, "");
    assert_eq!(
        h.service.all_styles(&ProfileId::from(7)).sites[0].site,
        "example.com"
    );
    assert!(h.service.remove_style(TAB, &id).styles.is_empty());

    let h = harness(
        WorkModelProvider::OpenAi,
        Some(script(
            "= Nice\n@css\nbody{background:url(https://evil.example/p.png)}\n@end\n",
        )),
    );
    h.run(ask(PageAssistIntentV1::Style, "make it dark")).await;
    assert!(h.events().iter().any(|e| matches!(e, PageAssistEventV1::Style { style } if style.state == PageStyleStateV1::Rejected)));
    assert!(h.page.lock().applied.is_empty());
}

#[tokio::test]
async fn hosted_cost_confirms_later() {
    let h = harness(WorkModelProvider::Cloud, Some(script("= ok {r2}")));
    let turn = h.run(ask(PageAssistIntentV1::Ask, "q")).await;
    assert_eq!(
        h.last(),
        PageAssistEventV1::Done {
            cost: Some(PageAssistCostV1::Hosted {
                model: "Luna".into(),
                credits: None
            })
        }
    );
    h.service.settle_hosted(TAB, turn, "0.3".into());
    assert_eq!(
        h.last(),
        PageAssistEventV1::Cost {
            cost: PageAssistCostV1::Hosted {
                model: "Luna".into(),
                credits: Some("0.3".into())
            }
        }
    );
}

#[tokio::test]
async fn ai_off_ends_every_thread() {
    let h = harness(WorkModelProvider::OpenAi, Some(script("= ok")));
    h.run(ask(PageAssistIntentV1::Ask, "q")).await;
    h.service.end_all();
    assert!(h.service.status(TAB).await.entries.is_empty());
}

// ---------------------------------------------------------------- loopback

/// One loopback HTTP exchange per entry: the response to write, and the
/// request body read, returned when the server ends.
fn loopback(responses: Vec<String>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let mut bodies = Vec::new();
        for response in responses {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 8192];
            loop {
                let read = socket.read(&mut chunk).unwrap();
                assert!(read > 0);
                request.extend_from_slice(&chunk[..read]);
                let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length: ")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    bodies.push(String::from_utf8_lossy(&request[end + 4..]).into_owned());
                    break;
                }
            }
            socket.write_all(response.as_bytes()).unwrap();
        }
        bodies
    });
    (base, server)
}

fn sse(lines: &[&str]) -> String {
    let mut body = String::new();
    for line in lines {
        let chunk = serde_json::json!({
            "id": "c1", "object": "chat.completion.chunk", "model": "model",
            "choices": [{"index": 0, "delta": {"content": line}, "finish_reason": null}]
        });
        body.push_str(&format!("data: {chunk}\n\n"));
    }
    let end = serde_json::json!({
        "id": "c1", "object": "chat.completion.chunk", "model": "model",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 900, "completion_tokens": 40, "total_tokens": 940}
    });
    body.push_str(&format!("data: {end}\n\ndata: [DONE]\n\n"));
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n{body}"
    )
}

fn loopback_harness(base: &str) -> Harness {
    let h = harness(WorkModelProvider::Compatible, None);
    let client = LeadClient::new(
        LeadTarget::compatible(base).unwrap(),
        zephium_app::work_models::static_credential(
            LeadSecret::new("sk-test-loopback".into()).unwrap(),
        ),
        None,
    );
    *h.models.client.lock().unwrap() = Some(Arc::new(client));
    h
}

#[tokio::test]
async fn loopback_provider_streams_a_cited_answer_without_the_password() {
    let (base, server) = loopback(vec![sse(&[
        "= Pro is $12",
        " a month {r2}\n- Refund",
        "s in 30 days {r3,r404}\n",
    ])]);
    let h = loopback_harness(&base);
    h.run(ask(PageAssistIntentV1::Ask, "How much?")).await;
    let bodies = server.join().unwrap();
    assert!(!bodies[0].contains(PASSWORD));
    assert!(bodies[0].contains("How much?"));
    let parts = h.parts();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].text, "Pro is $12 a month");
    assert_eq!(parts[1].cites.len(), 1, "r404 is dropped");
    assert_eq!(
        h.last(),
        PageAssistEventV1::Done {
            cost: Some(PageAssistCostV1::Local {
                model: "Luna".into()
            })
        }
    );
}

#[tokio::test]
async fn loopback_provider_refusals_become_faults() {
    for (response, expected) in [
        (
            "HTTP/1.1 402 Payment Required\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}",
            WorkModelFaultV1::ProviderCredit,
        ),
        (
            "HTTP/1.1 429 Too Many Requests\r\nretry-after: 3600\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}",
            WorkModelFaultV1::RateLimited,
        ),
    ] {
        let (base, server) = loopback(vec![response.to_owned()]);
        let h = loopback_harness(&base);
        h.run(ask(PageAssistIntentV1::Ask, "q")).await;
        server.join().unwrap();
        let PageAssistEventV1::Failed { fault: PageAssistFaultV1::Model { fault, .. } } = h.last() else {
            panic!("{:?}", h.events());
        };
        assert_eq!(fault, expected);
        assert!(h.parts().is_empty());
    }
}
