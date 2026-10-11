//! One explicit approval of the fixed disposable-account fixture. The offer is
//! process-local and never a model tool, durable grant, or run-wide permission.
use zephium_core::work::{runtime::*, WorkExecutionId, WorkStepId};

pub(super) const MESSAGE: &str = "Zephium Work qualification: approved send and extraction test.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Decision {
    pub offer: u32,
    pub approve: bool,
}

pub(super) fn command(bytes: &[u8]) -> Option<Decision> {
    let text = std::str::from_utf8(bytes).ok()?;
    let (verb, number) = text.split_once(' ')?;
    let approve = match verb {
        "approve" => true,
        "decline" => false,
        _ => return None,
    };
    if number.is_empty()
        || number.len() > 10
        || number.starts_with('0')
        || !number.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some(Decision {
        offer: number.parse().ok()?,
        approve,
    })
}

#[derive(Default)]
pub(super) struct Gate {
    next: u32,
    used: bool,
    pending: Option<(u32, WorkExecutionId, WorkStepId, WorkConfirmV1)>,
}

impl Gate {
    pub(super) fn eligible(confirm: &WorkConfirmV1) -> bool {
        confirm.site == "app.slack.com"
            && confirm.category == WorkConfirmCategoryV1::Communication
            && confirm.text.as_deref() == Some(MESSAGE)
            && confirm.page.is_some()
            && confirm.provenance.is_empty()
            && !confirm.run_option
            && confirm.decision.is_none()
    }

    /// A changed or replaced confirmation invalidates the old offer even when
    /// its human-readable text happens to match.
    pub(super) fn offer(
        &mut self,
        execution: WorkExecutionId,
        step: WorkStepId,
        confirm: &WorkConfirmV1,
    ) -> Option<(u32, bool)> {
        if self.used || !Self::eligible(confirm) {
            self.pending = None;
            return None;
        }
        if let Some((offer, old_execution, old_step, old)) = &self.pending {
            if *old_execution == execution && *old_step == step && old == confirm {
                return Some((*offer, false));
            }
        }
        self.pending = None;
        self.next = self.next.checked_add(1)?;
        self.pending = Some((self.next, execution, step, confirm.clone()));
        Some((self.next, true))
    }

    pub(super) fn clear(&mut self) {
        self.pending = None;
    }

    pub(super) fn decide(
        &mut self,
        decision: Decision,
        execution: WorkExecutionId,
        step: WorkStepId,
        confirm: &WorkConfirmV1,
    ) -> Option<bool> {
        let (offer, held_execution, held_step, held) = self.pending.as_ref()?;
        if self.used
            || decision.offer != *offer
            || execution != *held_execution
            || step != *held_step
            || held != confirm
            || !Self::eligible(confirm)
        {
            return None;
        }
        // Never replay after uncertain command acknowledgement.
        self.used = true;
        self.pending = None;
        Some(decision.approve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn confirm() -> WorkConfirmV1 {
        WorkConfirmV1 {
            site: "app.slack.com".into(),
            category: WorkConfirmCategoryV1::Communication,
            headline: "Send the qualification message?".into(),
            action: "press Send".into(),
            text: Some(MESSAGE.into()),
            facts: vec![],
            page: Some(1.into()),
            provenance: vec![],
            run_option: false,
            decision: None,
        }
    }
    #[test]
    fn commands_require_one_exact_offer() {
        assert_eq!(
            command(b"approve 1"),
            Some(Decision {
                offer: 1,
                approve: true
            })
        );
        assert_eq!(
            command(b"decline 2"),
            Some(Decision {
                offer: 2,
                approve: false
            })
        );
        for bytes in [
            "approve",
            "approve 0",
            "approve 01",
            "approve +1",
            "approve 1 ",
            "approve 1\napprove 2",
            "approve 4294967296",
            "allow 1",
            "check 1",
        ] {
            assert!(command(bytes.as_bytes()).is_none(), "{bytes}");
        }
    }
    #[test]
    fn approvals_are_exact_single_use_and_never_cover_other_effects() {
        let mut gate = Gate::default();
        let original = confirm();
        let yes = Decision {
            offer: 1,
            approve: true,
        };
        assert_eq!(gate.decide(yes, 1.into(), 2.into(), &original), None);
        assert_eq!(gate.offer(1.into(), 2.into(), &original), Some((1, true)));
        assert_eq!(gate.offer(1.into(), 2.into(), &original), Some((1, false)));
        assert_eq!(gate.decide(yes, 2.into(), 2.into(), &original), None);
        let mut changed = original.clone();
        changed.facts.push(WorkConfirmFactV1 {
            label: "Channel".into(),
            value: "other".into(),
        });
        assert_eq!(gate.decide(yes, 1.into(), 2.into(), &changed), None);
        assert_eq!(gate.offer(1.into(), 2.into(), &changed), Some((2, true)));
        assert_eq!(gate.decide(yes, 1.into(), 2.into(), &changed), None);
        assert_eq!(
            gate.decide(
                Decision {
                    offer: 2,
                    approve: true
                },
                1.into(),
                2.into(),
                &changed
            ),
            Some(true)
        );
        assert_eq!(gate.offer(1.into(), 3.into(), &original), None);
        for category in [
            WorkConfirmCategoryV1::Purchase,
            WorkConfirmCategoryV1::Destructive,
            WorkConfirmCategoryV1::Save,
            WorkConfirmCategoryV1::Edit,
            WorkConfirmCategoryV1::Type,
        ] {
            let mut c = original.clone();
            c.category = category;
            assert!(!Gate::eligible(&c));
        }
        let mut c = original.clone();
        c.text = Some(format!("{MESSAGE} extra"));
        assert!(!Gate::eligible(&c));
        c = original.clone();
        c.site = "slack.com.evil.test".into();
        assert!(!Gate::eligible(&c));
        c = original;
        c.page = None;
        assert!(!Gate::eligible(&c));
    }
    #[test]
    fn vanished_offers_and_declines_cannot_be_replayed() {
        let c = confirm();
        let mut gate = Gate::default();
        gate.offer(1.into(), 2.into(), &c);
        gate.clear();
        assert_eq!(
            gate.decide(
                Decision {
                    offer: 1,
                    approve: true
                },
                1.into(),
                2.into(),
                &c
            ),
            None
        );
        assert_eq!(gate.offer(1.into(), 2.into(), &c), Some((2, true)));
        assert_eq!(
            gate.decide(
                Decision {
                    offer: 2,
                    approve: false
                },
                1.into(),
                2.into(),
                &c
            ),
            Some(false)
        );
        assert_eq!(gate.offer(1.into(), 2.into(), &c), None);
    }
}
