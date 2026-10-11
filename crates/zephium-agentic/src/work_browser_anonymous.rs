//! Original anonymous read intent; never a signed-out storage or pixel grant.
use crate::{AgentAccountScope, AgentPlanLeaseBinding, AgentPolicyInstant, AgentRunManifest,
    ContextIdentity, ContextKind, ContextRunId, ContextNavigationTarget, SemanticOrigin};
use std::{fmt, sync::{Arc, atomic::{AtomicU8, Ordering}}};

/// Closed status of the original task proof. No account/page data is exposed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkBrowserAnonymousReadStatus {
    /// No proof was issued or bound to the original resource.
    Absent,
    /// Original intent remains current; this is not cookie or pixel permission.
    Current,
    /// The page was handed to a person; subsequent acquisition cannot revive it.
    HumanHandoff,
    /// A different actor run attempted to acquire the original page.
    SuccessorRun,
    /// The original approval or active deadline elapsed.
    Expired,
    /// The owning resource was cancelled, quarantined, destroyed or dropped.
    Closed,
}

/// Opaque one-resource original anonymous read proof. Clones share revocation.
/// No serialization or constructor from booleans/IDs can mint this proof.
#[derive(Clone)]
pub struct WorkBrowserAnonymousReadAuthority(Arc<OriginalRead>);
struct OriginalRead {
    identity: ContextIdentity,
    document: ContextNavigationTarget,
    expires: AgentPolicyInstant,
    deadline: std::time::Instant,
    status: AtomicU8,
}
impl fmt::Debug for WorkBrowserAnonymousReadAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WorkBrowserAnonymousReadAuthority([redacted])")
    }
}
impl WorkBrowserAnonymousReadAuthority {
    /// Trusted original read intent is supplied before dispatch. Both original
    /// manifest and exact node independently restrict account scope; storage
    /// class, sensitivity, cookies, and a goal's prose cannot establish this.
    pub fn from_original_read(
        manifest: &AgentRunManifest,
        lease: AgentPlanLeaseBinding,
        identity: ContextIdentity,
        document: &ContextNavigationTarget,
        now: AgentPolicyInstant,
        deadline: std::time::Instant,
    ) -> Option<Self> {
        let node = manifest.plan_node(lease.node())?;
        let origin = SemanticOrigin::parse(document.as_url().as_str()).ok()?;
        let anonymous = [AgentAccountScope::Anonymous];
        if manifest.scope().accounts() != anonymous || node.accounts() != anonymous
            || identity.kind() != ContextKind::Owned || manifest.run() != identity.owner()
            || !manifest.scope().effects().contains(crate::SemanticEffectClass::Read)
            || !node.effects().contains(crate::SemanticEffectClass::Read)
            || !manifest.scope().profiles().contains(&identity.profile())
            || !node.profiles().contains(&identity.profile())
            || !manifest.scope().origins().contains(&origin) || !node.origins().contains(&origin)
            || now < manifest.issued_at() || now >= manifest.expires_at()
            || now >= node.expires_at() || deadline <= std::time::Instant::now()
        { return None; }
        Some(Self(Arc::new(OriginalRead {
            identity, document: document.clone(), expires: node.expires_at().min(manifest.expires_at()), deadline,
            status: AtomicU8::new(0),
        })))
    }
    /// Read-only current status. It is only a prerequisite for separately
    /// admitted operations; it never proves native storage or account state.
    pub fn status(&self, now: AgentPolicyInstant) -> WorkBrowserAnonymousReadStatus {
        if now >= self.0.expires || std::time::Instant::now() >= self.0.deadline {
            self.revoke(WorkBrowserAnonymousReadStatus::Expired);
        }
        match self.0.status.load(Ordering::Acquire) {
            1 => WorkBrowserAnonymousReadStatus::Current,
            2 => WorkBrowserAnonymousReadStatus::HumanHandoff,
            3 => WorkBrowserAnonymousReadStatus::SuccessorRun,
            4 => WorkBrowserAnonymousReadStatus::Expired,
            5 => WorkBrowserAnonymousReadStatus::Closed,
            _ => WorkBrowserAnonymousReadStatus::Absent,
        }
    }
    pub(crate) fn claim(&self, identity: ContextIdentity, now: AgentPolicyInstant) -> bool {
        self.0.identity == identity && self.status(now) == WorkBrowserAnonymousReadStatus::Absent
            && self.0.status.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire).is_ok()
    }
    pub(crate) fn claim_resource(&self, context: crate::ContextId, profile: zephium_core::ids::ProfileId, document: &ContextNavigationTarget, now: AgentPolicyInstant) -> bool {
        self.0.identity.id() == context && self.0.identity.profile() == profile
            && self.0.document == *document && self.claim(self.0.identity, now)
    }
    pub(crate) fn acquire(&self, run: ContextRunId, now: AgentPolicyInstant) -> bool {
        if run != self.0.identity.owner() { self.revoke(WorkBrowserAnonymousReadStatus::SuccessorRun); }
        self.status(now) == WorkBrowserAnonymousReadStatus::Current
    }
    pub(crate) fn bound_deadline(&self, now: AgentPolicyInstant, requested: AgentPolicyInstant) -> Option<AgentPolicyInstant> {
        self.bound_deadline_at(now,requested,std::time::Instant::now())
    }
    fn bound_deadline_at(&self, now: AgentPolicyInstant, requested: AgentPolicyInstant, wall_now: std::time::Instant) -> Option<AgentPolicyInstant> {
        if self.status(now) != WorkBrowserAnonymousReadStatus::Current { return None; }
        let remaining=self.0.deadline.checked_duration_since(wall_now)?;
        let remaining=u64::try_from(remaining.as_millis()).ok()?;
        let wall_bound=AgentPolicyInstant::from_millis(now.millis().checked_add(remaining)?);
        let bound=requested.min(self.0.expires).min(wall_bound);
        (now<bound).then_some(bound)
    }
    pub(crate) fn matches(&self, identity: ContextIdentity, now: AgentPolicyInstant) -> bool {
        self.0.identity == identity && self.status(now) == WorkBrowserAnonymousReadStatus::Current
    }
    pub(crate) fn revoke(&self, reason: WorkBrowserAnonymousReadStatus) {
        let value = match reason {
            WorkBrowserAnonymousReadStatus::HumanHandoff => 2,
            WorkBrowserAnonymousReadStatus::SuccessorRun => 3,
            WorkBrowserAnonymousReadStatus::Expired => 4,
            _ => 5,
        };
        // Preserve the first closed loss reason, including after expiry/cleanup.
        let _ = self.0.status.fetch_update(Ordering::AcqRel, Ordering::Acquire,
            |current| (current <= 1).then_some(value));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{AgentAccountId, AgentEffectScope, AgentPlanLeaseId, AgentPlanNodeAuthority,
        AgentPlanNodeId, AgentPlanNodeScope, AgentRunBudget, AgentRunManifestId, AgentRunScope,
        ContextId, SemanticEffectClass, SemanticOrigin, SemanticSensitivity};
    use zephium_core::ids::ProfileId;
    fn now(value: u64) -> AgentPolicyInstant { AgentPolicyInstant::from_millis(value) }
    fn identity() -> ContextIdentity { ContextIdentity::new(ContextId::parse("00000000000000000000000003").unwrap(),
        ContextRunId::parse("00000000000000000000000002").unwrap(), ProfileId::from(1), ContextKind::Owned) }
    fn lease() -> AgentPlanLeaseBinding { AgentPlanLeaseBinding::new(AgentPlanLeaseId::generate(),AgentPlanNodeId::parse("00000000000000000000000004").unwrap()) }
    fn manifest(accounts: Vec<AgentAccountScope>, node_accounts: Vec<AgentAccountScope>, sensitivity: SemanticSensitivity) -> AgentRunManifest {
        manifest_effect(accounts,node_accounts,sensitivity,SemanticEffectClass::Read)
    }
    fn manifest_effect(accounts: Vec<AgentAccountScope>, node_accounts: Vec<AgentAccountScope>, sensitivity: SemanticSensitivity, effect: SemanticEffectClass) -> AgentRunManifest {
        let effects=AgentEffectScope::try_new(&[effect]).unwrap();
        let budget=AgentRunBudget::try_new(10,1000,1000,1).unwrap();
        let origin=SemanticOrigin::parse("https://example.test/").unwrap();
        let scope=AgentRunScope::try_new(vec![ProfileId::from(1)],accounts,vec![origin.clone()],sensitivity,effects,vec![]).unwrap();
        let authority=AgentPlanNodeAuthority::try_new(vec![ProfileId::from(1)],node_accounts,vec![origin],sensitivity,effects).unwrap();
        AgentRunManifest::try_new(AgentRunManifestId::parse("00000000000000000000000001").unwrap(),ContextRunId::parse("00000000000000000000000002").unwrap(),scope,budget,now(1),now(100),
            vec![AgentPlanNodeScope::new(AgentPlanNodeId::parse("00000000000000000000000004").unwrap(),authority,budget,now(90))]).unwrap()
    }
    pub(crate) fn proof() -> WorkBrowserAnonymousReadAuthority {
        WorkBrowserAnonymousReadAuthority::from_original_read(&manifest(vec![AgentAccountScope::Anonymous],vec![AgentAccountScope::Anonymous],SemanticSensitivity::Public),lease(),identity(),&ContextNavigationTarget::parse("https://example.test/").unwrap(),now(2),std::time::Instant::now()+std::time::Duration::from_secs(60)).unwrap()
    }
    #[test]
    fn construction_requires_both_original_anonymous_scopes_and_exact_identity() {
        let identified=AgentAccountScope::Authenticated(AgentAccountId::parse("00000000000000000000000008").unwrap());
        for (accounts,node_accounts,expected) in [
            (vec![AgentAccountScope::Anonymous],vec![AgentAccountScope::Anonymous],true),
            (vec![identified],vec![identified],false),
            (vec![identified,AgentAccountScope::Anonymous],vec![AgentAccountScope::Anonymous],false),
            (vec![identified,AgentAccountScope::Anonymous],vec![identified],false),
        ] {
            for sensitivity in [SemanticSensitivity::Public,SemanticSensitivity::Sensitive] {
                let m=manifest(accounts.clone(),node_accounts.clone(),sensitivity);
                let admit=|id,at| WorkBrowserAnonymousReadAuthority::from_original_read(&m,lease(),id,&ContextNavigationTarget::parse("https://example.test/").unwrap(),at,std::time::Instant::now()+std::time::Duration::from_secs(60));
                assert_eq!(admit(identity(),now(2)).is_some(),expected);
                assert!(admit(ContextIdentity::new(ContextId::parse("00000000000000000000000003").unwrap(),ContextRunId::parse("00000000000000000000000009").unwrap(),ProfileId::from(1),ContextKind::Owned),now(2)).is_none());
                assert!(admit(ContextIdentity::new(ContextId::parse("00000000000000000000000003").unwrap(),ContextRunId::parse("00000000000000000000000002").unwrap(),ProfileId::from(9),ContextKind::Owned),now(2)).is_none());
                assert!(admit(identity(),now(0)).is_none());
                assert!(admit(identity(),now(90)).is_none());
            }
        }
    }
    #[test]
    fn one_construction_claim_and_successor_acquisition_cannot_reissue() {
        let p=proof(); let clone=p.clone();
        assert_eq!(p.status(now(2)),WorkBrowserAnonymousReadStatus::Absent);
        assert!(!p.claim(ContextIdentity::new(ContextId::parse("00000000000000000000000009").unwrap(),identity().owner(),identity().profile(),ContextKind::Owned),now(2)));
        assert!(p.claim(identity(),now(2)));
        assert!(!clone.claim(identity(),now(2)));
        assert!(p.acquire(identity().owner(),now(3)));
        assert!(!p.acquire(ContextRunId::parse("00000000000000000000000009").unwrap(),now(4)));
        assert_eq!(clone.status(now(4)),WorkBrowserAnonymousReadStatus::SuccessorRun);
        assert!(!p.acquire(identity().owner(),now(5)));
    }
    #[test]
    fn every_handoff_revokes_all_clones_without_reacquisition_or_expiry_relabel() {
        let p=proof(); assert!(p.claim(identity(),now(2))); let clone=p.clone();
        p.revoke(WorkBrowserAnonymousReadStatus::HumanHandoff);
        assert!(!clone.acquire(identity().owner(),now(3)));
        assert!(!clone.claim(identity(),now(3)));
        assert_eq!(clone.status(now(100)),WorkBrowserAnonymousReadStatus::HumanHandoff);
        p.revoke(WorkBrowserAnonymousReadStatus::Closed);
        assert_eq!(clone.status(now(101)),WorkBrowserAnonymousReadStatus::HumanHandoff);
    }
    #[test]
    fn expiry_cancellation_and_wrong_context_refuse_without_storage_inference() {
        for reason in [WorkBrowserAnonymousReadStatus::Expired,WorkBrowserAnonymousReadStatus::Closed] {
            let p=proof();assert!(p.claim(identity(),now(2)));let clone=p.clone();
            if reason==WorkBrowserAnonymousReadStatus::Expired {assert_eq!(p.status(now(90)),reason)} else {p.revoke(reason)}
            assert_eq!(clone.status(now(3)),reason);
            assert!(!clone.matches(identity(),now(3)));
        }
        let p=proof();assert!(p.claim(identity(),now(2)));
        assert!(!p.matches(ContextIdentity::new(ContextId::parse("00000000000000000000000009").unwrap(),identity().owner(),identity().profile(),ContextKind::Owned),now(3)));
        assert!(p.matches(identity(),now(3)));
        // Storage class and cookie contents are intentionally absent: this is
        // original task authority, never a signed-out native session witness.
    }
    #[test]
    fn write_only_manifest_and_expired_wall_deadline_cannot_issue_original_read() {
        let m=manifest_effect(vec![AgentAccountScope::Anonymous],vec![AgentAccountScope::Anonymous],SemanticSensitivity::Public,SemanticEffectClass::LocalWrite);
        let target=ContextNavigationTarget::parse("https://example.test/").unwrap();
        assert!(WorkBrowserAnonymousReadAuthority::from_original_read(&m,lease(),identity(),&target,now(2),std::time::Instant::now()+std::time::Duration::from_secs(1)).is_none());
        let m=manifest(vec![AgentAccountScope::Anonymous],vec![AgentAccountScope::Anonymous],SemanticSensitivity::Public);
        assert!(WorkBrowserAnonymousReadAuthority::from_original_read(&m,lease(),identity(),&target,now(2),std::time::Instant::now()-std::time::Duration::from_secs(1)).is_none());
        assert!(WorkBrowserAnonymousReadAuthority::from_original_read(&m,lease(),identity(),&ContextNavigationTarget::parse("https://other.test/").unwrap(),now(2),std::time::Instant::now()+std::time::Duration::from_secs(1)).is_none());
    }

    #[test]
    fn near_wall_expiry_cannot_admit_a_new_eight_second_media_window() {
        let p=proof();assert!(p.claim(identity(),now(2)));
        let one_millisecond_before=p.0.deadline-std::time::Duration::from_millis(1);
        assert_eq!(p.bound_deadline_at(now(3),now(8003),one_millisecond_before),Some(now(4)));
        assert_eq!(p.bound_deadline_at(now(3),now(8003),p.0.deadline),None);
        assert_eq!(p.bound_deadline_at(now(3),now(8003),p.0.deadline+std::time::Duration::from_millis(1)),None);
        assert_eq!(p.bound_deadline_at(now(3),now(2),one_millisecond_before),None);
        p.revoke(WorkBrowserAnonymousReadStatus::HumanHandoff);
        assert_eq!(p.bound_deadline_at(now(3),now(80),one_millisecond_before),None);
    }

}
