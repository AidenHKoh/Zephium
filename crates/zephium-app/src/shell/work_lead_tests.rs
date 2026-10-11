//! A lead run end to end over a scripted model: parts in parallel, found
//! things placed per part, a plan pointing at them, a corrected fault, the
//! reply, and a follow-up that revises instead of adding.
use super::work_planning_tests::{drive, fixture};
use crate::work_agent::{WorkAgentBrowseRequest, WorkBrowserOutcome};
use crate::work_lead::{LeadModel, WorkLeadModels, WorkLeadService};
use crate::work_runtime::WorkArtifactDraft;
use crate::WorkIntent;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use zephium_core::work::{artifact::*, model::*, parts::*, runtime::*, search::*, *};
use zephium_ipc::work::WorkCommandV1;

struct Script {
    calls: Mutex<Vec<String>>,
}

fn call(id: &str, name: &str, arguments: Value) -> WorkModelPart {
    WorkModelPart::ToolCall(WorkModelToolCall {
        id: id.into(),
        name: name.into(),
        arguments,
    })
}
fn results(request: &WorkModelRequest) -> String {
    request
        .messages
        .iter()
        .filter_map(|m| match m {
            WorkModelMessage::ToolResults(results) => Some(
                results
                    .iter()
                    .map(|r| r.content.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn first_key(text: &str) -> String {
    let at = text.find("[s").expect("a source key");
    let end = text[at..].find(']').unwrap();
    text[at + 1..at + end].split(',').next().unwrap().to_owned()
}

impl Script {
    fn lead(&self, request: &WorkModelRequest, turn: usize) -> Vec<WorkModelPart> {
        let seen = results(request);
        let follow_up = request.messages.iter().any(|m| match m {
            WorkModelMessage::User(parts) => parts
                .iter()
                .any(|p| matches!(p, WorkModelPart::Text(t) if t.contains("Earlier requests"))),
            _ => false,
        });
        if follow_up {
            return match turn {
                0 => vec![call("r1", "read_canvas", json!({}))],
                1 => {
                    let plan = seen
                        .lines()
                        .find(|l| l.contains(" plan "))
                        .and_then(|l| l.split_whitespace().nth(1))
                        .unwrap()
                        .to_owned();
                    vec![
                        call(
                            "r2",
                            "revise",
                            json!({"id": plan, "data": {"steps": [
                                {"title": "Fly home early", "kind": "travel", "when": "Fri 10 Jan"}
                            ]}}),
                        ),
                        call(
                            "r3",
                            "create",
                            json!({"kind": "reply", "title": "Shorter trip", "data": {
                            "headline": "Home a day early", "text": "The plan now ends on Friday."}}),
                        ),
                        call(
                            "r5",
                            "create",
                            json!({"kind": "plan", "title": "Your trip, shorter", "data": {"steps": [
                                {"title": "Fly home early", "kind": "travel"}]}}),
                        ),
                    ]
                }
                2 => {
                    assert!(seen.contains("already covers this subject"), "{seen}");
                    vec![call(
                        "r4",
                        "finish",
                        json!({"say": "The plan ends a day earlier."}),
                    )]
                }
                _ => vec![call(
                    "r4",
                    "finish",
                    json!({"say": "The plan ends a day earlier."}),
                )],
            };
        }
        match turn {
            0 => vec![
                WorkModelPart::Text("Starting the stay and entry parts.".into()),
                call("a", "load_skill", json!({"name": "trip-planning"})),
                call(
                    "b",
                    "start_part",
                    json!({"title": "Stay", "helper": "browser",
                    "goal": "Two homes near the YC office for 6–12 January",
                    "brief": "2 guests, under $300 a night", "service": "airbnb.com"}),
                ),
                call(
                    "c",
                    "start_part",
                    json!({"title": "Entry", "helper": "research",
                    "goal": "What a Polish citizen needs to enter the US"}),
                ),
            ],
            1 => {
                assert!(seen.contains("Part Stay"), "{seen}");
                assert!(seen.contains("Part Entry"), "{seen}");
                let picks = seen
                    .lines()
                    .find(|l| l.contains(" picks \""))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .expect("the Stay part's picks")
                    .to_owned();
                vec![
                    call(
                        "d",
                        "create",
                        json!({"kind": "plan", "title": "Your trip", "data": {
                        "steps": [
                            {"title": "Stay in the Mission", "kind": "stay", "when": "6–12 Jan",
                             "pick": {"artifact": picks, "index": 0}},
                            {"title": "Apply for ESTA", "kind": "task", "when": "Before 3 Jan"}
                        ],
                        "total": {"label": "Estimated total", "value": "$1,740"}}}),
                    ),
                    call(
                        "e",
                        "create",
                        json!({"kind": "reply", "title": "Your trip", "data": {
                        "headline": "x".repeat(90), "text": "Six nights in the Mission."}}),
                    ),
                ]
            }
            2 => {
                assert!(
                    seen.contains("headline is one line of 1 to 80 characters"),
                    "{seen}"
                );
                vec![call(
                    "f",
                    "create",
                    json!({"kind": "reply", "title": "Your trip", "data": {
                    "headline": "Six nights in the Mission, $1,740", "text": "Apply for **ESTA** first."}}),
                )]
            }
            _ => vec![call(
                "g",
                "finish",
                json!({"say": "Your trip is on the canvas.",
                "followups": ["Book the Mission loft"], "title": "YC trip from Warsaw."}),
            )],
        }
    }
    fn browser(&self, request: &WorkModelRequest, turn: usize) -> Vec<WorkModelPart> {
        match turn {
            0 => vec![call(
                "h1",
                "browse",
                json!({"start": "airbnb.com",
                "goal": "Homes near the YC office for 6–12 January, 2 guests",
                "records": {"title": "Homes", "max_items": 2, "columns": [
                    {"name": "price", "value": {"kind": "text"}, "required": false},
                    {"name": "photo", "value": {"kind": "image_url"}, "required": false}]}}),
            )],
            1 => {
                let seen = results(request);
                let key = first_key(&seen);
                vec![call(
                    "h2",
                    "create",
                    json!({"kind": "picks", "title": "Homes near YC", "data": {
                    "facet": "stay", "items": [
                        {"name": "Mission loft", "price": {"display": "$290 / night", "amount": 290, "currency": "USD"},
                         "image_candidates": ["https://a0.muscache.com/im/pictures/loft.jpg"],
                         "recommended": true, "source": key},
                        {"name": "SoMa studio", "price": {"display": "$240 / night"}, "source": key}]},
                    "sources": [key]}),
                )]
            }
            _ => vec![call(
                "h3",
                "finish",
                json!({"summary": "2 homes", "digest": "Mission loft $290/night; SoMa studio $240/night."}),
            )],
        }
    }
    fn research(&self, request: &WorkModelRequest, turn: usize) -> Vec<WorkModelPart> {
        match turn {
            0 => vec![call(
                "e1",
                "web_search",
                json!({"query": "US entry requirements Polish citizens ESTA"}),
            )],
            1 => {
                let key = first_key(&results(request));
                vec![call(
                    "e2",
                    "create",
                    json!({"kind": "list", "title": "Entry needs", "data": {
                    "style": "requirements", "items": [{"title": "ESTA approved before you fly", "source": key}]},
                    "sources": [key]}),
                )]
            }
            _ => {
                let seen = results(request);
                assert!(seen.contains("finish's digest"), "{seen}");
                let key = first_key(&seen);
                vec![call(
                    "e3",
                    "finish",
                    json!({"summary": "Entry needs", "digest": format!("ESTA required [{key}]")}),
                )]
            }
        }
    }
}

impl WorkModelClient for Script {
    fn call<'a>(
        &'a self,
        request: WorkModelRequest,
        _: &'a (dyn Fn(WorkModelEvent) + Send + Sync),
    ) -> WorkModelFuture<'a> {
        Box::pin(async move {
            let turn = request
                .messages
                .iter()
                .filter(|m| matches!(m, WorkModelMessage::Assistant(_)))
                .count();
            let system = request
                .system
                .iter()
                .map(|b| b.text.as_str())
                .collect::<String>();
            let (who, assistant) = if system.contains("You are the Work agent") {
                ("lead", self.lead(&request, turn))
            } else if system.contains("You work on web pages") {
                ("browser", self.browser(&request, turn))
            } else {
                ("research", self.research(&request, turn))
            };
            self.calls.lock().unwrap().push(format!("{who}:{turn}"));
            Ok(WorkModelOutcome {
                stop: WorkModelStop::ToolUse,
                usage: WorkModelUsage {
                    input_tokens: 2_000,
                    cached_input_tokens: 1_000,
                    cache_write_input_tokens: 0,
                    output_tokens: 200,
                    reasoning_tokens: 0,
                    cost_micros: None,
                },
                assistant,
            })
        })
    }
}

/// The chosen model's own search, as most runs now use it.
const SEARCH_MODEL: &str = "anthropic/claude-haiku-4-5-20251001";
struct Search;

struct BurstSearches(Script);
impl WorkModelClient for BurstSearches {
    fn call<'a>(
        &'a self,
        request: WorkModelRequest,
        event: &'a (dyn Fn(WorkModelEvent) + Send + Sync),
    ) -> WorkModelFuture<'a> {
        Box::pin(async move {
            let system: String = request
                .system
                .iter()
                .map(|block| block.text.as_str())
                .collect();
            let first = !request
                .messages
                .iter()
                .any(|message| matches!(message, WorkModelMessage::Assistant(_)));
            let research = !system.contains("You are the Work agent")
                && !system.contains("You work on web pages");
            let mut outcome = self.0.call(request, event).await?;
            if first && research {
                outcome.assistant = [
                    "ESTA eligibility nationality",
                    "ESTA processing times",
                    "ESTA application fees",
                ]
                .into_iter()
                .enumerate()
                .map(|(index, query)| {
                    call(
                        &format!("burst-{index}"),
                        "web_search",
                        json!({"query":query}),
                    )
                })
                .collect();
            }
            Ok(outcome)
        })
    }
}

struct SlowSearch(std::sync::atomic::AtomicUsize);

#[tokio::test]
async fn a_finished_reply_keeps_an_unsettled_approved_write_unknown() {
    let store = Arc::new(zephium_store::SqliteStore::in_memory().unwrap());
    let (mut shell, queue, handle, profile) = fixture(store);
    let create = handle
        .work_document(WorkIntent::Create {
            objective: "Plan my YC batch trip from Warsaw".into(),
        })
        .unwrap();
    let work = create.work_id().unwrap();
    drive(&mut shell, &queue, create).await.unwrap();
    let client = Arc::new(Script {
        calls: Mutex::new(Vec::new()),
    });
    let models = WorkLeadModels {
        lead: model(client.clone()),
        page: model(client.clone()),
        light: model(client),
    };
    let injected = std::sync::atomic::AtomicBool::new(false);
    let service = WorkLeadService::new(handle);
    let result = drive(
        &mut shell,
        &queue,
        service.run(
            profile,
            command(work, WorkRevision::INITIAL),
            None,
            models,
            &Search,
            |probe, request| {
                let first = !injected.swap(true, std::sync::atomic::Ordering::SeqCst);
                async move {
                    if first {
                        // A native write was approved but its settlement was lost.
                        // Only its durable fact is injected; no file is touched.
                        probe
                            .commit_step(WorkRuntimeUpdate::BeginStep {
                                execution: probe.execution(),
                                attempt: probe.attempt(),
                                step: WorkStepFact {
                                    model_fault: None,
                                    id: WorkStepId::generate(),
                                    turn: 1,
                                    kind: WorkStepKindV1::WriteFile {
                                        path: "/Users/person/notes.txt".into(),
                                        content: "Approved draft".into(),
                                        decision: Some(true),
                                    },
                                    status: WorkStepStatus::Running,
                                    usage: None,
                                    artifacts: vec![],
                                    evidence: None,
                                    note: None,
                                    measurements: None,
                                    local: None,
                                    account: None,
                                    part: None,
                                },
                                artifacts: vec![],
                                evidence: None,
                                file: None,
                            })
                            .await?;
                    }
                    page(request).await
                }
            },
            |_| {},
        ),
    )
    .await
    .unwrap();
    let execution = &result.executions[0];
    assert!(execution
        .artifacts
        .iter()
        .any(|artifact| matches!(artifact.data, WorkArtifactDataV1::Reply { .. })));
    assert!(execution
        .steps
        .iter()
        .any(|step| matches!(step.kind, WorkStepKindV1::Finish { .. })));
    assert_eq!(execution.status, WorkExecutionStatus::Interrupted);
    assert!(execution.attempts[0].usage.is_none());
    assert!(execution
        .steps
        .iter()
        .filter(|step| matches!(step.kind, WorkStepKindV1::WriteFile { .. }))
        .all(|step| step.status == WorkStepStatus::OutcomeUnknown));
}
impl WorkPublicSearchProvider for SlowSearch {
    fn grant(&self) -> Option<(WorkSearchProvider, String)> {
        Search.grant()
    }
    fn search<'a>(
        &'a self,
        scope: &'a WorkPublicSearchScope,
        context: &'a [zephium_core::work::context::WorkContextBody],
        limits: WorkExecutionLimits,
    ) -> WorkPublicSearchFuture<'a> {
        Box::pin(async move {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Search.search(scope, context, limits).await
        })
    }
}

#[tokio::test]
async fn concurrent_parts_drain_partial_search_batches_before_retrying_worker_capacity() {
    let store = Arc::new(zephium_store::SqliteStore::in_memory().unwrap());
    let (mut shell, queue, handle, profile) = fixture(store);
    let create = handle
        .work_document(WorkIntent::Create {
            objective: "Plan my YC batch trip from Warsaw".into(),
        })
        .unwrap();
    let work = create.work_id().unwrap();
    drive(&mut shell, &queue, create).await.unwrap();
    let client = Arc::new(BurstSearches(Script {
        calls: Mutex::new(Vec::new()),
    }));
    let models = WorkLeadModels {
        lead: model(client.clone()),
        page: model(client.clone()),
        light: model(client),
    };
    let search = SlowSearch(std::sync::atomic::AtomicUsize::new(0));
    let mut request = command(work, WorkRevision::INITIAL);
    let WorkRuntimeIntent::BeginAgent { limits, .. } = &mut request.intent else {
        panic!()
    };
    limits.max_workers = 2;
    let service = WorkLeadService::new(handle);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        drive(
            &mut shell,
            &queue,
            service.run(
                profile,
                request,
                None,
                models,
                &search,
                |_, request| async {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    page(request).await
                },
                |_| {},
            ),
        ),
    )
    .await
    .expect("no admission deadlock")
    .unwrap();
    let execution = &result.executions[0];
    assert!(
        matches!(
            execution.status,
            WorkExecutionStatus::Completed | WorkExecutionStatus::NeedsReview
        ),
        "{:?}",
        execution.status
    );
    assert_eq!(search.0.load(std::sync::atomic::Ordering::SeqCst), 3);
    let searches: Vec<_> = execution
        .steps
        .iter()
        .filter(|step| matches!(step.kind, WorkStepKindV1::Search { .. }))
        .collect();
    assert_eq!(searches.len(), 3);
    assert!(searches
        .iter()
        .all(|step| step.status == WorkStepStatus::Succeeded));
    assert!(execution
        .steps
        .iter()
        .all(|step| step.status != WorkStepStatus::Running));
}
impl WorkPublicSearchProvider for Search {
    fn grant(&self) -> Option<(WorkSearchProvider, String)> {
        Some((WorkSearchProvider::Native, SEARCH_MODEL.into()))
    }

    fn search<'a>(
        &'a self,
        scope: &'a WorkPublicSearchScope,
        _: &'a [zephium_core::work::context::WorkContextBody],
        _: WorkExecutionLimits,
    ) -> WorkPublicSearchFuture<'a> {
        Box::pin(async move {
            assert!(scope.query.contains("ESTA"));
            assert_eq!(scope.provider, WorkSearchProvider::Native);
            assert_eq!(scope.model, SEARCH_MODEL);
            Ok(WorkPublicSearchResult {
                evidence: WorkProviderSearchEvidenceV1 {
                    version: 1,
                    provider: WorkSearchProvider::Native,
                    model: SEARCH_MODEL.into(),
                    response_model: SEARCH_MODEL.into(),
                    response_id: "zr_entry".into(),
                    search_call_id: "zs_entry".into(),
                    answer: "Polish citizens travel on ESTA [1].".into(),
                    citations: vec![WorkProviderSearchCitation {
                        url: "https://esta.cbp.dhs.gov/".into(),
                        title: "Official ESTA Application".into(),
                        start_index: 0,
                        end_index: 10,
                        snippet: None,
                    }],
                    actual_input_tokens: 200,
                    actual_output_tokens: 100,
                },
                usage: WorkUsage {
                    model_tokens: 300,
                    cost_micro_usd: 1_000,
                    operations: 1,
                    accounting: WorkUsageAccounting::Exact,
                },
            })
        })
    }
}

pub(super) fn model(client: Arc<dyn WorkModelClient>) -> LeadModel {
    LeadModel {
        entry: WorkModelEntry {
            id: "openai/gpt-6".into(),
            model: WorkModelRef {
                provider: WorkModelProvider::OpenAi,
                wire: WorkModelWire::OpenAiResponses,
                model: "gpt-6".into(),
            },
            display_name: "GPT-6".into(),
            roles: vec![WorkModelRole::Lead],
            recommended: true,
            context_window: 400_000,
            max_output: 32_000,
            supports: WorkModelSupports {
                tools: true,
                vision: true,
                prompt_cache: true,
                reasoning: true,
                native_search: true,
            },
            price: Some(WorkModelPrice {
                input: 2_000_000,
                cached_input: 200_000,
                output: 8_000_000,
            }),
        },
        client,
    }
}

pub(super) fn command(work: WorkId, expected: WorkRevision) -> WorkCommandV1 {
    WorkCommandV1 {
        version: 1,
        work,
        expected_revision: expected,
        command: WorkCommandId::generate(),
        intent: WorkRuntimeIntent::BeginAgent {
            grant: WorkAgentGrantV1 {
                provider: WorkSearchProvider::OpenAi,
                model: PUBLIC_SEARCH_MODEL.into(),
                max_turns: 10,
                max_steps: 32,
                browse_hops: 4,
                folders: vec![],
                accounts: vec![],
                private: false,
                lead: None,
                skill: None,
                search_fallback: None,
            },
            limits: WorkExecutionLimits {
                model_tokens: 1_000_000,
                cost_micro_usd: 3_000_000,
                operations: 256,
                timeout_seconds: 120,
                max_workers: 4,
            },
        },
    }
}

async fn page(request: WorkAgentBrowseRequest) -> Result<WorkBrowserOutcome, WorkError> {
    let WorkStepKindV1::Read { goal, .. } = &request.step else {
        panic!("a page step");
    };
    assert!(goal.as_deref().is_some_and(|g| g.contains("YC office")));
    let subject = |name: &str, image: &str| WorkSubject {
        name: name.into(),
        descriptor: Some("Entire home".into()),
        homepage: Some(format!("https://www.airbnb.com/rooms/{}", name.len())),
        record_media: None,
        media_attempted: false,
        image_candidates: vec![image.into()],
    };
    Ok(WorkBrowserOutcome {
        status: WorkStepStatus::Succeeded,
        usage: Some(WorkUsage {
            model_tokens: 5_000,
            cost_micro_usd: 20_000,
            operations: 4,
            accounting: WorkUsageAccounting::Exact,
        }),
        artifacts: vec![WorkArtifactDraft {
            output: request.output,
            title: "Homes".into(),
            data: WorkArtifactDataV1::Findings {
                subjects: vec![
                    subject(
                        "Mission loft",
                        "https://a0.muscache.com/im/pictures/loft.jpg",
                    ),
                    subject(
                        "SoMa studio",
                        "https://a0.muscache.com/im/pictures/studio.jpg",
                    ),
                ],
                items: vec![WorkFinding {
                    claim: "Mission loft is $290 a night".into(),
                    subject: Some(0),
                    evidence: vec![0],
                    confidence: WorkConfidence::Supported,
                    detail: None,
                    general_knowledge: false,
                }],
            },
            evidence: vec![WorkEvidenceLink {
                extraction_id: WorkArtifactId::from(900),
                source_id: 1,
            }],
        }],
        intervention: None,
        note: None,
        measurements: None,
        helped: false,
        held_back: false,
        rerun: false,
        search: None,
        redirect: None,
    })
}

#[tokio::test]
async fn a_lead_run_splits_into_parts_builds_the_result_and_revises_on_follow_up() {
    let store = Arc::new(zephium_store::SqliteStore::in_memory().unwrap());
    let (mut shell, queue, handle, profile) = fixture(store);
    let create = handle
        .work_document(WorkIntent::Create {
            objective: "Plan my YC batch trip from Warsaw".into(),
        })
        .unwrap();
    let work = create.work_id().unwrap();
    drive(&mut shell, &queue, create).await.unwrap();
    let script = Arc::new(Script {
        calls: Mutex::new(Vec::new()),
    });
    let models = WorkLeadModels {
        lead: model(script.clone()),
        page: model(script.clone()),
        light: model(script.clone()),
    };
    let service = WorkLeadService::new(handle.clone());
    let first = drive(
        &mut shell,
        &queue,
        service.run(
            profile,
            command(work, WorkRevision::INITIAL),
            None,
            models.clone(),
            &Search,
            |_, request| page(request),
            |_| {},
        ),
    )
    .await
    .unwrap();
    let run = &first.executions[0];
    assert!(
        matches!(
            run.status,
            WorkExecutionStatus::NeedsReview | WorkExecutionStatus::Completed
        ),
        "{:?} {:?}",
        run.status,
        run.steps
            .iter()
            .map(|s| (s.status, s.note.clone()))
            .collect::<Vec<_>>()
    );
    assert!(run.agent_grant().unwrap().lead.is_some());
    let titles: Vec<(&str, WorkPartStateV1, Option<&str>)> = run
        .parts
        .iter()
        .map(|p| (p.title.as_str(), p.state, p.summary.as_deref()))
        .collect();
    assert!(
        titles.contains(&("Stay", WorkPartStateV1::Done, Some("2 homes"))),
        "{titles:?}"
    );
    assert!(
        titles.contains(&("Entry", WorkPartStateV1::Done, Some("Entry needs"))),
        "{titles:?}"
    );
    assert_eq!(run.inputs[0].kind, WorkInputKindV1::Skill);
    assert_eq!(run.inputs[0].label, "Trip planning");
    let stay = run.parts.iter().find(|p| p.title == "Stay").unwrap().id;
    let page_step = run
        .steps
        .iter()
        .find(|s| matches!(&s.kind, WorkStepKindV1::Read { goal: Some(_), .. }))
        .unwrap();
    assert_eq!(page_step.part, Some(stay));
    let object = |kind: &str| {
        run.artifacts
            .iter()
            .find(|a| a.data.kind_name() == kind)
            .unwrap_or_else(|| panic!("a {kind}"))
    };
    let picks = object("picks");
    assert_eq!(picks.part, Some(stay));
    assert!(!picks.evidence.is_empty());
    let WorkArtifactDataV1::Plan { steps, .. } = &object("plan").data else {
        panic!()
    };
    assert_eq!(steps[0].pick.as_ref().unwrap().artifact, picks.id);
    assert_eq!(object("plan").part, None);
    assert_eq!(
        run.artifacts
            .iter()
            .filter(|a| a.data.kind_name() == "reply")
            .count(),
        1
    );
    // A research part hands its facts to the lead; it places no list.
    let entry = run.parts.iter().find(|p| p.title == "Entry").unwrap().id;
    assert!(!run.artifacts.iter().any(|a| a.part == Some(entry)));
    let finish = run.steps.last().unwrap();
    assert!(
        matches!(&finish.kind, WorkStepKindV1::Finish { followups, .. } if followups.len() == 1)
    );
    assert_eq!(finish.note.as_deref(), Some("Your trip is on the canvas."));
    assert_eq!(run.title.as_deref(), Some("YC trip from Warsaw"));
    let calls = script.calls.lock().unwrap().clone();
    assert!(
        calls.contains(&"browser:2".into()) && calls.contains(&"research:2".into()),
        "{calls:?}"
    );

    let second = drive(
        &mut shell,
        &queue,
        service.run(
            profile,
            command(work, first.work.revision),
            None,
            models,
            &Search,
            |_, request| page(request),
            |_| {},
        ),
    )
    .await
    .unwrap();
    let follow_up = second.executions.last().unwrap();
    assert!(follow_up.parts.is_empty());
    let revised = follow_up
        .artifacts
        .iter()
        .find(|a| a.data.kind_name() == "plan")
        .unwrap();
    assert_eq!(revised.revises, Some(object("plan").id));
    assert_eq!(revised.title, "Your trip");
    assert_eq!(
        follow_up
            .artifacts
            .iter()
            .filter(|a| a.data.kind_name() == "plan")
            .count(),
        1
    );
}

/// A part whose site fails says what it needs; the lead may not stand a
/// figure or a to-do in for what was not found.
struct Honest;
impl WorkModelClient for Honest {
    fn call<'a>(
        &'a self,
        request: WorkModelRequest,
        _: &'a (dyn Fn(WorkModelEvent) + Send + Sync),
    ) -> WorkModelFuture<'a> {
        Box::pin(async move {
            let turn = request
                .messages
                .iter()
                .filter(|m| matches!(m, WorkModelMessage::Assistant(_)))
                .count();
            let lead = request
                .system
                .iter()
                .any(|b| b.text.contains("You are the Work agent"));
            let seen = results(&request);
            let assistant = match (lead, turn) {
                (true, 0) => vec![call(
                    "p",
                    "start_part",
                    json!({"title": "Flights", "helper": "browser", "service": "kayak.com",
                    "goal": "Round trips WAW to SFO for 5 January 2027"}),
                )],
                (true, 1) => {
                    assert!(seen.contains("could not do its job"), "{seen}");
                    assert!(seen.contains("another try on kayak.com"), "{seen}");
                    vec![
                        call(
                            "r",
                            "create",
                            json!({"kind": "reply", "title": "Flights", "data": {
                            "headline": "Airfare recheck complete", "text": "No fare was found.",
                            "figures": [{"label": "Exact-date fare", "value": "Not verified"}]}}),
                        ),
                        call(
                            "l",
                            "create",
                            json!({"kind": "list", "title": "To do", "data": {"style": "todo",
                            "items": [{"title": "Retry the Kayak flights check"}]}}),
                        ),
                    ]
                }
                (true, 2) => {
                    assert!(seen.contains("figure 1 stands for something"), "{seen}");
                    assert!(seen.contains("list: item 1 is about work"), "{seen}");
                    vec![call(
                        "r2",
                        "create",
                        json!({"kind": "reply", "title": "Flights", "data": {
                        "headline": "No flights found yet",
                        "text": "Kayak did not load; try again from the Flights row."}}),
                    )]
                }
                (true, _) => vec![call("f", "finish", json!({"say": "No flights yet."}))],
                (false, _) => vec![call(
                    "h",
                    "finish",
                    json!({"summary": "Kayak did not load", "digest": "The results page failed.",
                    "found": false, "need": {"kind": "retry", "target": "https://www.kayak.com/flights"}}),
                )],
            };
            Ok(WorkModelOutcome {
                stop: WorkModelStop::ToolUse,
                usage: WorkModelUsage {
                    input_tokens: 1_000,
                    cached_input_tokens: 0,
                    cache_write_input_tokens: 0,
                    output_tokens: 100,
                    reasoning_tokens: 0,
                    cost_micros: None,
                },
                assistant,
            })
        })
    }
}

#[tokio::test]
async fn a_part_that_cannot_do_its_job_carries_its_fix_and_nothing_stands_for_it() {
    let store = Arc::new(zephium_store::SqliteStore::in_memory().unwrap());
    let (mut shell, queue, handle, profile) = fixture(store);
    let create = handle
        .work_document(WorkIntent::Create {
            objective: "Recheck flights WAW to SFO".into(),
        })
        .unwrap();
    let work = create.work_id().unwrap();
    drive(&mut shell, &queue, create).await.unwrap();
    let client = Arc::new(Honest);
    let lead = LeadModel {
        client,
        ..model(Arc::new(Script {
            calls: Mutex::new(Vec::new()),
        }))
    };
    let models = WorkLeadModels {
        lead: lead.clone(),
        page: lead.clone(),
        light: lead,
    };
    let done = drive(
        &mut shell,
        &queue,
        WorkLeadService::new(handle.clone()).run(
            profile,
            command(work, WorkRevision::INITIAL),
            None,
            models,
            &Search,
            |_, request| page(request),
            |_| {},
        ),
    )
    .await
    .unwrap();
    let run = &done.executions[0];
    let flights = &run.parts[0];
    assert_eq!(flights.state, WorkPartStateV1::Failed);
    assert_eq!(
        flights.need,
        Some(WorkPartNeedV1::Retry {
            host: Some("kayak.com".into()),
            reason: None,
        })
    );
    let kinds: Vec<&str> = run.artifacts.iter().map(|a| a.data.kind_name()).collect();
    assert_eq!(kinds, ["reply"]);
}

/// A Slack part when the person added a Slack MCP server: the run offers it
/// once, the part runs as a connection and calls the server's tools.
struct Connected;
impl WorkModelClient for Connected {
    fn call<'a>(
        &'a self,
        request: WorkModelRequest,
        _: &'a (dyn Fn(WorkModelEvent) + Send + Sync),
    ) -> WorkModelFuture<'a> {
        Box::pin(async move {
            let turn = request
                .messages
                .iter()
                .filter(|m| matches!(m, WorkModelMessage::Assistant(_)))
                .count();
            let system = request
                .system
                .iter()
                .map(|b| b.text.as_str())
                .collect::<String>();
            let seen = results(&request);
            let assistant = if system.contains("You are the Work agent") {
                match turn {
                    0 => vec![call(
                        "p",
                        "start_part",
                        json!({"title": "Slack", "helper": "browser", "service": "app.slack.com",
                        "goal": "Unread mentions since yesterday"}),
                    )],
                    1 => {
                        assert!(
                            seen.contains("Part Slack") && seen.contains("done"),
                            "{seen}"
                        );
                        vec![call(
                            "r",
                            "create",
                            json!({"kind": "reply", "title": "Today", "data": {
                            "headline": "One mention today", "text": "Ana asked about the deck."}}),
                        )]
                    }
                    _ => vec![call("f", "finish", json!({"say": "Slack is read."}))],
                }
            } else {
                assert!(system.contains("Connection helper"), "{system}");
                match turn {
                    0 => {
                        assert!(
                            request.tools.iter().any(|t| t.name == "slack__echo"),
                            "the server's tools are offered"
                        );
                        vec![call("e", "slack__echo", json!({"text": "mentions"}))]
                    }
                    _ => vec![call(
                        "h",
                        "finish",
                        json!({"summary": "1 mention", "digest": "Ana: the deck."}),
                    )],
                }
            };
            Ok(WorkModelOutcome {
                stop: WorkModelStop::ToolUse,
                usage: WorkModelUsage {
                    input_tokens: 1_000,
                    cached_input_tokens: 0,
                    cache_write_input_tokens: 0,
                    output_tokens: 100,
                    reasoning_tokens: 0,
                    cost_micros: None,
                },
                assistant,
            })
        })
    }
}

#[tokio::test]
#[ignore = "needs the zephium-mcp-fixture binary in ZEPHIUM_MCP_FIXTURE"]
async fn a_service_the_person_connected_is_offered_once_before_its_website() {
    use zephium_ipc::work::{WorkServerTransportV1, WorkServerV1};
    let program = std::env::var("ZEPHIUM_MCP_FIXTURE").expect("set ZEPHIUM_MCP_FIXTURE");
    let store = Arc::new(zephium_store::SqliteStore::in_memory().unwrap());
    let (mut shell, queue, handle, profile) = fixture(store);
    let data = std::env::temp_dir().join(format!("zephium-connected-{}", std::process::id()));
    crate::work_connections::store::install(&data);
    let server = WorkServerV1 {
        id: "slack".into(),
        name: "Slack".into(),
        transport: WorkServerTransportV1::Stdio {
            command: program,
            args: vec![],
            env: vec![],
        },
        enabled: true,
    };
    let connections = crate::work_connections::store::shared().unwrap();
    connections
        .put(&profile.to_string(), server.clone(), None)
        .unwrap();
    crate::work_connections::mcp::check(&profile.to_string(), &server).await;
    let create = handle
        .work_document(WorkIntent::Create {
            objective: "What do I need to do today in Slack?".into(),
        })
        .unwrap();
    let work = create.work_id().unwrap();
    drive(&mut shell, &queue, create).await.unwrap();
    let lead = LeadModel {
        client: Arc::new(Connected),
        ..model(Arc::new(Script {
            calls: Mutex::new(Vec::new()),
        }))
    };
    let models = WorkLeadModels {
        lead: lead.clone(),
        page: lead.clone(),
        light: lead,
    };
    let answering = {
        let handle = handle.clone();
        tokio::spawn(async move {
            let mut answered = 0;
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                let Ok(request) = handle.work_projection(profile, work) else {
                    continue;
                };
                let zephium_ipc::work::WorkReplyV1::Projection { projection } =
                    request.response(profile).await.reply
                else {
                    continue;
                };
                let Some(execution) = projection.executions.last() else {
                    continue;
                };
                let open = execution.steps.iter().find_map(|step| match &step.kind {
                    WorkStepKindV1::Ask {
                        options,
                        answer: None,
                        purpose,
                        ..
                    } if step.status == WorkStepStatus::Running => {
                        Some((step.id, options[0].clone(), *purpose))
                    }
                    _ => None,
                });
                let Some((step, first, purpose)) = open else {
                    continue;
                };
                assert_eq!(purpose, Some(WorkAskPurposeV1::Connection));
                answered += 1;
                let _ = handle
                    .work_command(
                        profile,
                        WorkCommandV1 {
                            version: 1,
                            work,
                            expected_revision: projection.work.revision,
                            command: WorkCommandId::generate(),
                            intent: WorkRuntimeIntent::AnswerStep {
                                execution: execution.id,
                                step,
                                answer: first,
                            },
                        },
                    )
                    .unwrap()
                    .await;
                if answered > 3 {
                    return answered;
                }
            }
        })
    };
    let done = drive(
        &mut shell,
        &queue,
        WorkLeadService::new(handle.clone()).run(
            profile,
            command(work, WorkRevision::INITIAL),
            None,
            models,
            &Search,
            |_, _| async { panic!("the Slack part never opens the website") },
            |_| {},
        ),
    )
    .await
    .unwrap();
    answering.abort();
    let run = &done.executions[0];
    let asks: Vec<&str> = run
        .steps
        .iter()
        .filter_map(|s| match &s.kind {
            WorkStepKindV1::Ask { prompt, .. } => Some(prompt.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        asks,
        ["Use Slack (MCP)? Its tools read Slack as you in this work."]
    );
    let slack = &run.parts[0];
    assert_eq!(slack.helper, WorkHelperV1::Connection);
    assert_eq!(slack.state, WorkPartStateV1::Done);
    assert!(run
        .steps
        .iter()
        .any(|s| matches!(&s.kind, WorkStepKindV1::Call { call } if call.service == "slack")));
    let _ = std::fs::remove_dir_all(&data);
}

/// A lead scripted turn by turn: the turn's index and every tool result so
/// far in, the calls out.
struct Turns(fn(usize, &str) -> Vec<WorkModelPart>);
impl WorkModelClient for Turns {
    fn call<'a>(
        &'a self,
        request: WorkModelRequest,
        _: &'a (dyn Fn(WorkModelEvent) + Send + Sync),
    ) -> WorkModelFuture<'a> {
        Box::pin(async move {
            let turn = request
                .messages
                .iter()
                .filter(|m| matches!(m, WorkModelMessage::Assistant(_)))
                .count();
            Ok(WorkModelOutcome {
                stop: WorkModelStop::ToolUse,
                usage: WorkModelUsage {
                    input_tokens: 1_000,
                    cached_input_tokens: 0,
                    cache_write_input_tokens: 0,
                    output_tokens: 100,
                    reasoning_tokens: 0,
                    cost_micros: None,
                },
                assistant: (self.0)(turn, &results(&request)),
            })
        })
    }
}

async fn scripted(
    objective: &str,
    turns: fn(usize, &str) -> Vec<WorkModelPart>,
) -> WorkExecutionFact {
    let store = Arc::new(zephium_store::SqliteStore::in_memory().unwrap());
    let (mut shell, queue, handle, profile) = fixture(store);
    let create = handle
        .work_document(WorkIntent::Create {
            objective: objective.into(),
        })
        .unwrap();
    let work = create.work_id().unwrap();
    drive(&mut shell, &queue, create).await.unwrap();
    let lead = LeadModel {
        client: Arc::new(Turns(turns)),
        ..model(Arc::new(Script {
            calls: Mutex::new(Vec::new()),
        }))
    };
    let models = WorkLeadModels {
        lead: lead.clone(),
        page: lead.clone(),
        light: lead,
    };
    let done = drive(
        &mut shell,
        &queue,
        WorkLeadService::new(handle.clone()).run(
            profile,
            command(work, WorkRevision::INITIAL),
            None,
            models,
            &Search,
            |_, request| page(request),
            |_| {},
        ),
    )
    .await
    .unwrap();
    done.executions[0].clone()
}

#[tokio::test]
async fn a_refused_object_stands_on_its_second_try_without_its_optional_excess() {
    fn turns(turn: usize, seen: &str) -> Vec<WorkModelPart> {
        let plan = || {
            call(
                "p",
                "create",
                json!({"kind": "plan", "title": "Your trip", "data": {"steps": [
                    {"title": "Fly to SFO", "kind": "travel", "detail": "x".repeat(300)},
                    {"title": "Check in", "kind": "stay", "day": "5 Jan"}]}}),
            )
        };
        match turn {
            0 => vec![plan()],
            1 => {
                assert!(
                    seen.contains("plan: steps[1].day is not a field of steps[1]"),
                    "{seen}"
                );
                vec![plan()]
            }
            2 => {
                assert!(
                    seen.contains("Left out to place it: steps[1].day (not a field of plan); steps[0].detail (300 characters; the limit is 280)"),
                    "{seen}"
                );
                vec![
                    call(
                        "r",
                        "create",
                        json!({"kind": "reply", "title": "Your trip", "data": {
                        "headline": "Two steps to San Francisco", "text": "Fly, then check in."}}),
                    ),
                    call(
                        "f",
                        "finish",
                        json!({"say": "Your trip is on the canvas.", "title": "Trip to San Francisco"}),
                    ),
                ]
            }
            _ => panic!("the run finished at turn 2"),
        }
    }
    let run = scripted("Plan my trip", turns).await;
    assert_eq!(run.status, WorkExecutionStatus::NeedsReview);
    let plan = run
        .artifacts
        .iter()
        .find_map(|a| match &a.data {
            WorkArtifactDataV1::Plan { steps, .. } => Some(steps.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(plan[0].detail, None);
    assert_eq!(plan[1].title, "Check in");
    assert_eq!(run.title.as_deref(), Some("Trip to San Francisco"));
}

#[tokio::test]
async fn a_run_that_stops_moving_closes_with_what_it_has() {
    fn turns(_: usize, _: &str) -> Vec<WorkModelPart> {
        vec![WorkModelPart::Text("Thinking about it.".into())]
    }
    let run = scripted("Plan my day", turns).await;
    assert_eq!(run.status, WorkExecutionStatus::NeedsReview);
    assert!(run.steps.iter().all(|s| s.status != WorkStepStatus::Failed));
    let reply = run
        .artifacts
        .iter()
        .find_map(|a| match &a.data {
            WorkArtifactDataV1::Reply { headline, text, .. } => {
                Some((headline.clone(), text.clone()))
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(reply.0, "I couldn't finish this");
    assert!(reply
        .1
        .contains("I stopped before I could put the rest together."));
    assert!(matches!(
        run.steps.last().unwrap().kind,
        WorkStepKindV1::Finish { .. }
    ));
}

/// A lead run over the person's engine: `browse` answers every page the
/// run opens, results pages included.
async fn scripted_search(
    objective: &str,
    turns: fn(usize, &str) -> Vec<WorkModelPart>,
    search: &dyn WorkPublicSearchProvider,
    browse: fn(WorkAgentBrowseRequest) -> WorkBrowserOutcome,
) -> WorkExecutionFact {
    let store = Arc::new(zephium_store::SqliteStore::in_memory().unwrap());
    let (mut shell, queue, handle, profile) = fixture(store);
    let create = handle
        .work_document(WorkIntent::Create {
            objective: objective.into(),
        })
        .unwrap();
    let work = create.work_id().unwrap();
    drive(&mut shell, &queue, create).await.unwrap();
    let lead = LeadModel {
        client: Arc::new(Turns(turns)),
        ..model(Arc::new(Script {
            calls: Mutex::new(Vec::new()),
        }))
    };
    let models = WorkLeadModels {
        lead: lead.clone(),
        page: lead.clone(),
        light: lead,
    };
    let done = drive(
        &mut shell,
        &queue,
        WorkLeadService::new(handle.clone()).run(
            profile,
            command(work, WorkRevision::INITIAL),
            None,
            models,
            search,
            |_, request| async move { Ok(browse(request)) },
            |_| {},
        ),
    )
    .await
    .unwrap();
    done.executions[0].clone()
}

fn outcome(
    search: Option<WorkSearchPage>,
    artifacts: Vec<WorkArtifactDraft>,
) -> WorkBrowserOutcome {
    WorkBrowserOutcome {
        status: WorkStepStatus::Succeeded,
        usage: Some(WorkUsage {
            model_tokens: 0,
            cost_micro_usd: 0,
            operations: 1,
            accounting: WorkUsageAccounting::Exact,
        }),
        artifacts,
        intervention: None,
        note: None,
        measurements: None,
        helped: false,
        held_back: false,
        rerun: false,
        search,
        redirect: None,
    }
}

/// The search a browser search falls back to, counting what it is asked.
struct Fallback(Arc<std::sync::atomic::AtomicUsize>);
impl WorkPublicSearchProvider for Fallback {
    fn search<'a>(
        &'a self,
        scope: &'a WorkPublicSearchScope,
        _: &'a [zephium_core::work::context::WorkContextBody],
        limits: WorkExecutionLimits,
    ) -> WorkPublicSearchFuture<'a> {
        Box::pin(async move {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(scope.provider, WorkSearchProvider::Native);
            let asked = WorkPublicSearchScope {
                query: format!("{} ESTA", scope.query),
                ..scope.clone()
            };
            Search.search(&asked, &[], limits).await
        })
    }
}

fn browser_search(asked: &Arc<std::sync::atomic::AtomicUsize>) -> zephium_agentic::BrowserSearch {
    zephium_agentic::BrowserSearch::try_new(zephium_core::search::SearchEngine::DuckDuckGo, "")
        .unwrap()
        .with_fallback(
            Box::new(Fallback(asked.clone())),
            WorkSearchFallbackV1 {
                provider: WorkSearchProvider::Native,
                model: SEARCH_MODEL.into(),
            },
        )
        .unwrap()
}

#[tokio::test]
async fn a_lead_search_reads_the_engines_results_then_the_page_it_picks() {
    fn turns(turn: usize, seen: &str) -> Vec<WorkModelPart> {
        match turn {
            0 => vec![call(
                "s",
                "web_search",
                json!({"query": "rust async runtime", "freshness": "week"}),
            )],
            1 => {
                assert!(seen.contains("Results from DuckDuckGo"), "{seen}");
                assert!(
                    seen.contains(
                        "1. [s1] Tokio - An asynchronous Rust runtime — https://tokio.rs/"
                    ),
                    "{seen}"
                );
                assert!(
                    seen.contains("   Tokio is an event-driven platform."),
                    "{seen}"
                );
                assert!(
                    seen.contains("2. [s2] async-std — https://async.rs/"),
                    "{seen}"
                );
                vec![call("f", "web_fetch", json!({"url": "https://tokio.rs/"}))]
            }
            2 => {
                assert!(
                    seen.contains("Tokio schedules tasks on a work-stealing pool"),
                    "{seen}"
                );
                vec![
                    call(
                        "r",
                        "create",
                        json!({"kind": "reply", "title": "Tokio", "data": {
                        "headline": "Use Tokio", "text": "It is the common async runtime."}}),
                    ),
                    call(
                        "d",
                        "finish",
                        json!({"say": "Done.", "title": "Rust async runtime"}),
                    ),
                ]
            }
            _ => panic!("the run finished at turn 2"),
        }
    }
    fn browse(request: WorkAgentBrowseRequest) -> WorkBrowserOutcome {
        if let Some(page) = &request.results {
            assert!(
                matches!(&request.step, WorkStepKindV1::Search { query } if query == "rust async runtime (past week)")
            );
            assert!(request.session == crate::work_sites::SiteSession::Private);
            assert_eq!(
                page.url.as_str(),
                "https://duckduckgo.com/?q=rust+async+runtime&df=w"
            );
            return outcome(
                Some(WorkSearchPage::Results(vec![
                    WorkSearchHit {
                        title: "Tokio - An asynchronous Rust runtime".into(),
                        url: "https://tokio.rs/".into(),
                        snippet: Some("Tokio is an event-driven platform.".into()),
                    },
                    WorkSearchHit {
                        title: "async-std".into(),
                        url: "https://async.rs/".into(),
                        snippet: None,
                    },
                ])),
                vec![],
            );
        }
        let WorkStepKindV1::Read {
            url, goal: None, ..
        } = &request.step
        else {
            panic!("a page read");
        };
        assert_eq!(url, "https://tokio.rs/");
        outcome(
            None,
            vec![WorkArtifactDraft {
                output: request.output,
                title: "Notes from tokio.rs".into(),
                data: WorkArtifactDataV1::Findings {
                    subjects: vec![],
                    items: vec![WorkFinding {
                        claim: "Tokio schedules tasks on a work-stealing pool".into(),
                        subject: None,
                        evidence: vec![0],
                        confidence: WorkConfidence::Supported,
                        detail: None,
                        general_knowledge: false,
                    }],
                },
                evidence: vec![WorkEvidenceLink {
                    extraction_id: WorkArtifactId::from(901),
                    source_id: 1,
                }],
            }],
        )
    }
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let run = scripted_search(
        "Which async runtime should I use in Rust?",
        turns,
        &browser_search(&asked),
        browse,
    )
    .await;
    assert_eq!(run.status, WorkExecutionStatus::NeedsReview);
    let evidence = &run.provider_evidence[0].evidence;
    assert_eq!(evidence.provider, WorkSearchProvider::Browser);
    assert_eq!(evidence.model, "duckduckgo");
    assert!(evidence.answer.is_empty());
    assert_eq!(evidence.citations.len(), 2);
    let search = run
        .steps
        .iter()
        .find(|step| matches!(step.kind, WorkStepKindV1::Search { .. }))
        .unwrap();
    assert_eq!(search.status, WorkStepStatus::Succeeded);
    let usage = search.usage.unwrap();
    assert_eq!((usage.model_tokens, usage.cost_micro_usd), (0, 0));
    assert!(run.steps.iter().any(|step| matches!(
        &step.kind,
        WorkStepKindV1::Read { url, .. } if url == "https://tokio.rs/"
    )));
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_challenged_results_page_falls_back_to_the_models_own_search() {
    fn turns(turn: usize, seen: &str) -> Vec<WorkModelPart> {
        match turn {
            0 => vec![call("s", "web_search", json!({"query": "us entry rules"}))],
            1 => {
                assert!(seen.contains("Polish citizens travel on ESTA"), "{seen}");
                vec![
                    call(
                        "r",
                        "create",
                        json!({"kind": "reply", "title": "Entry", "data": {
                        "headline": "ESTA", "text": "Apply for an ESTA."}}),
                    ),
                    call(
                        "d",
                        "finish",
                        json!({"say": "Done.", "title": "Rust async runtime"}),
                    ),
                ]
            }
            _ => panic!("the run finished at turn 1"),
        }
    }
    fn browse(request: WorkAgentBrowseRequest) -> WorkBrowserOutcome {
        assert!(request.results.is_some(), "only the results page opens");
        outcome(
            Some(WorkSearchPage::Wall(WorkSearchWall::Challenge)),
            vec![],
        )
    }
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let run = scripted_search(
        "What does a Pole need to enter the US?",
        turns,
        &browser_search(&asked),
        browse,
    )
    .await;
    assert_eq!(run.status, WorkExecutionStatus::NeedsReview);
    // The fallback's evidence belongs to the run under its browser grant.
    let evidence = &run.provider_evidence[0].evidence;
    assert_eq!(evidence.provider, WorkSearchProvider::Native);
    assert_eq!(evidence.model, SEARCH_MODEL);
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
}

/// A read whose first load the page's server redirected: the browser read
/// failed naming where, as the composition reports it.
fn redirected(to: &str) -> WorkBrowserOutcome {
    WorkBrowserOutcome {
        status: WorkStepStatus::Failed,
        usage: Some(WorkUsage {
            model_tokens: 0,
            cost_micro_usd: 0,
            operations: 1,
            accounting: WorkUsageAccounting::Exact,
        }),
        artifacts: vec![],
        intervention: None,
        note: Some(format!("The page redirects to {to}")),
        measurements: None,
        helped: false,
        held_back: false,
        rerun: false,
        search: None,
        redirect: Some(to.into()),
    }
}

fn notes(request: WorkAgentBrowseRequest, claim: &str) -> WorkBrowserOutcome {
    outcome(
        None,
        vec![WorkArtifactDraft {
            output: request.output,
            title: "Notes".into(),
            data: WorkArtifactDataV1::Findings {
                subjects: vec![],
                items: vec![WorkFinding {
                    claim: claim.into(),
                    subject: None,
                    evidence: vec![0],
                    confidence: WorkConfidence::Supported,
                    detail: None,
                    general_knowledge: false,
                }],
            },
            evidence: vec![WorkEvidenceLink {
                extraction_id: WorkArtifactId::from(902),
                source_id: 1,
            }],
        }],
    )
}

fn finish() -> Vec<WorkModelPart> {
    vec![
        call(
            "r",
            "create",
            json!({"kind": "reply", "title": "Plans", "data": {
            "headline": "Plans", "text": "The plans are listed."}}),
        ),
        call("d", "finish", json!({"say": "Done.", "title": "Plans"})),
    ]
}

fn reads(run: &WorkExecutionFact) -> Vec<(String, WorkStepStatus, Option<String>, usize)> {
    run.steps
        .iter()
        .filter_map(|step| match &step.kind {
            WorkStepKindV1::Read { url, .. } => Some((
                url.clone(),
                step.status,
                step.note.clone(),
                step.artifacts.len(),
            )),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_same_site_redirect_is_read_again_as_one_fresh_read_of_its_destination() {
    fn turns(turn: usize, seen: &str) -> Vec<WorkModelPart> {
        match turn {
            0 => vec![call(
                "f",
                "web_fetch",
                json!({"url": "https://hetzner.com/cloud"}),
            )],
            1 => {
                assert!(
                    seen.contains("https://hetzner.com/cloud redirects to https://www.hetzner.com/cloud; this is a read of https://www.hetzner.com/cloud."),
                    "{seen}"
                );
                assert!(seen.contains("CX22 costs 3.79 EUR a month"), "{seen}");
                assert!(!seen.contains("Failed"), "{seen}");
                finish()
            }
            _ => panic!("the run finished at turn 1"),
        }
    }
    fn browse(request: WorkAgentBrowseRequest) -> WorkBrowserOutcome {
        let WorkStepKindV1::Read { url, .. } = &request.step else {
            panic!("a page read");
        };
        match url.as_str() {
            "https://hetzner.com/cloud" => {
                assert_eq!(request.redirected_from, None);
                redirected("https://www.hetzner.com/cloud")
            }
            "https://www.hetzner.com/cloud" => {
                assert_eq!(
                    request.redirected_from.as_deref(),
                    Some("https://hetzner.com/cloud")
                );
                notes(request, "CX22 costs 3.79 EUR a month")
            }
            other => panic!("unexpected read of {other}"),
        }
    }
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let run = scripted_search(
        "Hetzner cloud prices",
        turns,
        &browser_search(&asked),
        browse,
    )
    .await;
    assert_eq!(run.status, WorkExecutionStatus::NeedsReview);
    assert_eq!(
        reads(&run),
        [
            (
                "https://hetzner.com/cloud".into(),
                WorkStepStatus::Failed,
                Some("The page redirects to https://www.hetzner.com/cloud".into()),
                0,
            ),
            (
                "https://www.hetzner.com/cloud".into(),
                WorkStepStatus::Succeeded,
                reads(&run)[1].2.clone(),
                1,
            ),
        ]
    );
}

#[tokio::test]
async fn a_public_cross_site_redirect_is_isolated_and_chains_stop_at_their_bound_or_loop() {
    fn turns(turn: usize, seen: &str) -> Vec<WorkModelPart> {
        match turn {
            0 => vec![
                call(
                    "a",
                    "web_fetch",
                    json!({"url": "https://plans.example.com/a"}),
                ),
                call(
                    "b",
                    "web_fetch",
                    json!({"url": "https://loop.example.org/a"}),
                ),
                call(
                    "c",
                    "web_fetch",
                    json!({"url": "https://round.example.org/a"}),
                ),
                // Work never loads plain http: this reads the https page.
                call("e", "web_fetch", json!({"url": "http://example.com/"})),
            ],
            1 => {
                assert!(
                    seen.contains("https://plans.example.com/a redirects to https://other.example.net/plans; this is a read of https://other.example.net/plans.") && seen.contains("The destination lists its plans"),
                    "{seen}"
                );
                assert!(
                    seen.contains("https://loop.example.org/a redirects to https://loop.example.org/c; this is a read of https://loop.example.org/c.\nFailed: The page redirects to https://loop.example.org/d\nRead https://loop.example.org/d if it is what you need"),
                    "{seen}"
                );
                assert!(
                    seen.contains("https://round.example.org/a redirects to https://round.example.org/b; this is a read of https://round.example.org/b.\nFailed: The page redirects to https://round.example.org/a\n"),
                    "{seen}"
                );
                finish()
            }
            _ => panic!("the run finished at turn 1"),
        }
    }
    fn browse(request: WorkAgentBrowseRequest) -> WorkBrowserOutcome {
        let WorkStepKindV1::Read { url, .. } = &request.step else {
            panic!("a page read");
        };
        match url.as_str() {
            "https://example.com/" => notes(request, "Example Domain is for examples"),
            "https://round.example.org/a" => redirected("https://round.example.org/b"),
            "https://round.example.org/b" => redirected("https://round.example.org/a"),
            "https://plans.example.com/a" => redirected("https://other.example.net/plans"),
            "https://other.example.net/plans" => {
                assert!(request.private_run);
                assert_eq!(request.session, crate::work_sites::SiteSession::Private);
                assert!(!request.allow_edits);
                assert!(matches!(request.step, WorkStepKindV1::Read { goal: None, .. }));
                notes(request, "The destination lists its plans")
            },
            "https://loop.example.org/a" => redirected("https://loop.example.org/b"),
            "https://loop.example.org/b" => redirected("https://loop.example.org/c"),
            "https://loop.example.org/c" => redirected("https://loop.example.org/d"),
            other => panic!("unexpected read of {other}"),
        }
    }
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let run = scripted_search("Compare plans", turns, &browser_search(&asked), browse).await;
    assert_eq!(run.status, WorkExecutionStatus::NeedsReview);
    let urls: Vec<String> = reads(&run).into_iter().map(|(url, ..)| url).collect();
    // Each address is read once: the bound ends one chain, the loop another.
    assert_eq!(urls.len(), 8, "{urls:?}");
    assert!(urls.iter().any(|url| url == "https://example.com/"));
    assert_eq!(urls.iter().filter(|url| url.contains("other.example.net")).count(), 1);
    assert!(!urls.iter().any(|url| url.ends_with("/d")));
}

struct FolderHandoffModel {
    root: String,
    helper_calls: std::sync::atomic::AtomicUsize,
}
impl WorkModelClient for FolderHandoffModel {
    fn call<'a>(
        &'a self,
        request: WorkModelRequest,
        _: &'a (dyn Fn(WorkModelEvent) + Send + Sync),
    ) -> WorkModelFuture<'a> {
        Box::pin(async move {
            let lead = request.tools.iter().any(|tool| tool.name == "start_part");
            let turn = request
                .messages
                .iter()
                .filter(|message| matches!(message, WorkModelMessage::Assistant(_)))
                .count();
            let assistant = if lead {
                match turn {
                    0 => ["First", "Second"].into_iter().enumerate().map(|(i,title)| call(&format!("part-{i}"), "start_part", json!({"title":title,"helper":"computer","goal":"Use the chosen project folder for this synthetic handoff fixture"}))).collect(),
                    1 => vec![call("part-again","start_part",json!({"title":"Again","helper":"computer","goal":"Use the same folder for this synthetic handoff fixture"}))],
                    _ => vec![call("reply","create",json!({"kind":"reply","title":"Folder handoff","data":{"headline":"Folder handoff recorded","text":"This synthetic fixture performed no file changes."}})),call("finish","finish",json!({"say":"The fixture is complete."}))],
                }
            } else {
                let context = request
                    .messages
                    .iter()
                    .filter_map(|message| match message {
                        WorkModelMessage::User(parts) => Some(
                            parts
                                .iter()
                                .filter_map(|part| match part {
                                    WorkModelPart::Text(text) => Some(text.as_str()),
                                    _ => None,
                                })
                                .collect::<String>(),
                        ),
                        _ => None,
                    })
                    .collect::<String>();
                assert!(
                    context.contains(&self.root),
                    "the helper receives only the chosen root"
                );
                self.helper_calls
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                vec![call(
                    "helper-finish",
                    "finish",
                    json!({"summary":"Scoped","digest":"Synthetic permission routing only; no file reads or changes.","found":true}),
                )]
            };
            Ok(WorkModelOutcome {
                stop: WorkModelStop::ToolUse,
                usage: WorkModelUsage {
                    input_tokens: 100,
                    cached_input_tokens: 0,
                    cache_write_input_tokens: 0,
                    output_tokens: 20,
                    reasoning_tokens: 0,
                    cost_micros: Some(1),
                },
                assistant,
            })
        })
    }
}

#[tokio::test]
async fn zero_folder_computer_parts_share_picker_and_only_accept_real_choice() {
    let _serial = crate::WORK_RUNTIME_TEST_SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let folder = tempfile::Builder::new()
        .prefix("zephium-picker-home-")
        .tempdir()
        .unwrap();
    let _home = crate::work_files::TestGrantHome::new(folder.path().canonicalize().unwrap());
    let project = folder.path().join("Documents/project");
    std::fs::create_dir_all(&project).unwrap();
    let root = project
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    for mode in ["picked", "cancelled", "bare-label", "stopped"] {
        let store = Arc::new(zephium_store::SqliteStore::in_memory().unwrap());
        let (mut shell, queue, handle, profile) = fixture(store);
        let create = handle
            .work_document(WorkIntent::Create {
                objective: "Inspect my project with the Computer helper".into(),
            })
            .unwrap();
        let work = create.work_id().unwrap();
        drive(&mut shell, &queue, create).await.unwrap();
        let client = Arc::new(FolderHandoffModel {
            root: root.clone(),
            helper_calls: std::sync::atomic::AtomicUsize::new(0),
        });
        let lead = model(client.clone());
        let models = WorkLeadModels {
            lead: lead.clone(),
            page: lead.clone(),
            light: lead,
        };
        let answered = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let answering = {
            let handle = handle.clone();
            let answered = answered.clone();
            let root = root.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    let Ok(read) = handle.work_projection(profile, work) else {
                        continue;
                    };
                    let zephium_ipc::work::WorkReplyV1::Projection { projection } =
                        read.response(profile).await.reply
                    else {
                        continue;
                    };
                    let Some(execution) = projection.executions.last() else {
                        continue;
                    };
                    let Some(step) = execution.steps.iter().find(|step| {
                        step.status == WorkStepStatus::Running
                            && matches!(
                                step.kind,
                                WorkStepKindV1::Ask {
                                    purpose: Some(WorkAskPurposeV1::Folder),
                                    ..
                                }
                            )
                    }) else {
                        continue;
                    };
                    assert!(step
                        .local
                        .as_ref()
                        .is_some_and(|local| local.folder.is_none()));
                    assert!(execution
                        .parts
                        .iter()
                        .any(|part| part.state == WorkPartStateV1::Waiting
                            && part.need == Some(WorkPartNeedV1::ChooseFolder)));
                    assert_eq!(
                        answered.load(std::sync::atomic::Ordering::SeqCst),
                        0,
                        "a declined or granted shared picker is not asked again"
                    );
                    let intent = if mode == "stopped" {
                        WorkRuntimeIntent::Cancel {
                            execution: execution.id,
                            intervention: None,
                        }
                    } else {
                        WorkRuntimeIntent::AnswerStep {
                            execution: execution.id,
                            step: step.id,
                            answer: match mode {
                                "picked" => root.clone(),
                                "bare-label" => "Choose folder…".into(),
                                _ => "Not now".into(),
                            },
                        }
                    };
                    let result = handle
                        .work_command(
                            profile,
                            WorkCommandV1 {
                                version: 1,
                                work,
                                expected_revision: projection.work.revision,
                                command: WorkCommandId::generate(),
                                intent,
                            },
                        )
                        .unwrap()
                        .await;
                    if result.is_ok() {
                        answered.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                }
            })
        };
        let service = WorkLeadService::new(handle.clone());
        let result = drive(
            &mut shell,
            &queue,
            tokio::time::timeout(
                std::time::Duration::from_secs(15),
                service.run(
                    profile,
                    command(work, WorkRevision::INITIAL),
                    None,
                    models,
                    &Search,
                    |_, _| async { panic!("no web browsing in folder fixture") },
                    |_| {},
                ),
            ),
        )
        .await;
        answering.abort();
        let projection = result.expect("bounded handoff").expect("settled handoff");
        assert_eq!(answered.load(std::sync::atomic::Ordering::SeqCst), 1);
        let execution = &projection.executions[0];
        assert!(execution.parts.iter().all(|part| part.state.terminal()));
        assert!(!execution
            .steps
            .iter()
            .any(|step| step.status == WorkStepStatus::Running));
        if mode == "picked" {
            assert!(
                client
                    .helper_calls
                    .load(std::sync::atomic::Ordering::SeqCst)
                    >= 2
            );
            assert!(execution
                .parts
                .iter()
                .all(|part| part.need != Some(WorkPartNeedV1::ChooseFolder)));
        } else {
            assert_eq!(
                client
                    .helper_calls
                    .load(std::sync::atomic::Ordering::SeqCst),
                0
            );
            assert!(
                execution
                    .parts
                    .iter()
                    .all(|part| part.need == Some(WorkPartNeedV1::ChooseFolder)),
                "{mode}"
            );
        }
        assert!(!execution.steps.iter().any(|step| step.kind.files()));
    }
}

#[tokio::test]
async fn list_record_source_survives_real_propose_and_read_canvas_without_private_pixels() {
    const PRIVATE: &str = "Confidential synthetic invoice total is 47219 euros";
    fn turns(turn: usize, seen: &str) -> Vec<WorkModelPart> {
        match turn {
            0 => vec![call(
                "read",
                "web_fetch",
                json!({"url":"https://records.example.com/one"}),
            )],
            1 => {
                assert!(
                    seen.contains("[s"),
                    "synthetic fetch returned no source key: {seen}"
                );
                let key = first_key(seen);
                assert!(!seen.contains("native_crop"));
                vec![call(
                    "make",
                    "create",
                    json!({"kind":"list","title":"Reading records","data":{"style":"reading","items":[
                        {"title":"Keep exact row","detail":"Kept detail","source":key,"record_source":key,
                         "record_media":{"origin":"native_crop","digest":"MODEL_FORGED_REFERENCE"},"media_attempted":true},
                        {"title":"Keep unknown row text","detail":"Still available","source":key,"record_source":"missing-source-key"}
                    ]}}),
                )]
            }
            2 => {
                assert!(seen.contains("Placed list"), "{seen}");
                assert!(
                    seen.contains("record_source"),
                    "optional unknown source should be reported"
                );
                let id = seen
                    .split("Placed list ")
                    .nth(1)
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .trim_end_matches('.');
                assert!(
                    WorkArtifactId::parse(id).is_some(),
                    "valid object id from create result"
                );
                vec![call("canvas", "read_canvas", json!({"ids":[id]}))]
            }
            3 => {
                let line = seen
                    .lines()
                    .find(|line| line.starts_with("[{") && line.contains("Reading records"))
                    .unwrap_or_else(|| panic!("synthetic canvas returned: {seen}"));
                let read: Value = serde_json::from_str(line).unwrap();
                let data = &read[0]["data"];
                assert!(data["items"][0]["record_source"]
                    .as_str()
                    .is_some_and(|key| key.starts_with('s')));
                assert!(data["items"][1].get("record_source").is_none());
                assert_eq!(data["items"][1]["detail"], "Still available");
                assert!(!line.contains("record_media"));
                assert!(!line.contains("media_attempted"));
                assert!(!line.contains("native_crop"));
                assert!(!line.contains("MODEL_FORGED_REFERENCE"));
                vec![call(
                    "private-search",
                    "web_search",
                    json!({"query":PRIVATE}),
                )]
            }
            4 => {
                assert!(
                    seen.contains("private") || seen.contains("person's"),
                    "private disclosure must be refused before public search: {seen}"
                );
                vec![
                    call(
                        "reply",
                        "create",
                        json!({"kind":"reply","title":"Records ready","data":{"headline":"Records ready","text":"Both list texts stay available."}}),
                    ),
                    call(
                        "finish",
                        "finish",
                        json!({"say":"The text remains available.","title":"Reading records"}),
                    ),
                ]
            }
            _ => panic!("unexpected extra turn {turn}"),
        }
    }
    let store = Arc::new(zephium_store::SqliteStore::in_memory().unwrap());
    let (mut shell, queue, handle, profile) = fixture(store);
    let create = handle
        .work_document(WorkIntent::Create {
            objective: "Read the record at https://records.example.com/one and keep a list".into(),
        })
        .unwrap();
    let work = create.work_id().unwrap();
    drive(&mut shell, &queue, create).await.unwrap();
    let lead = LeadModel {
        client: Arc::new(Turns(turns)),
        ..model(Arc::new(Script {
            calls: Mutex::new(Vec::new()),
        }))
    };
    let models = WorkLeadModels {
        lead: lead.clone(),
        page: lead.clone(),
        light: lead,
    };
    let done=drive(&mut shell,&queue,WorkLeadService::new(handle).run(profile,command(work,WorkRevision::INITIAL),None,models,&Search,|owner,request|async move{
        assert!(request.results.is_none(),"private query must never reach a results page");
        let source=WorkEvidenceLink{extraction_id:WorkArtifactId::from(910),source_id: 1};
        let media=zephium_core::work::media::WorkRecordMediaRefV1{profile:owner.profile(),work:owner.work(),execution:owner.execution(),attempt:owner.attempt(),step:request.id,extraction:source.extraction_id,record:0,source:"a".repeat(64),digest:"b".repeat(64),width:128,height:96,origin:zephium_core::work::media::WorkRecordMediaOriginV1::NativeCrop};
        let mut result=outcome(None,vec![WorkArtifactDraft{output:request.output,title:"Native records".into(),evidence:vec![source],data:serde_json::from_value(json!({"kind":"comparison_matrix","subjects":[{"name":PRIVATE,"record_media":media,"media_attempted":true}],"criteria":[{"name":"Title","kind":{"kind":"text"}}],"cells":[[{"value":{"kind":"text","text":PRIVATE},"evidence":[0]}]]})).unwrap()}]);
        // Metadata-only fixture: no PNG or Store image asset is created.
        let draft=&result.artifacts[0];
        WorkArtifactV1{version:1,id:WorkArtifactId::generate(),execution:owner.execution(),node:owner.node(),attempt:owner.attempt(),output:draft.output.clone(),title:draft.title.clone(),data:draft.data.clone(),evidence:draft.evidence.clone(),review:WorkOutputReview::SourceMappedNeedsReview,presentation:WorkArtifactPresentationV1::Automatic,general_knowledge:false,revises:None,part:None}.validate().expect("valid synthetic source before browser result");
        result.measurements=Some(WorkStepMeasurementsV1{resolved_handoff:Some(WorkPageHandoffKindV1::SignIn),..Default::default()});Ok(result)
    },|_|{})).await.unwrap();
    let execution = &done.executions[0];
    let list = execution
        .artifacts
        .iter()
        .find(|artifact| matches!(artifact.data, WorkArtifactDataV1::List { .. }))
        .unwrap();
    let WorkArtifactDataV1::List { items, .. } = &list.data else {
        panic!()
    };
    assert_eq!(items.len(), 2);
    assert!(items[0].record_source.is_some());
    assert!(items[0].record_media.is_some());
    assert!(items[1].record_source.is_none());
    assert!(items[1].record_media.is_none());
    assert_eq!(items[1].title, "Keep unknown row text");
    assert!(crate::work_agent::private_artifact_texts([(
        &execution.steps[..],
        &execution.artifacts[..]
    )])
    .iter()
    .any(|text| text.contains("Keep exact row")));
    assert!(execution
        .steps
        .iter()
        .filter(|step| matches!(step.kind, WorkStepKindV1::Search { .. }))
        .all(|step| step.status != WorkStepStatus::Succeeded));
}

struct FailedSiteAdvice {
    account: bool,
}
impl WorkModelClient for FailedSiteAdvice {
    fn call<'a>(
        &'a self,
        request: WorkModelRequest,
        _: &'a (dyn Fn(WorkModelEvent) + Send + Sync),
    ) -> WorkModelFuture<'a> {
        Box::pin(async move {
            let turn = request
                .messages
                .iter()
                .filter(|message| matches!(message, WorkModelMessage::Assistant(_)))
                .count();
            let lead = request
                .system
                .iter()
                .any(|block| block.text.contains("You are the Work agent"));
            let seen = results(&request);
            let assistant = match (lead, turn) {
                (true, 0) => vec![call(
                    "part",
                    "start_part",
                    json!({"title":"Account","helper":"browser","service":"work.example.test","goal":if self.account {"Read my account inbox"}else{"Read the public documentation"}}),
                )],
                (true, 1) => {
                    assert!(seen.contains("could not do its job"), "{seen}");
                    assert_eq!(
                        seen.contains("search the web once"),
                        !self.account,
                        "account={} {seen}",
                        self.account
                    );
                    vec![call(
                        "reply",
                        "create",
                        json!({"kind":"reply","title":"Site unavailable","data":{"headline":"The requested content could not be read","text":"The browser helper could not read the requested site."}}),
                    )]
                }
                (true, _) => vec![call(
                    "finish",
                    "finish",
                    json!({"say":"The requested content could not be read."}),
                )],
                (false, _) => vec![call(
                    "helper-finish",
                    "finish",
                    json!({"summary":"Could not read the site","digest":"The browser did not obtain the requested content.","found":false,"need":{"kind":"retry","target":"https://work.example.test/"}}),
                )],
            };
            Ok(WorkModelOutcome {
                stop: WorkModelStop::ToolUse,
                assistant,
                usage: WorkModelUsage {
                    input_tokens: 100,
                    cached_input_tokens: 0,
                    cache_write_input_tokens: 0,
                    output_tokens: 10,
                    reasoning_tokens: 0,
                    cost_micros: None,
                },
            })
        })
    }
}
#[tokio::test]
async fn failed_account_parts_never_suggest_public_source_substitution() {
    for account in [true, false] {
        let store = Arc::new(zephium_store::SqliteStore::in_memory().unwrap());
        let (mut shell, queue, handle, profile) = fixture(store);
        let create = handle
            .work_document(WorkIntent::Create {
                objective: if account {
                    "Read my account inbox".into()
                } else {
                    "Read the public documentation".into()
                },
            })
            .unwrap();
        let work = create.work_id().unwrap();
        drive(&mut shell, &queue, create).await.unwrap();
        let lead = LeadModel {
            client: Arc::new(FailedSiteAdvice { account }),
            ..model(Arc::new(Script {
                calls: Mutex::new(Vec::new()),
            }))
        };
        let models = WorkLeadModels {
            lead: lead.clone(),
            page: lead.clone(),
            light: lead,
        };
        let projection = drive(
            &mut shell,
            &queue,
            WorkLeadService::new(handle).run(
                profile,
                command(work, WorkRevision::INITIAL),
                None,
                models,
                &Search,
                |_, _| async { panic!("scripted failure requires no browsing") },
                |_| {},
            ),
        )
        .await
        .unwrap();
        let execution = &projection.executions[0];
        assert_eq!(execution.parts.len(), 1);
        assert_eq!(execution.parts[0].state, WorkPartStateV1::Failed);
        assert!(!execution
            .steps
            .iter()
            .any(|step| step.status == WorkStepStatus::Running));
    }
}
