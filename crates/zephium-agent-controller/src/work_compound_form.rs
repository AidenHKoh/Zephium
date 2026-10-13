//! Bounded native preparation of one model-proposed search/filter form.
use super::*;

const MAX_FORM_ACTIONS: usize = 24;
const MAX_FORM_MILLIS: u64 = 12_000;

pub(super) enum PreparedForm {
    Refresh(AgentProviderObservationCheckpoint),
    Ready(Box<SemanticObservation>),
    Follow(Box<(AgentProviderContinuation, ContextNavigationTarget)>),
}

impl AgentWorkController {
    pub(super) fn can_batch_search_fill(
        proposal: &AgentBrowserToolProposal,
        look: &SemanticObservation,
    ) -> bool {
        let AgentBrowserToolProposal::Act(batch) = proposal else {
            return false;
        };
        let [action] = batch.actions() else {
            return false;
        };
        Self::is_batchable_search_fill(action, look)
    }

    fn is_batchable_search_fill(
        action: &SemanticActionProposal,
        look: &SemanticObservation,
    ) -> bool {
        let SemanticActionIntent::Fill { target, .. } = action.intent() else {
            return false;
        };
        if action.effect() != SemanticEffectClass::LocalWrite {
            return false;
        }
        let Ok(frame_id) = look.reference_frame(*target) else {
            return false;
        };
        let Some(frame) = look.frames().iter().find(|frame| frame.frame() == frame_id) else {
            return false;
        };
        let Some(index) = frame
            .nodes()
            .iter()
            .position(|node| node.reference() == *target)
        else {
            return false;
        };
        let search_scope = |mut at: usize| loop {
            let node = &frame.nodes()[at];
            if node.landmark_kind() == Some(SemanticLandmarkKind::Search) {
                return Some(at);
            }
            at = usize::from(node.parent()?);
        };
        let Some(scope) = search_scope(index) else {
            return false;
        };
        let eligible = |node: &SemanticNode| {
            node.sensitivity() == SemanticSensitivity::Public
                && node.operations().contains(SemanticOperationClass::Fill)
                && node.form().is_some_and(|form| {
                    form.method() == SemanticFormMethod::Get && form.same_origin() && form.search()
                })
        };
        eligible(&frame.nodes()[index])
            && frame
                .nodes()
                .iter()
                .enumerate()
                .filter(|(at, node)| eligible(node) && search_scope(*at) == Some(scope))
                .take(2)
                .count()
                == 2
    }

    pub(super) async fn prepare_compound_form(
        state: &mut WorkState,
        worker: &mut AgentRuntimeWorker,
        browser: &WorkBrowser<'_>,
        turn: AgentBrowserProviderTurn,
        original: &SemanticObservation,
    ) -> Result<PreparedForm, AgentWorkFailure> {
        let session = state.session.as_ref().ok_or(AgentWorkFailure::Contract)?;
        let config = session.config.clone();
        let binding = match turn.turn.proposal() {
            AgentBrowserToolProposal::FillForm(form) => turn
                .turn
                .continuation()
                .bind_form_targets(form, original, &config),
            _ => return Err(AgentWorkFailure::Contract),
        };
        let (_, continuation) = turn.into_tool_turn().into_parts();
        let binding = match binding {
            Ok(binding) => binding,
            Err(error) if error.fault != AgentBrowserFormFault::Authority => {
                return continuation
                    .into_form_refresh(original, &config, 0, false, Some(error))
                    .map(PreparedForm::Refresh)
                    .map_err(|_| AgentWorkFailure::Contract)
            }
            Err(_) => return Err(AgentWorkFailure::Contract),
        };
        let mut current = original.clone();
        let deadline = Instant::now() + Duration::from_millis(MAX_FORM_MILLIS);
        let mut actions = 0usize;
        let mut committed_choices = [false; MAX_AGENT_BROWSER_FORM_FIELDS];
        let mut completed = 0u8;
        let mut failure = None;
        'fields: for index in 0..binding.field_count() {
            let mut waited = false;
            loop {
                state.check_task_contract()?;
                state.native.check_control(worker, browser)?;
                if actions >= MAX_FORM_ACTIONS || Instant::now() >= deadline {
                    failure = Some(AgentBrowserFormError {
                        field: Some((index + 1) as u8),
                        fault: AgentBrowserFormFault::Limit,
                    });
                    break 'fields;
                }
                if committed_choices[index] && binding.value_matches(index, &current) {
                    completed += 1;
                    break;
                }
                let recipe = match binding.field_action(index, &current) {
                    Ok(None) => {
                        completed += 1;
                        break;
                    }
                    Ok(Some(recipe)) => recipe,
                    Err(error)
                        if error.fault == AgentBrowserFormFault::OptionPending && !waited =>
                    {
                        waited = true;
                        let wake = (Instant::now() + Duration::from_millis(400))
                            .min(deadline)
                            .min(state.native.deadline);
                        tokio::select! {
                            biased;
                            event = state.native.next_event(worker, browser) => {
                                state.native.retain(event?)?;
                                return Err(AgentWorkFailure::Mailbox);
                            }
                            () = tokio::time::sleep_until(tokio::time::Instant::from_std(wake)) => {}
                        }
                        current = Box::pin(Self::observe(state, worker, browser)).await?;
                        if state.task_progress(&current)?
                            == AgentWorkTaskProgress::ReadyForExtraction
                        {
                            return Ok(PreparedForm::Ready(Box::new(current)));
                        }
                        continue;
                    }
                    Err(error) => {
                        failure = Some(error);
                        break 'fields;
                    }
                };
                let selected = binding.is_choice(index)
                    && match recipe.intent() {
                        SemanticActionIntent::Select { .. } => true,
                        SemanticActionIntent::Click { target } => current
                            .frames()
                            .iter()
                            .flat_map(|frame| frame.nodes())
                            .any(|node| {
                                node.reference() == *target && node.role() == SemanticRole::Option
                            }),
                        _ => false,
                    };
                let next = Box::pin(Self::form_primitive(
                    state, worker, browser, &current, recipe, false,
                ))
                .await?;
                let next = match next {
                    Ok(next) => next,
                    Err(fault) => {
                        failure = Some(AgentBrowserFormError {
                            field: Some((index + 1) as u8),
                            fault,
                        });
                        break 'fields;
                    }
                };
                let committed = selected && binding.choice_collapsed(index, &current, &next);
                current = next;
                if state.task_progress(&current)? == AgentWorkTaskProgress::ReadyForExtraction {
                    return Ok(PreparedForm::Ready(Box::new(current)));
                }
                actions += 1;
                if committed {
                    committed_choices[index] = true;
                }
            }
        }
        // Dependent widgets can clear an earlier field. Recheck every value
        // before submission; successful earlier typing is not enough.
        if failure.is_none() {
            for index in 0..binding.field_count() {
                if !(committed_choices[index] && binding.value_matches(index, &current))
                    && !matches!(binding.field_action(index, &current), Ok(None))
                {
                    failure = Some(AgentBrowserFormError {
                        field: Some((index + 1) as u8),
                        fault: AgentBrowserFormFault::Stale,
                    });
                    break;
                }
            }
        }
        let mut submitted = false;
        if failure.is_none() && (actions >= MAX_FORM_ACTIONS || Instant::now() >= deadline) {
            failure = Some(AgentBrowserFormError {
                field: None,
                fault: AgentBrowserFormFault::Limit,
            });
        }
        if failure.is_none() {
            match binding.submit_reference(&current) {
                Ok(Some(target)) => {
                    let recipe = SemanticActionProposal::try_new(
                        SemanticActionIntent::Click { target },
                        SemanticEffectClass::Read,
                        SemanticWaitCondition::Immediate,
                        SemanticVerification::PageChanged,
                        SemanticSettleBudget::try_new(MIN_AGENT_BROWSER_SNAPSHOT_SETTLE_MILLIS)
                            .map_err(|_| AgentWorkFailure::Contract)?,
                    )
                    .map_err(|_| AgentWorkFailure::Contract)?;
                    match Box::pin(Self::form_primitive(
                        state, worker, browser, &current, recipe, true,
                    ))
                    .await?
                    {
                        Ok(_) => submitted = true,
                        Err(fault) => failure = Some(AgentBrowserFormError { field: None, fault }),
                    }
                    if let Some(target) = state
                        .follow
                        .take()
                        .and_then(|target| Self::followable(state, target))
                    {
                        return Ok(PreparedForm::Follow(Box::new((continuation, target))));
                    }
                }
                Ok(None) => {}
                Err(error) => failure = Some(error),
            }
        }
        continuation
            .into_form_refresh(original, &config, completed, submitted, failure)
            .map(PreparedForm::Refresh)
            .map_err(|_| AgentWorkFailure::Contract)
    }

    async fn form_primitive(
        state: &mut WorkState,
        worker: &mut AgentRuntimeWorker,
        browser: &WorkBrowser<'_>,
        observation: &SemanticObservation,
        mut recipe: SemanticActionProposal,
        read_only: bool,
    ) -> Result<Result<SemanticObservation, AgentBrowserFormFault>, AgentWorkFailure> {
        state.native.check_control(worker, browser)?;
        state.refresh_account(worker, browser)?;
        let frames = observation
            .frames()
            .iter()
            .map(|frame| frame.frame().clone())
            .collect::<Vec<_>>();
        let session = state.session.as_mut().ok_or(AgentWorkFailure::Contract)?;
        session
            .policy
            .admit_owned_observation(observation, session.account)
            .map_err(|_| AgentWorkFailure::Contract)?;
        let batch =
            SemanticActionBatchId::new(session.next_action).ok_or(AgentWorkFailure::Contract)?;
        let mut admitted = None;
        for _ in 0..2 {
            let Ok(proposal) = crate::AgentBrowserActionProposal::bind_code_owned(
                recipe.clone(),
                observation,
                &frames,
                batch,
            ) else {
                return Ok(Err(AgentBrowserFormFault::Target));
            };
            match state.task.assess_observed(proposal.action(), observation) {
                Ok(assessment) => {
                    if let Err(fault) = compound_effect(assessment.actual_effect(), read_only) {
                        return Ok(Err(fault));
                    }
                    if !state
                        .action_authority(observation)?
                        .permits_action(observation, &recipe)
                    {
                        return Ok(Err(AgentBrowserFormFault::Target));
                    }
                    admitted = Some((proposal, assessment));
                    break;
                }
                Err(AgentWorkFailure::EffectRequired(effect))
                    if (!read_only || effect == SemanticEffectClass::Read)
                        && matches!(
                            effect,
                            SemanticEffectClass::Read | SemanticEffectClass::LocalWrite
                        ) =>
                {
                    recipe = SemanticActionProposal::try_new(
                        recipe.intent().clone(),
                        effect,
                        recipe.wait(),
                        recipe.verification(),
                        recipe.settle_budget(),
                    )
                    .map_err(|_| AgentWorkFailure::Contract)?;
                }
                Err(AgentWorkFailure::EffectRequired(_)) => {
                    return Ok(Err(AgentBrowserFormFault::SeparateActionRequired))
                }
                Err(AgentWorkFailure::ActionDenied) => {
                    return Ok(Err(AgentBrowserFormFault::Target))
                }
                Err(error) => return Err(error),
            }
        }
        let Some((proposal, assessment)) = admitted else {
            return Ok(Err(AgentBrowserFormFault::Target));
        };
        state
            .journal_mut()?
            .emit(AgentWorkEventKind::ToolProposed(AgentBrowserToolKind::Act))?;
        let result = Self::execute_prepared_action(
            state,
            worker,
            browser,
            proposal,
            assessment,
            observation,
        )
        .await;
        let (current, _, transition) = match result {
            Ok(result) => result,
            Err(AgentWorkFailure::Browser(error))
                if read_only
                    && matches!(
                        error,
                        AgentBrowserProviderError::Action(
                            crate::AgentBrowserActionError::Verification(_)
                                | crate::AgentBrowserActionError::Failed(_)
                        ) | AgentBrowserProviderError::ActionRejected(_)
                            | AgentBrowserProviderError::ActionUnverified
                    ) =>
            {
                let session = state.session.as_mut().ok_or(AgentWorkFailure::Contract)?;
                let _ = session.take_rejected_refusal();
                if session
                    .close_failed_owned_action()
                    .map_err(AgentWorkFailure::Browser)?
                {
                    return Ok(Err(AgentBrowserFormFault::Unverified));
                }
                return Err(AgentWorkFailure::Browser(error));
            }
            Err(error) => return Err(error),
        };
        if let Some(batch) = transition.batch_result() {
            state.task.accept_verified_action(batch, &current)?;
        }
        Ok(Ok(current))
    }
}

fn compound_effect(
    effect: SemanticEffectClass,
    read_only: bool,
) -> Result<(), AgentBrowserFormFault> {
    match effect {
        SemanticEffectClass::Read => Ok(()),
        SemanticEffectClass::LocalWrite if !read_only => Ok(()),
        _ => Err(AgentBrowserFormFault::SeparateActionRequired),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn compound_submission_never_widens_an_effect_or_hides_why_it_stopped() {
        assert_eq!(compound_effect(SemanticEffectClass::Read, true), Ok(()));
        assert_eq!(
            compound_effect(SemanticEffectClass::LocalWrite, false),
            Ok(())
        );
        assert_eq!(
            compound_effect(SemanticEffectClass::LocalWrite, true),
            Err(AgentBrowserFormFault::SeparateActionRequired)
        );
    }
    #[test]
    fn every_consequential_effect_requires_its_separate_admission() {
        for effect in [
            SemanticEffectClass::ExternalWrite,
            SemanticEffectClass::Communication,
            SemanticEffectClass::Purchase,
            SemanticEffectClass::Destructive,
            SemanticEffectClass::CapabilityBoundary,
        ] {
            for read_only in [false, true] {
                assert_eq!(
                    compound_effect(effect, read_only),
                    Err(AgentBrowserFormFault::SeparateActionRequired)
                );
            }
        }
    }
    fn look(nodes: serde_json::Value) -> SemanticObservation {
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
                ContextCapabilities::try_new(
                    ContextKind::Owned,
                    &[ContextCapability::Observe, ContextCapability::Act],
                )
                .unwrap(),
            )
            .unwrap();
        let operation = registry
            .begin_context(identity.id(), ContextOperationId::new(1).unwrap())
            .unwrap();
        registry
            .settle_construction(identity.id(), operation, ContextSettlement::Applied)
            .unwrap();
        let context = registry.join(identity.id()).unwrap();
        let frame = SemanticFrameJoin::try_new(
            context,
            FrameId::MAIN,
            context.frame_generation(),
            SemanticOrigin::parse("https://forms.example.test/").unwrap(),
            SemanticFrameTrust::SameOrigin,
        )
        .unwrap();
        let snapshot = decode_semantic_snapshot(
            SemanticDecodeContext::new(
                SemanticInvocationId::new(1).unwrap(),
                frame,
                SemanticSnapshotGeneration::new(1).unwrap(),
            ),
            &serde_json::to_vec(
                &json!({"v":SEMANTIC_WIRE_VERSION,"i":1,"g":1,"c":"complete","n":nodes}),
            )
            .unwrap(),
        )
        .unwrap();
        SemanticObservationAssembler::new(
            SemanticObservationRequest::initial(
                SemanticObservationId::new(1).unwrap(),
                context,
                SemanticObservationBudget::INITIAL_FILTERED,
            ),
            snapshot,
        )
        .unwrap()
        .finish()
        .unwrap()
    }
    #[test]
    fn batch_offer_is_limited_to_one_same_origin_get_search_form() {
        let nodes = json!([
            {"k":1,"r":"document"}, {"k":2,"p":0,"r":"landmark","lm":"search"},
            {"k":3,"p":1,"r":"textbox","n":"From","o":2,"ff":13},
            {"k":4,"p":1,"r":"textbox","n":"To","o":2,"ff":13}
        ]);
        let fill = SemanticActionProposal::try_new(
            SemanticActionIntent::Fill {
                target: SemanticReferenceId::parse("@a3").unwrap(),
                value: SemanticActionText::try_new("Northport".into()).unwrap(),
            },
            SemanticEffectClass::LocalWrite,
            SemanticWaitCondition::Immediate,
            SemanticVerification::TargetValueMatchesInput,
            SemanticSettleBudget::try_new(2000).unwrap(),
        )
        .unwrap();
        assert!(AgentWorkController::is_batchable_search_fill(
            &fill,
            &look(nodes.clone())
        ));
        let mut single = nodes.clone();
        single.as_array_mut().unwrap().pop();
        assert!(!AgentWorkController::is_batchable_search_fill(
            &fill,
            &look(single)
        ));
        let mut post = nodes.clone();
        post[2]["ff"] = json!(14);
        assert!(!AgentWorkController::is_batchable_search_fill(
            &fill,
            &look(post)
        ));
        let mut unrelated = nodes.clone();
        unrelated[3]["p"] = json!(0);
        assert!(!AgentWorkController::is_batchable_search_fill(
            &fill,
            &look(unrelated)
        ));
        let mut ordinary = nodes;
        ordinary[1]["lm"] = json!("main");
        assert!(!AgentWorkController::is_batchable_search_fill(
            &fill,
            &look(ordinary)
        ));
    }
}
