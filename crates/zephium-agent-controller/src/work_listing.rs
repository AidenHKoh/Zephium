//! Bounded mechanical loading of an already observed collection.

use super::*;
use std::collections::BTreeSet;

const MAX_LOADING_STEPS: usize = 6;
const MAX_LOADING_MILLIS: u64 = 8_000;

#[derive(Default)]
struct ListingProgress {
    identities: BTreeSet<[u8; 32]>,
    unchanged: u8,
}

impl ListingProgress {
    fn observe(&mut self, census: &NativeListingCensus, requested: usize) -> bool {
        let previous = self.identities.len();
        self.identities.extend(census.identities.iter().copied());
        self.unchanged = if self.identities.len() == previous {
            self.unchanged.saturating_add(1)
        } else {
            0
        };
        !census.limited
            && !census.identities.is_empty()
            && self.identities.len() < requested
            && self.unchanged < 2
    }
}

impl AgentWorkController {
    pub(super) async fn load_listing(
        state: &mut WorkState,
        worker: &mut AgentRuntimeWorker,
        browser: &WorkBrowser<'_>,
        mut observation: SemanticObservation,
    ) -> Result<SemanticObservation, AgentWorkFailure> {
        let Some(requested) = state
            .extraction_schema
            .as_ref()
            .filter(|schema| schema.is_row_collection())
            .and_then(|schema| schema.fields().first())
            .and_then(SemanticExtractionFieldSchema::max_list_items)
            .filter(|count| *count > 1)
        else {
            return Ok(observation);
        };
        // Historical observations must remain available to the one later mapping.
        // A task without that capability keeps its ordinary observation path.
        if !state.actions_before_extraction
            || !(state.progressive_observation || state.navigation_discovery.is_some())
        {
            return Ok(observation);
        }
        let started = Instant::now();
        if observation
            .frames()
            .first()
            .and_then(|frame| frame.nodes().first())
            .is_some_and(|node| node.role() != SemanticRole::Document)
        {
            state.native.check_control(worker, browser)?;
            state.refresh_account(worker, browser)?;
            observation = Box::pin(Self::observe(state, worker, browser)).await?;
        }
        let mut progress = ListingProgress::default();
        for _ in 0..MAX_LOADING_STEPS {
            state.check_task_contract()?;
            state.native.check_control(worker, browser)?;
            let session = state.session.as_ref().ok_or(AgentWorkFailure::Contract)?;
            if started.elapsed() >= Duration::from_millis(MAX_LOADING_MILLIS)
                || session.next_action > session.max_actions
                || session.turns.saturating_add(1) >= session.max_model_calls
                || state.task.human_wall(&observation).is_some()
                || looks_like_human_challenge(&observation)
                || observation
                    .frames()
                    .iter()
                    .flat_map(|frame| frame.nodes())
                    .any(|node| node.sensitivity() != SemanticSensitivity::Public)
            {
                break;
            }
            let census = native_listing_census(&observation);
            if !progress.observe(&census, requested) {
                break;
            }
            let (intent, verification) = match census.continuation {
                Some(
                    NativeListingContinuation::LoadMore(target)
                    | NativeListingContinuation::NextPage(target),
                ) => (
                    SemanticActionIntent::Click { target },
                    SemanticVerification::PageChanged,
                ),
                None => {
                    let Some(target) = census.scroll else { break };
                    (
                        SemanticActionIntent::Scroll {
                            target,
                            direction: SemanticScrollDirection::Down,
                            amount: SemanticScrollAmount::Page,
                        },
                        SemanticVerification::ScrollPositionChanged,
                    )
                }
            };
            let recipe = SemanticActionProposal::try_new(
                intent,
                SemanticEffectClass::Read,
                SemanticWaitCondition::MutationQuiet(
                    SemanticMutationQuietPeriod::try_new(250)
                        .map_err(|_| AgentWorkFailure::Contract)?,
                ),
                verification,
                SemanticSettleBudget::try_new(MIN_AGENT_BROWSER_SNAPSHOT_SETTLE_MILLIS)
                    .map_err(|_| AgentWorkFailure::Contract)?,
            )
            .map_err(|_| AgentWorkFailure::Contract)?;
            let Some(next) = Box::pin(Self::code_owned_read(
                state,
                worker,
                browser,
                &observation,
                recipe,
            ))
            .await?
            else {
                break;
            };
            observation = next;
        }
        Ok(observation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn census(ids: &[u8]) -> NativeListingCensus {
        NativeListingCensus {
            identities: ids.iter().map(|id| [*id; 32]).collect(),
            scroll: None,
            continuation: None,
            limited: false,
        }
    }

    #[test]
    fn virtualized_records_count_once_across_looks_and_loading_stops_at_the_bound() {
        let mut progress = ListingProgress::default();
        assert!(progress.observe(&census(&[1, 2]), 4));
        assert!(progress.observe(&census(&[2, 3]), 4));
        assert!(!progress.observe(&census(&[3, 4]), 4));
        assert_eq!(progress.identities.len(), 4);
    }

    #[test]
    fn repeated_candidates_stop_loading_without_claiming_the_requested_count() {
        let mut progress = ListingProgress::default();
        assert!(progress.observe(&census(&[1, 2]), 3));
        assert!(progress.observe(&census(&[1, 2]), 3));
        assert!(!progress.observe(&census(&[2, 1]), 3));
        assert_eq!(progress.identities.len(), 2);
    }

    #[test]
    fn no_collection_or_structural_limit_cannot_trigger_loading() {
        assert!(!ListingProgress::default().observe(&census(&[]), 3));
        let mut limited = census(&[1, 2]);
        limited.limited = true;
        assert!(!ListingProgress::default().observe(&limited, 3));
    }
}
