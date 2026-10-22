//! One turn, start to finish: read, ask, parse, pace, then steps or a style
//! where the turn asked for them. Every wait races the turn's cancellation.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use zephium_app::work_models::WorkCallSource;
use zephium_core::ids::ProfileId;
use zephium_core::work::model::{
    work_model_cache_key, WorkModelEvent, WorkModelMessage, WorkModelPart, WorkModelReasoning,
    WorkModelRequest, WorkModelSystemBlock, WorkModelToolChoice,
};
use zephium_ipc::page_assist::*;
use zephium_page_assist::actions::{self, Proposal, Verb};
use zephium_page_assist::complexity;
use zephium_page_assist::css;
use zephium_page_assist::guard;
use zephium_page_assist::outline::Outline;
use zephium_page_assist::pacer::Pacer;
use zephium_page_assist::page::PageRead;
use zephium_page_assist::prompt::{self, Message};
use zephium_page_assist::protocol::{Output, Parser};
use zephium_page_assist::text;

use super::port::{ReadFailure, Readout, Role, StepEffect, StepFailure, StepRequest};
use super::{fault, readout, Cancel, CancelReason, PageAssist};

/// Longest a page read may take.
const READ_TIMEOUT: Duration = Duration::from_secs(8);
/// Longest a style turn waits for the page outline before restyling without it.
const OUTLINE_TIMEOUT: Duration = Duration::from_secs(3);
/// Longest one model call may take.
const CALL_TIMEOUT: Duration = Duration::from_secs(90);

pub(super) struct Context {
    pub(super) service: PageAssist,
    pub(super) tab: String,
    pub(super) turn: u32,
    pub(super) profile: ProfileId,
    pub(super) epoch: u64,
    pub(super) site: String,
    pub(super) intent: PageAssistIntentV1,
    pub(super) suggestion: Option<PageAssistSuggestionV1>,
    pub(super) question: String,
    pub(super) selection: Option<String>,
    pub(super) cancel: Arc<Cancel>,
    pub(super) leased: Arc<AtomicBool>,
    pub(super) took_over: Arc<AtomicBool>,
}

/// How a turn ended, before it is told to the panel.
enum End {
    Done(Option<PageAssistCostV1>),
    Failed(PageAssistFaultV1),
    Cancelled(CancelReason),
}

impl Context {
    fn emit(&self, kind: PageAssistEventV1) -> bool {
        let event = {
            let mut state = self.service.state();
            state
                .threads
                .get_mut(&self.tab)
                .and_then(|thread| thread.emit(&self.tab, self.turn, kind))
        };
        let sent = event.is_some();
        self.service.emit(event);
        sent
    }

    fn now(&self) -> u64 {
        self.service.ports().clock.now_ms()
    }

    async fn read(&self) -> Result<(Readout, PageRead), PageAssistFaultV1> {
        let page = &self.service.ports().page;
        let read = tokio::time::timeout(READ_TIMEOUT, page.read(&self.tab, self.epoch));
        let readout = match read.await {
            Ok(Ok(readout)) if readout.epoch == self.epoch => readout,
            Ok(Ok(_)) | Ok(Err(ReadFailure::PageChanged)) => {
                return Err(PageAssistFaultV1::PageChanged)
            }
            Ok(Err(ReadFailure::NotReady)) => return Err(PageAssistFaultV1::Loading),
            Ok(Err(ReadFailure::Unavailable)) | Err(_) => {
                return Err(PageAssistFaultV1::Unreadable)
            }
        };
        let read = readout::page_read(
            &self.site,
            &readout,
            &zephium_agentic::contains_recognized_secret,
        );
        if read.fragments.is_empty() {
            return Err(PageAssistFaultV1::Unreadable);
        }
        self.service
            .state()
            .traits
            .insert(self.tab.clone(), (self.epoch, read.traits));
        Ok((readout, read))
    }

    /// The page's outline for a style turn; a page that cannot give one is
    /// still restyled, generically.
    async fn outline(&self) -> Option<Outline> {
        let page = &self.service.ports().page;
        let outline = tokio::time::timeout(OUTLINE_TIMEOUT, page.outline(&self.tab, self.epoch));
        match outline.await {
            Ok(Ok(outline)) => Some(Outline::admit(outline)),
            Ok(Err(_)) | Err(_) => None,
        }
    }

    fn cite(
        &self,
        read: &PageRead,
        part: zephium_page_assist::protocol::ParsedPart,
    ) -> PageAssistPartV1 {
        let cites = {
            let mut state = self.service.state();
            state
                .threads
                .get_mut(&self.tab)
                .and_then(|thread| thread.record_mut(self.turn))
                .map(|record| record.book.cite(read, &part.fragments))
                .unwrap_or_default()
        };
        PageAssistPartV1 {
            kind: part.kind,
            text: part.text,
            label: part.label,
            cites,
        }
    }

    /// Waits for the person's decision on the waiting confirm or style.
    async fn decision(&self) -> Result<PageAssistDecisionV1, CancelReason> {
        let (sender, receiver) = oneshot::channel();
        self.service.running_turn(&self.tab, self.turn, |running| {
            running.decision = Some(sender)
        });
        tokio::select! {
            reason = self.cancel.cancelled() => Err(reason),
            decision = receiver => decision.map_err(|_| CancelReason::Gone),
        }
    }
}

/// Runs one admitted turn to its end.
pub(super) async fn run(context: Context) {
    let end = tokio::select! {
        biased;
        reason = context.cancel.cancelled() => End::Cancelled(reason),
        end = body(&context) => end,
    };
    context.leased.store(false, Ordering::Release);
    match end {
        End::Done(cost) => {
            context.emit(PageAssistEventV1::Done { cost });
        }
        End::Failed(fault) => {
            context.emit(PageAssistEventV1::Failed { fault });
        }
        End::Cancelled(CancelReason::Stopped) => {
            context.emit(PageAssistEventV1::Cancelled);
        }
        End::Cancelled(CancelReason::Navigated) => {
            context.emit(PageAssistEventV1::Failed {
                fault: PageAssistFaultV1::PageChanged,
            });
        }
        End::Cancelled(CancelReason::Gone) => {}
    }
    let mut state = context.service.state();
    if state
        .running
        .get(&context.tab)
        .is_some_and(|running| running.turn == context.turn)
    {
        state.running.remove(&context.tab);
    }
}

async fn body(context: &Context) -> End {
    if let Some(offer) = complexity::upfront_offer(context.intent, &context.question) {
        context.emit(PageAssistEventV1::Offer { offer });
        return End::Done(None);
    }
    // A full site is told before any request is spent on it.
    if context.intent == PageAssistIntentV1::Style
        && !context
            .service
            .site_has_room(&context.profile, &context.site, 0)
    {
        context.emit(PageAssistEventV1::Style {
            style: PageStyleProposalV1 {
                id: None,
                summary: String::new(),
                site: context.site.clone(),
                rules: 0,
                state: PageStyleStateV1::Full,
            },
        });
        return End::Done(None);
    }
    context.emit(PageAssistEventV1::Status {
        phase: PageAssistPhaseV1::Reading,
    });
    let (readout, read) = match context.read().await {
        Ok(read) => read,
        Err(fault) => return End::Failed(fault),
    };
    context.emit(PageAssistEventV1::Status {
        phase: PageAssistPhaseV1::Read {
            passages: read.passages(),
        },
    });
    let summary = context.intent == PageAssistIntentV1::Summarize
        && (context.question.is_empty()
            || context.suggestion == Some(PageAssistSuggestionV1::Summarize));
    if summary && replay_summary(context, read.fingerprint) {
        return End::Done(None);
    }
    let outline = if context.intent == PageAssistIntentV1::Style {
        context.outline().await
    } else {
        None
    };
    let answer = match answer(context, &read, outline.as_ref()).await {
        Ok(answer) => answer,
        Err(end) => return end,
    };
    if summary && !answer.parts.is_empty() {
        let mut state = context.service.state();
        let book = state
            .threads
            .get(&context.tab)
            .and_then(|thread| thread.record(context.turn))
            .map(|record| record.book.clone())
            .unwrap_or_default();
        state
            .summaries
            .put(read.fingerprint, answer.parts.clone(), book);
    }
    match context.intent {
        PageAssistIntentV1::Act => act(context, &readout, &answer).await,
        PageAssistIntentV1::Style => style(context, &answer, outline.as_ref()).await,
        _ if answer.parts.is_empty() => End::Failed(PageAssistFaultV1::NoAnswer),
        _ => End::Done(Some(answer.cost)),
    }
}

fn replay_summary(context: &Context, fingerprint: u64) -> bool {
    let cached = {
        let mut state = context.service.state();
        let Some((parts, book)) = state
            .summaries
            .get(fingerprint)
            .map(|(parts, book)| (parts.to_vec(), book.clone()))
        else {
            return false;
        };
        if let Some(record) = state
            .threads
            .get_mut(&context.tab)
            .and_then(|thread| thread.record_mut(context.turn))
        {
            record.book = book;
        }
        parts
    };
    for part in cached {
        context.emit(PageAssistEventV1::Part { part });
    }
    true
}

struct Answer {
    parts: Vec<PageAssistPartV1>,
    actions: Vec<String>,
    css: Option<String>,
    cost: PageAssistCostV1,
}

fn request(
    context: &Context,
    read: &PageRead,
    outline: Option<&Outline>,
    entry: &zephium_core::work::model::WorkModelEntry,
) -> WorkModelRequest {
    let history = {
        let state = context.service.state();
        state
            .threads
            .get(&context.tab)
            .map(|thread| thread.history(context.turn))
            .unwrap_or_default()
    };
    let prompt = prompt::build(&prompt::Turn {
        intent: context.intent,
        suggestion: context.suggestion,
        question: &context.question,
        selection: context.selection.as_deref(),
        page: read,
        history: &history,
        outline,
    });
    let system = vec![WorkModelSystemBlock {
        text: prompt.system.to_owned(),
        cache: true,
    }];
    let messages = prompt
        .messages
        .into_iter()
        .map(|message| match message {
            Message::User(parts) => {
                WorkModelMessage::User(parts.into_iter().map(WorkModelPart::Text).collect())
            }
            Message::Assistant(text) => {
                WorkModelMessage::Assistant(vec![WorkModelPart::Text(text)])
            }
        })
        .collect();
    WorkModelRequest {
        image_fence: None,
        cache_key: Some(work_model_cache_key(&system, &[])),
        model: entry.model.clone(),
        system,
        tools: Vec::new(),
        messages,
        max_output_tokens: prompt.max_output_tokens.min(entry.max_output.max(256)),
        reasoning: entry.supports.reasoning.then_some(WorkModelReasoning::Low),
        native_search: false,
        parallel_tools: false,
        tool_choice: WorkModelToolChoice::Auto,
        volatile_tail: 1,
    }
}

async fn answer(
    context: &Context,
    read: &PageRead,
    outline: Option<&Outline>,
) -> Result<Answer, End> {
    let ports = context.service.ports();
    let source = WorkCallSource::Page {
        tab: context.tab.clone(),
        turn: context.turn,
    };
    let model = ports
        .models
        .resolve(&context.profile, source)
        .await
        .map_err(|error| End::Failed(fault::model_fault(error, false)))?;
    let hosted = model.entry.model.provider == zephium_core::work::model::WorkModelProvider::Cloud;
    let request = request(context, read, outline, &model.entry);
    context.emit(PageAssistEventV1::Status {
        phase: if context.intent == PageAssistIntentV1::Style {
            PageAssistPhaseV1::Styling
        } else {
            PageAssistPhaseV1::Answering
        },
    });
    let (sender, mut receiver) = mpsc::unbounded_channel::<String>();
    let events = move |event: WorkModelEvent| {
        if let WorkModelEvent::Text(text) = event {
            let _ = sender.send(text);
        }
    };
    let call = tokio::time::timeout(CALL_TIMEOUT, model.client.call(request, &events));
    tokio::pin!(call);
    let mut parser = Parser::default();
    let mut pacer: Pacer<PageAssistPartV1> = Pacer::default();
    let mut parts = Vec::new();
    let mut actions = Vec::new();
    let deliver = |batch: Vec<PageAssistPartV1>, parts: &mut Vec<PageAssistPartV1>| {
        for part in batch {
            parts.push(part.clone());
            context.emit(PageAssistEventV1::Part { part });
        }
    };
    let take = |outputs: Vec<Output>,
                pacer: &mut Pacer<PageAssistPartV1>,
                parts: &mut Vec<PageAssistPartV1>,
                actions: &mut Vec<String>| {
        for output in outputs {
            match output {
                Output::Part(part) => {
                    let part = context.cite(read, part);
                    deliver(pacer.push(part, context.now()), parts);
                }
                Output::Action(action) => actions.push(action),
            }
        }
    };
    let outcome = loop {
        let wait = pacer
            .deadline()
            .map(|due| Duration::from_millis(due.saturating_sub(context.now())));
        tokio::select! {
            biased;
            Some(text) = receiver.recv() => {
                take(parser.push(&text), &mut pacer, &mut parts, &mut actions);
            }
            _ = tokio::time::sleep(wait.unwrap_or_default()), if wait.is_some() => {
                let due = pacer.poll(context.now());
                deliver(due, &mut parts);
            }
            outcome = &mut call => break outcome,
        }
    };
    while let Ok(text) = receiver.try_recv() {
        take(parser.push(&text), &mut pacer, &mut parts, &mut actions);
    }
    let outcome = match outcome {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(error)) => {
            pacer.clear();
            return Err(End::Failed(fault::model_fault(error, hosted)));
        }
        Err(_) => {
            pacer.clear();
            return Err(End::Failed(PageAssistFaultV1::Model {
                fault: WorkModelFaultV1::ProviderDown,
                retry_after_ms: None,
            }));
        }
    };
    take(parser.finish(), &mut pacer, &mut parts, &mut actions);
    deliver(pacer.flush(), &mut parts);
    Ok(Answer {
        parts,
        actions,
        css: parser.css().map(str::to_owned),
        cost: fault::cost(&model.entry, &outcome.usage),
    })
}

fn category(effect: StepEffect) -> Option<WorkConfirmCategoryV1> {
    match effect {
        StepEffect::Commit(category) => Some(category),
        StepEffect::Read | StepEffect::Draft => None,
    }
}

fn step_event(
    index: usize,
    label: &str,
    effect: StepEffect,
    state: PageAssistStepStateV1,
) -> PageAssistEventV1 {
    PageAssistEventV1::Step {
        step: PageAssistStepV1 {
            index: u8::try_from(index).unwrap_or(u8::MAX),
            label: text::bounded(&text::clean(label), 120),
            category: category(effect),
            state,
        },
    }
}

/// Refuses, from the turn's own read, steps the engine must never run.
fn admissible(readout: &Readout, step: &StepRequest) -> bool {
    let Some(target) = step.target else {
        return step.verb == Verb::Scroll;
    };
    let Some(passage) = readout.passages.iter().find(|passage| passage.id == target) else {
        return false;
    };
    match (step.verb, passage.role) {
        (_, Role::Field(kind)) if kind.protected() => false,
        (Verb::Fill, Role::Field(_)) => true,
        (Verb::Fill, _) => false,
        (Verb::Open, Role::Link { same_site }) => same_site,
        (Verb::Open, _) => false,
        (Verb::Select, Role::Select) => true,
        (Verb::Select, _) => false,
        (Verb::Check | Verb::Uncheck, Role::Checkbox { .. }) => true,
        (Verb::Check | Verb::Uncheck, _) => false,
        (Verb::Click, Role::Link { same_site }) => same_site,
        (Verb::Click | Verb::Expand, Role::Button | Role::Checkbox { .. } | Role::Other) => true,
        (Verb::Click | Verb::Expand, _) => false,
        (Verb::Scroll, _) => true,
    }
}

async fn act(context: &Context, readout: &Readout, answer: &Answer) -> End {
    let proposals: Vec<Proposal> = answer
        .actions
        .iter()
        .filter_map(|line| actions::parse(line))
        .collect();
    let steps = match actions::plan(&proposals) {
        Ok(steps) => steps,
        Err(offer) => {
            context.emit(PageAssistEventV1::Offer { offer });
            return End::Done(Some(answer.cost.clone()));
        }
    };
    if steps.is_empty() {
        return if answer.parts.is_empty() {
            End::Failed(PageAssistFaultV1::NoAnswer)
        } else {
            End::Done(Some(answer.cost.clone()))
        };
    }
    let mut requests: Vec<StepRequest> = steps
        .into_iter()
        .map(|step| StepRequest {
            verb: step.verb,
            read: readout.read,
            target: step.target,
            direction: step.direction,
            value: step.value,
        })
        .collect();
    // A step that leaves the page ends the plan.
    let last = requests
        .iter()
        .position(|step| step.verb == Verb::Open)
        .map_or(requests.len(), |open| open + 1);
    let page = context.service.ports().page.clone();
    let mut readout = readout.clone();
    let mut labels: Vec<Option<String>> = vec![None; last];
    context.leased.store(true, Ordering::Release);
    context.emit(PageAssistEventV1::Status {
        phase: PageAssistPhaseV1::Acting,
    });
    for index in 0..last {
        if context.took_over.swap(false, Ordering::AcqRel) {
            match take_over(context, &mut requests[index..last], &labels[index..]).await {
                Ok(fresh) => readout = fresh,
                Err(end) => return skip_rest(context, &requests, &labels, index, last, end),
            }
        }
        let step = &requests[index];
        let resolved = if admissible(&readout, step) {
            page.resolve(&context.tab, context.epoch, step)
        } else {
            Err(StepFailure::Refused)
        };
        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(failure) => {
                context.emit(step_event(
                    index,
                    "",
                    StepEffect::Read,
                    PageAssistStepStateV1::Failed,
                ));
                return skip_rest(
                    context,
                    &requests,
                    &labels,
                    index + 1,
                    last,
                    step_end(failure),
                );
            }
        };
        labels[index] = Some(resolved.label.clone());
        if let StepEffect::Commit(_) = resolved.effect {
            context.emit(step_event(
                index,
                &resolved.label,
                resolved.effect,
                PageAssistStepStateV1::Confirm,
            ));
            match context.decision().await {
                Ok(PageAssistDecisionV1::Approve) => {}
                Ok(PageAssistDecisionV1::Decline) => {
                    context.emit(step_event(
                        index,
                        &resolved.label,
                        resolved.effect,
                        PageAssistStepStateV1::Declined,
                    ));
                    return skip_rest(
                        context,
                        &requests,
                        &labels,
                        index + 1,
                        last,
                        End::Done(Some(answer.cost.clone())),
                    );
                }
                Err(reason) => return End::Cancelled(reason),
            }
        }
        context.emit(step_event(
            index,
            &resolved.label,
            resolved.effect,
            PageAssistStepStateV1::Running,
        ));
        if step.verb == Verb::Open {
            context
                .service
                .running_turn(&context.tab, context.turn, |running| {
                    running.expects_navigation = true
                });
        }
        match page.act(&context.tab, context.epoch, step).await {
            Ok(()) => {
                context.emit(step_event(
                    index,
                    &resolved.label,
                    resolved.effect,
                    PageAssistStepStateV1::Done,
                ));
            }
            Err(failure) => {
                context.emit(step_event(
                    index,
                    &resolved.label,
                    resolved.effect,
                    PageAssistStepStateV1::Failed,
                ));
                return skip_rest(
                    context,
                    &requests,
                    &labels,
                    index + 1,
                    last,
                    step_end(failure),
                );
            }
        }
    }
    End::Done(Some(answer.cost.clone()))
}

fn step_end(failure: StepFailure) -> End {
    End::Failed(match failure {
        StepFailure::PageChanged => PageAssistFaultV1::PageChanged,
        StepFailure::Unsuitable | StepFailure::Refused | StepFailure::Failed => {
            PageAssistFaultV1::StepFailed
        }
    })
}

fn skip_rest(
    context: &Context,
    requests: &[StepRequest],
    labels: &[Option<String>],
    from: usize,
    last: usize,
    end: End,
) -> End {
    for index in from..last.min(requests.len()) {
        let label = labels.get(index).cloned().flatten().unwrap_or_default();
        context.emit(step_event(
            index,
            &label,
            StepEffect::Read,
            PageAssistStepStateV1::Skipped,
        ));
    }
    end
}

/// The person used the page: wait for Continue, then read it again and go
/// on only if every remaining step still names the same control.
async fn take_over(
    context: &Context,
    remaining: &mut [StepRequest],
    labels: &[Option<String>],
) -> Result<Readout, End> {
    context.emit(PageAssistEventV1::Paused {
        reason: PageAssistPauseV1::TookOver,
    });
    let (sender, receiver) = oneshot::channel();
    context
        .service
        .running_turn(&context.tab, context.turn, |running| {
            running.resume = Some(sender)
        });
    tokio::select! {
        reason = context.cancel.cancelled() => return Err(End::Cancelled(reason)),
        resumed = receiver => resumed.map_err(|_| End::Cancelled(CancelReason::Gone))?,
    }
    context.emit(PageAssistEventV1::Status {
        phase: PageAssistPhaseV1::Reading,
    });
    let (fresh, _) = context.read().await.map_err(End::Failed)?;
    context.emit(PageAssistEventV1::Status {
        phase: PageAssistPhaseV1::Acting,
    });
    // The rest of the plan now belongs to the fresh read, and only to it.
    for step in remaining.iter_mut() {
        step.read = fresh.read;
    }
    for (step, label) in remaining.iter().zip(labels) {
        let Some(label) = label else { continue };
        match context
            .service
            .ports()
            .page
            .resolve(&context.tab, context.epoch, step)
        {
            Ok(resolved) if &resolved.label == label => {}
            _ => return Err(End::Failed(PageAssistFaultV1::StepFailed)),
        }
    }
    Ok(fresh)
}

async fn style(context: &Context, answer: &Answer, outline: Option<&Outline>) -> End {
    let summary = answer
        .parts
        .iter()
        .find(|part| part.kind == PageAssistPartKindV1::Answer)
        .map(|part| text::bounded(&part.text, PAGE_ASSIST_MAX_EXCERPT_CHARS))
        .unwrap_or_else(|| "Page style".to_owned());
    let proposal = |state, rules, id| PageAssistEventV1::Style {
        style: PageStyleProposalV1 {
            id,
            summary: summary.clone(),
            site: context.site.clone(),
            rules,
            state,
        },
    };
    let validated = answer
        .css
        .as_deref()
        .and_then(|sheet| css::validate(sheet).ok())
        .and_then(|validated| guard::guard(&validated, outline));
    let Some(validated) = validated else {
        context.emit(proposal(PageStyleStateV1::Rejected, 0, None));
        return End::Done(Some(answer.cost.clone()));
    };
    let room = |bytes| {
        context
            .service
            .site_has_room(&context.profile, &context.site, bytes)
    };
    if !room(validated.css.len()) {
        context.emit(proposal(PageStyleStateV1::Full, validated.rules, None));
        return End::Done(Some(answer.cost.clone()));
    }
    context.emit(proposal(PageStyleStateV1::Proposed, validated.rules, None));
    match context.decision().await {
        Ok(PageAssistDecisionV1::Approve) => {
            let added =
                context
                    .service
                    .add_style(&context.profile, &context.site, &summary, validated.css);
            let state = if added.is_some() {
                PageStyleStateV1::Applied
            } else {
                PageStyleStateV1::Full
            };
            context.emit(proposal(state, validated.rules, added));
        }
        Ok(PageAssistDecisionV1::Decline) => {
            context.emit(proposal(PageStyleStateV1::Declined, validated.rules, None));
        }
        Err(reason) => return End::Cancelled(reason),
    }
    End::Done(Some(answer.cost.clone()))
}
