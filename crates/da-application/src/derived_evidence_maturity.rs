//! Closed, derived-only evidence maturity vocabulary.
//!
//! This module is intentionally metadata-only. It does not participate in the
//! parse workflow, policy decisions, graph writes, promotion, or import.

use serde::{Deserialize, Serialize};

/// One of the five evidence families supported by the derived protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FamilyId {
    Segmentation,
    Header,
    Bibliography,
    Body,
    Visual,
}

impl FamilyId {
    pub const ALL: [Self; 5] = [
        Self::Segmentation,
        Self::Header,
        Self::Bibliography,
        Self::Body,
        Self::Visual,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Segmentation => "segmentation",
            Self::Header => "header",
            Self::Bibliography => "bibliography",
            Self::Body => "body",
            Self::Visual => "visual",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "segmentation" => Some(Self::Segmentation),
            "header" => Some(Self::Header),
            "bibliography" => Some(Self::Bibliography),
            "body" => Some(Self::Body),
            "visual" => Some(Self::Visual),
            _ => None,
        }
    }
}

/// Ordered derived maturity ladder. There is deliberately no promoted state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaturityState {
    InventoryBound,
    OracleBound,
    AtomProven,
    SequenceProven,
    ShadowObserved,
    ReviewReady,
    HumanGateRequired,
}

impl MaturityState {
    pub const ALL: [Self; 7] = [
        Self::InventoryBound,
        Self::OracleBound,
        Self::AtomProven,
        Self::SequenceProven,
        Self::ShadowObserved,
        Self::ReviewReady,
        Self::HumanGateRequired,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InventoryBound => "inventory_bound",
            Self::OracleBound => "oracle_bound",
            Self::AtomProven => "atom_proven",
            Self::SequenceProven => "sequence_proven",
            Self::ShadowObserved => "shadow_observed",
            Self::ReviewReady => "review_ready",
            Self::HumanGateRequired => "human_gate_required",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "inventory_bound" => Some(Self::InventoryBound),
            "oracle_bound" => Some(Self::OracleBound),
            "atom_proven" => Some(Self::AtomProven),
            "sequence_proven" => Some(Self::SequenceProven),
            "shadow_observed" => Some(Self::ShadowObserved),
            "review_ready" => Some(Self::ReviewReady),
            "human_gate_required" => Some(Self::HumanGateRequired),
            _ => None,
        }
    }

    pub const fn ladder_index(self) -> usize {
        match self {
            Self::InventoryBound => 0,
            Self::OracleBound => 1,
            Self::AtomProven => 2,
            Self::SequenceProven => 3,
            Self::ShadowObserved => 4,
            Self::ReviewReady => 5,
            Self::HumanGateRequired => 6,
        }
    }

    pub const fn successor(self) -> Option<Self> {
        match self {
            Self::InventoryBound => Some(Self::OracleBound),
            Self::OracleBound => Some(Self::AtomProven),
            Self::AtomProven => Some(Self::SequenceProven),
            Self::SequenceProven => Some(Self::ShadowObserved),
            Self::ShadowObserved => Some(Self::ReviewReady),
            Self::ReviewReady => Some(Self::HumanGateRequired),
            Self::HumanGateRequired => None,
        }
    }
}

/// Receipt kind corresponding to one rung of the maturity ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptKind {
    Inventory,
    Oracle,
    Atom,
    Sequence,
    Shadow,
    Review,
}

impl ReceiptKind {
    pub const ALL: [Self; 6] = [
        Self::Inventory,
        Self::Oracle,
        Self::Atom,
        Self::Sequence,
        Self::Shadow,
        Self::Review,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::Oracle => "oracle",
            Self::Atom => "atom",
            Self::Sequence => "sequence",
            Self::Shadow => "shadow",
            Self::Review => "review",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "inventory" => Some(Self::Inventory),
            "oracle" => Some(Self::Oracle),
            "atom" => Some(Self::Atom),
            "sequence" => Some(Self::Sequence),
            "shadow" => Some(Self::Shadow),
            "review" => Some(Self::Review),
            _ => None,
        }
    }
}

/// Typed reasons why a derived state cannot be reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockedReason {
    MissingEvidence,
    LadderGap,
    AttemptedAutomaticPromotion,
    UnknownFamily,
    MalformedReceipt,
    StaleIdentity,
    DenominatorDrift,
    Pooling,
    Contamination,
    SemanticDrift,
    ConservationLoss,
    UnsafeProductionCall,
    MalformedPrompt,
    FailedEvidence,
}

impl BlockedReason {
    pub const ALL: [Self; 14] = [
        Self::MissingEvidence,
        Self::LadderGap,
        Self::AttemptedAutomaticPromotion,
        Self::UnknownFamily,
        Self::MalformedReceipt,
        Self::StaleIdentity,
        Self::DenominatorDrift,
        Self::Pooling,
        Self::Contamination,
        Self::SemanticDrift,
        Self::ConservationLoss,
        Self::UnsafeProductionCall,
        Self::MalformedPrompt,
        Self::FailedEvidence,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingEvidence => "missing_evidence",
            Self::LadderGap => "ladder_gap",
            Self::AttemptedAutomaticPromotion => "attempted_automatic_promotion",
            Self::UnknownFamily => "unknown_family",
            Self::MalformedReceipt => "malformed_receipt",
            Self::StaleIdentity => "stale_identity",
            Self::DenominatorDrift => "denominator_drift",
            Self::Pooling => "pooling",
            Self::Contamination => "contamination",
            Self::SemanticDrift => "semantic_drift",
            Self::ConservationLoss => "conservation_loss",
            Self::UnsafeProductionCall => "unsafe_production_call",
            Self::MalformedPrompt => "malformed_prompt",
            Self::FailedEvidence => "failed_evidence",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_str() == value)
    }
}

/// An identity-only receipt bound to one immutable source and artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundReceipt {
    pub kind: ReceiptKind,
    pub evidence_ref: String,
    pub source_sha256: String,
    pub artifact_sha256: String,
    pub grobid_profile: String,
}

/// Upper bound for every free-form field of a [`FreshnessIdentity`].
///
/// The bound exists because a freshness identity travels into cached artifacts:
/// an unbounded field would let a single oversized local artifact inflate every
/// consumer of the evidence set. Digest fields are fixed-width by validation
/// and need no separate bound.
pub const MAX_FRESHNESS_IDENTITY_FIELD_LEN: usize = 200;

/// Immutable lineage for a derived observation. `observed_at` is provenance,
/// not a wall-clock freshness oracle: freshness is derived only against an
/// explicitly selected target revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreshnessIdentity {
    pub target_revision: String,
    pub observed_revision: String,
    pub source_digest: String,
    pub representation_digest: String,
    pub parser_identity: String,
    pub config_identity: String,
    pub cache_key: String,
    pub observed_at: String,
}

/// The bounded result of comparing an observation with the selected target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessOutcome {
    Fresh,
    Stale,
    Superseded,
}

/// TEI is authoritative for the declared revision. Other sources may only
/// produce a deferred receipt; they cannot silently replace the declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeclaredRevisionTrust {
    TeiDeclared { revision: String },
    DeferredCrossCheck { receipt_ref: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessIdentityError {
    EmptyField,
    FieldTooLong,
    /// `parser_identity` or `config_identity` is not a label or a sha256
    /// fingerprint — for example a raw configuration value, URL or path.
    UnsafeIdentityLabel,
    InvalidDigest,
    TargetRevisionMismatch,
}

impl FreshnessIdentityError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmptyField => "empty_field",
            Self::FieldTooLong => "field_too_long",
            Self::UnsafeIdentityLabel => "unsafe_identity_label",
            Self::InvalidDigest => "invalid_digest",
            Self::TargetRevisionMismatch => "target_revision_mismatch",
        }
    }
}

/// Compare identity with the selected target without consulting the current
/// time. Versioned arXiv revisions use their numeric suffix when available;
/// opaque revision identifiers conservatively treat a mismatch as superseded.
pub fn assess_freshness(
    identity: &FreshnessIdentity,
    selected_target_revision: &str,
) -> Result<FreshnessOutcome, FreshnessIdentityError> {
    for value in [
        identity.target_revision.as_str(),
        identity.observed_revision.as_str(),
        selected_target_revision,
        identity.parser_identity.as_str(),
        identity.config_identity.as_str(),
        identity.cache_key.as_str(),
        identity.observed_at.as_str(),
    ] {
        if value.trim().is_empty() {
            return Err(FreshnessIdentityError::EmptyField);
        }
        if value.len() > MAX_FRESHNESS_IDENTITY_FIELD_LEN {
            return Err(FreshnessIdentityError::FieldTooLong);
        }
    }
    for label in [
        identity.parser_identity.as_str(),
        identity.config_identity.as_str(),
    ] {
        if !valid_identity_label(label) {
            return Err(FreshnessIdentityError::UnsafeIdentityLabel);
        }
    }
    if !valid_digest(&identity.source_digest) || !valid_digest(&identity.representation_digest) {
        return Err(FreshnessIdentityError::InvalidDigest);
    }
    if identity.target_revision != selected_target_revision {
        return Err(FreshnessIdentityError::TargetRevisionMismatch);
    }
    if identity.observed_revision == selected_target_revision {
        return Ok(FreshnessOutcome::Fresh);
    }
    Ok(
        match revision_number(&identity.observed_revision)
            .zip(revision_number(selected_target_revision))
        {
            Some((observed, target)) if observed < target => FreshnessOutcome::Stale,
            _ => FreshnessOutcome::Superseded,
        },
    )
}

fn revision_number(value: &str) -> Option<u32> {
    value.rsplit_once('v')?.1.parse().ok()
}

/// A freshness identity bound into one evidence set, together with the
/// declared-revision basis it was recorded under.
///
/// `identity.target_revision` is a **declaration** — the revision the producer
/// claims the observation belongs to. Recording it is not trusting it: a TEI
/// declaration must name exactly the identity target, any other source may only
/// produce a deferred cross-check receipt, and every read re-checks the
/// declaration ([`FreshnessBinding::validate_declaration`]). There is
/// deliberately no `Verified` declaration state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreshnessBinding {
    pub identity: FreshnessIdentity,
    pub declared_revision: DeclaredRevisionTrust,
}

impl FreshnessBinding {
    /// Record an identity under its declared-revision basis.
    pub fn new(
        identity: FreshnessIdentity,
        declared_revision: DeclaredRevisionTrust,
    ) -> Result<Self, FreshnessIdentityError> {
        let binding = Self {
            identity,
            declared_revision,
        };
        binding.validate_declaration()?;
        Ok(binding)
    }

    /// Re-check the declaration against the identity it accompanies. Callers
    /// that deserialize an artifact must call this instead of assuming the two
    /// fields agree.
    pub fn validate_declaration(&self) -> Result<(), FreshnessIdentityError> {
        match &self.declared_revision {
            DeclaredRevisionTrust::TeiDeclared { revision } => {
                if revision.trim().is_empty() {
                    return Err(FreshnessIdentityError::EmptyField);
                }
                if revision.len() > MAX_FRESHNESS_IDENTITY_FIELD_LEN {
                    return Err(FreshnessIdentityError::FieldTooLong);
                }
                if revision != &self.identity.target_revision {
                    return Err(FreshnessIdentityError::TargetRevisionMismatch);
                }
            }
            DeclaredRevisionTrust::DeferredCrossCheck { receipt_ref } => {
                if receipt_ref.trim().is_empty() {
                    return Err(FreshnessIdentityError::EmptyField);
                }
                if receipt_ref.len() > MAX_FRESHNESS_IDENTITY_FIELD_LEN {
                    return Err(FreshnessIdentityError::FieldTooLong);
                }
            }
        }
        Ok(())
    }
}

/// The bounded, typed currency verdict for one evidence set.
///
/// `Unattributed` exists so that a set carrying no identity cannot be read as
/// fresh: currency is unknown, not asserted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum FreshnessAssessment {
    /// No identity is bound: the set does not say which run produced it.
    Unattributed,
    Fresh {
        observed_revision: String,
    },
    Stale {
        target_revision: String,
        observed_revision: String,
    },
    Superseded {
        target_revision: String,
        observed_revision: String,
    },
    /// The identity declares one target while the caller selected another.
    Mismatched {
        declared_target_revision: String,
        selected_target_revision: String,
    },
    /// The identity or its declaration is unusable; absence is not compliance.
    Malformed {
        error: FreshnessIdentityError,
    },
}

impl FreshnessAssessment {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Unattributed => "unattributed",
            Self::Fresh { .. } => "fresh",
            Self::Stale { .. } => "stale",
            Self::Superseded { .. } => "superseded",
            Self::Mismatched { .. } => "mismatched",
            Self::Malformed { .. } => "malformed",
        }
    }

    pub const fn is_fresh(&self) -> bool {
        matches!(self, Self::Fresh { .. })
    }
}

/// Assess one evidence set against an explicitly selected target revision.
///
/// When `selected_target_revision` is `None` the binding's own declared target
/// is used, which is all a stored set can answer on its own. Neither path
/// consults a clock or a digest: revision ordering decides currency, so two
/// byte-identical digests cannot rescue a superseded input.
pub fn assess_set_freshness(
    set: &FamilyEvidenceSet,
    selected_target_revision: Option<&str>,
) -> FreshnessAssessment {
    let Some(binding) = &set.freshness else {
        return FreshnessAssessment::Unattributed;
    };
    if let Err(error) = binding.validate_declaration() {
        return FreshnessAssessment::Malformed { error };
    }
    let selected = selected_target_revision.unwrap_or(&binding.identity.target_revision);
    match assess_freshness(&binding.identity, selected) {
        Ok(FreshnessOutcome::Fresh) => FreshnessAssessment::Fresh {
            observed_revision: binding.identity.observed_revision.clone(),
        },
        Ok(FreshnessOutcome::Stale) => FreshnessAssessment::Stale {
            target_revision: binding.identity.target_revision.clone(),
            observed_revision: binding.identity.observed_revision.clone(),
        },
        Ok(FreshnessOutcome::Superseded) => FreshnessAssessment::Superseded {
            target_revision: binding.identity.target_revision.clone(),
            observed_revision: binding.identity.observed_revision.clone(),
        },
        Err(FreshnessIdentityError::TargetRevisionMismatch) => FreshnessAssessment::Mismatched {
            declared_target_revision: binding.identity.target_revision.clone(),
            selected_target_revision: selected.to_owned(),
        },
        Err(error) => FreshnessAssessment::Malformed { error },
    }
}

/// The complete, injected receipt set for one paper and one evidence family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyEvidenceSet {
    pub family: FamilyId,
    pub paper_id: String,
    #[serde(default)]
    pub inventory: Option<BoundReceipt>,
    #[serde(default)]
    pub oracle: Option<BoundReceipt>,
    #[serde(default)]
    pub atom: Option<BoundReceipt>,
    #[serde(default)]
    pub sequence: Option<BoundReceipt>,
    #[serde(default)]
    pub shadow: Option<BoundReceipt>,
    #[serde(default)]
    pub review: Option<BoundReceipt>,
    #[serde(default)]
    pub human_decision_ref: Option<String>,
    /// Bounded revision/freshness identity for this set, when the producing
    /// path recorded one. `None` means currency is unknown — never that the
    /// set was measured against the selected target.
    #[serde(default)]
    pub freshness: Option<FreshnessBinding>,
    #[serde(default)]
    pub stale_identity: bool,
    #[serde(default)]
    pub denominator_drift: bool,
    #[serde(default)]
    pub pooling: bool,
    #[serde(default)]
    pub contamination: bool,
    #[serde(default)]
    pub semantic_drift: bool,
    #[serde(default)]
    pub conservation_loss: bool,
    #[serde(default)]
    pub unsafe_production_call: bool,
    #[serde(default)]
    pub failed_evidence: bool,
    #[serde(default)]
    pub claims_promoted: bool,
    #[serde(default)]
    pub deployed: bool,
    #[serde(default)]
    pub import_eligible: bool,
}

/// Closed diagnostic result emitted by the pure derivation function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MaturityDerivation {
    Reached {
        state: MaturityState,
        evidence_refs: Vec<String>,
    },
    Blocked {
        reason: BlockedReason,
        missing_or_offending: Option<String>,
        evidence_refs: Vec<String>,
    },
}

/// Human authorization is orthogonal to evidence maturity.
///
/// `Pending` is the safe default: a complete review package is not a human
/// decision. In particular, `human_decision_ref` is diagnostic provenance and
/// must never be interpreted as an authorization grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum HumanAuthorization {
    Pending,
    Authorized { decision_ref: String },
}

/// The machine-checkable result of separating review-package readiness from
/// human authorization. A `ReviewReady` result may still be `Pending`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewReadiness {
    pub maturity: MaturityState,
    pub evidence_refs: Vec<String>,
    pub authorization: HumanAuthorization,
}

impl ReviewReadiness {
    /// Production/import behavior is allowed only after an explicit,
    /// non-empty human authorization. Readiness alone never grants rights.
    pub fn authorizes_production_or_import(&self) -> bool {
        self.maturity == MaturityState::ReviewReady
            && matches!(
                &self.authorization,
                HumanAuthorization::Authorized { decision_ref }
                    if !decision_ref.trim().is_empty()
            )
    }
}

/// Derive review-package completeness without conflating it with a human gate.
///
/// This is the canonical readiness surface for callers that need to decide
/// whether review work can begin. The older `derive_maturity` function keeps
/// its closed ladder and `HumanGateRequired` terminal state for wire/backward
/// compatibility; this function deliberately reports the evidence terminal as
/// `ReviewReady` and leaves authorization `Pending`.
pub fn derive_review_readiness(
    set: &FamilyEvidenceSet,
) -> Result<ReviewReadiness, MaturityDerivation> {
    match derive_maturity(set) {
        MaturityDerivation::Reached {
            state: MaturityState::HumanGateRequired,
            evidence_refs,
        } => Ok(ReviewReadiness {
            maturity: MaturityState::ReviewReady,
            evidence_refs,
            authorization: HumanAuthorization::Pending,
        }),
        MaturityDerivation::Reached {
            state,
            evidence_refs,
        } => Ok(ReviewReadiness {
            maturity: state,
            evidence_refs,
            authorization: HumanAuthorization::Pending,
        }),
        blocked @ MaturityDerivation::Blocked { .. } => Err(blocked),
    }
}

fn blocked(
    reason: BlockedReason,
    missing_or_offending: impl Into<Option<String>>,
    evidence_refs: Vec<String>,
) -> MaturityDerivation {
    MaturityDerivation::Blocked {
        reason,
        missing_or_offending: missing_or_offending.into(),
        evidence_refs,
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// A parser or configuration identity is a *label or a sha256 fingerprint*,
/// never a raw configuration value.
///
/// The accepted alphabet excludes the separators that carry secrets and
/// locations: no whitespace, no `=`, `:`, `/`, `?` or `&`, so an environment
/// dump (`TOKEN=...`), a URL or a filesystem path is rejected instead of being
/// persisted into an artifact. The rule is syntactic — a label-shaped secret is
/// indistinguishable from a label by construction, which is why the producer
/// must emit a label or fingerprint rather than pass configuration through.
fn valid_identity_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FRESHNESS_IDENTITY_FIELD_LEN
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'@' | b'+' | b'-')
        })
}

/// Derive the highest contiguous maturity prefix from immutable receipts.
///
/// This function has no authority to promote, import, write, or consult a
/// runtime parse state. Explicit authority or integrity claims always win
/// over receipt presence and are returned as typed blocks. A bound freshness
/// identity is evaluated as well: an observation that is stale or superseded
/// blocks with [`BlockedReason::StaleIdentity`], while a set that carries no
/// identity keeps its legacy behaviour (currency is unknown, not asserted).
pub fn derive_maturity(set: &FamilyEvidenceSet) -> MaturityDerivation {
    derive_maturity_inner(set, None)
}

/// Currency-required variant of [`derive_maturity`].
///
/// The caller has selected the target revision, so the set must positively
/// account for it: no identity, a declaration that names a different target,
/// a stale or superseded observation, or an unusable identity all block with
/// [`BlockedReason::StaleIdentity`] or [`BlockedReason::MalformedReceipt`].
/// A declared revision is never treated as trusted — it is compared against
/// the selected target instead.
pub fn derive_maturity_against_target(
    set: &FamilyEvidenceSet,
    selected_target_revision: &str,
) -> MaturityDerivation {
    derive_maturity_inner(set, Some(selected_target_revision))
}

fn derive_maturity_inner(
    set: &FamilyEvidenceSet,
    selected_target_revision: Option<&str>,
) -> MaturityDerivation {
    if set.claims_promoted || set.deployed || set.import_eligible {
        return blocked(
            BlockedReason::AttemptedAutomaticPromotion,
            Some("authority_claim".to_owned()),
            Vec::new(),
        );
    }

    match assess_set_freshness(set, selected_target_revision) {
        FreshnessAssessment::Fresh { .. } => {}
        FreshnessAssessment::Unattributed => {
            if selected_target_revision.is_some() {
                return blocked(
                    BlockedReason::StaleIdentity,
                    Some("freshness_identity".to_owned()),
                    Vec::new(),
                );
            }
        }
        FreshnessAssessment::Stale { .. } | FreshnessAssessment::Superseded { .. } => {
            return blocked(
                BlockedReason::StaleIdentity,
                Some("freshness_identity.observed_revision".to_owned()),
                Vec::new(),
            );
        }
        FreshnessAssessment::Mismatched { .. } => {
            return blocked(
                BlockedReason::StaleIdentity,
                Some("freshness_identity.target_revision".to_owned()),
                Vec::new(),
            );
        }
        FreshnessAssessment::Malformed { .. } => {
            return blocked(
                BlockedReason::MalformedReceipt,
                Some("freshness_identity".to_owned()),
                Vec::new(),
            );
        }
    }

    let integrity_flags = [
        (set.pooling, BlockedReason::Pooling),
        (set.contamination, BlockedReason::Contamination),
        (set.stale_identity, BlockedReason::StaleIdentity),
        (set.denominator_drift, BlockedReason::DenominatorDrift),
        (set.conservation_loss, BlockedReason::ConservationLoss),
        (set.semantic_drift, BlockedReason::SemanticDrift),
        (
            set.unsafe_production_call,
            BlockedReason::UnsafeProductionCall,
        ),
        (set.failed_evidence, BlockedReason::FailedEvidence),
    ];
    if let Some((_, reason)) = integrity_flags.into_iter().find(|(set, _)| *set) {
        return blocked(reason, Some("integrity_flag".to_owned()), Vec::new());
    }

    if set.paper_id.trim().is_empty() {
        return blocked(
            BlockedReason::MalformedReceipt,
            Some("paper_id".to_owned()),
            Vec::new(),
        );
    }

    let slots = [
        (ReceiptKind::Inventory, "inventory", set.inventory.as_ref()),
        (ReceiptKind::Oracle, "oracle", set.oracle.as_ref()),
        (ReceiptKind::Atom, "atom", set.atom.as_ref()),
        (ReceiptKind::Sequence, "sequence", set.sequence.as_ref()),
        (ReceiptKind::Shadow, "shadow", set.shadow.as_ref()),
        (ReceiptKind::Review, "review", set.review.as_ref()),
    ];
    let mut source_sha256: Option<&str> = None;
    for (expected_kind, name, receipt) in slots
        .iter()
        .filter_map(|(kind, name, receipt)| receipt.map(|receipt| (*kind, *name, receipt)))
    {
        if receipt.kind != expected_kind {
            return blocked(
                BlockedReason::MalformedReceipt,
                Some(format!("{name}.kind")),
                Vec::new(),
            );
        }
        if receipt.evidence_ref.trim().is_empty() {
            return blocked(
                BlockedReason::MissingEvidence,
                Some(format!("{name}.evidence_ref")),
                Vec::new(),
            );
        }
        if !valid_digest(&receipt.source_sha256) {
            return blocked(
                BlockedReason::MalformedReceipt,
                Some(format!("{name}.source_sha256")),
                Vec::new(),
            );
        }
        if !valid_digest(&receipt.artifact_sha256) {
            return blocked(
                BlockedReason::MalformedReceipt,
                Some(format!("{name}.artifact_sha256")),
                Vec::new(),
            );
        }
        if receipt.grobid_profile != "structural" && receipt.grobid_profile != "rich" {
            return blocked(
                BlockedReason::MalformedReceipt,
                Some(format!("{name}.grobid_profile")),
                Vec::new(),
            );
        }
        if let Some(source) = source_sha256 {
            if source != receipt.source_sha256 {
                return blocked(
                    BlockedReason::MalformedReceipt,
                    Some(format!("{name}.source_sha256")),
                    Vec::new(),
                );
            }
        } else {
            source_sha256 = Some(&receipt.source_sha256);
        }
    }

    let first_gap = slots.iter().position(|(_, _, receipt)| receipt.is_none());
    let contiguous = slots.iter().take_while(|(_, _, receipt)| receipt.is_some());
    let evidence_refs: Vec<String> = contiguous
        .map(|(_, _, receipt)| {
            receipt
                .expect("take_while guarantees receipt")
                .evidence_ref
                .clone()
        })
        .collect();
    if let Some(index) = first_gap
        && slots[index + 1..]
            .iter()
            .any(|(_, _, later)| later.is_some())
    {
        return blocked(
            BlockedReason::LadderGap,
            Some(slots[index].1.to_owned()),
            evidence_refs,
        );
    }
    if evidence_refs.is_empty() {
        blocked(
            BlockedReason::MissingEvidence,
            Some(
                first_gap
                    .map(|index| slots[index].1.to_owned())
                    .unwrap_or_else(|| "inventory".to_owned()),
            ),
            evidence_refs,
        )
    } else {
        MaturityDerivation::Reached {
            state: if evidence_refs.len() == ReceiptKind::ALL.len() {
                MaturityState::HumanGateRequired
            } else {
                MaturityState::ALL[evidence_refs.len() - 1]
            },
            evidence_refs,
        }
    }
}

/// True only for one immediate forward step on the closed ladder.
pub fn can_advance(from: MaturityState, to: MaturityState) -> bool {
    match from.successor() {
        Some(next) => next.ladder_index() == to.ladder_index(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_wire_vocabularies_have_exact_sizes_and_names() {
        assert_eq!(FamilyId::ALL.len(), 5);
        assert_eq!(MaturityState::ALL.len(), 7);
        assert_eq!(ReceiptKind::ALL.len(), 6);
        assert_eq!(BlockedReason::ALL.len(), 14);
        assert_eq!(
            MaturityState::ALL.map(MaturityState::as_str),
            [
                "inventory_bound",
                "oracle_bound",
                "atom_proven",
                "sequence_proven",
                "shadow_observed",
                "review_ready",
                "human_gate_required",
            ]
        );
        assert!(MaturityState::parse("promoted").is_none());
    }

    #[test]
    fn all_wire_values_roundtrip_and_unknown_values_are_rejected() {
        for family in FamilyId::ALL {
            assert_eq!(FamilyId::parse(family.as_str()), Some(family));
        }
        for state in MaturityState::ALL {
            assert_eq!(MaturityState::parse(state.as_str()), Some(state));
        }
        for kind in ReceiptKind::ALL {
            assert_eq!(ReceiptKind::parse(kind.as_str()), Some(kind));
        }
        for reason in BlockedReason::ALL {
            assert_eq!(BlockedReason::parse(reason.as_str()), Some(reason));
        }
        assert!(FamilyId::parse("lattice").is_none());
        assert!(MaturityState::parse("future_state").is_none());
        assert!(ReceiptKind::parse("decision").is_none());
        assert!(BlockedReason::parse("invented_reason").is_none());
    }

    #[test]
    fn derivation_is_tagged_and_roundtrips() {
        let values = [
            MaturityDerivation::Reached {
                state: MaturityState::ReviewReady,
                evidence_refs: vec!["receipt-a".into()],
            },
            MaturityDerivation::Blocked {
                reason: BlockedReason::LadderGap,
                missing_or_offending: Some("atom".into()),
                evidence_refs: vec!["receipt-a".into()],
            },
        ];
        for value in values {
            let json = serde_json::to_value(&value).unwrap();
            let back: MaturityDerivation = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(value, back);
            assert!(
                json.get("status")
                    .and_then(serde_json::Value::as_str)
                    .is_some()
            );
        }
    }

    #[test]
    fn every_ladder_step_has_only_its_immediate_successor() {
        for (index, state) in MaturityState::ALL.iter().copied().enumerate() {
            let successor = state.successor();
            if let Some(next) = successor {
                assert_eq!(next.ladder_index(), index + 1);
                assert!(can_advance(state, next));
            } else {
                assert_eq!(index, MaturityState::ALL.len() - 1);
            }
            for target in MaturityState::ALL {
                assert_eq!(can_advance(state, target), successor == Some(target));
            }
        }
    }

    #[test]
    fn self_backward_and_skipped_transitions_are_rejected() {
        for (index, state) in MaturityState::ALL.iter().copied().enumerate() {
            assert!(!can_advance(state, state));
            if index > 0 {
                assert!(!can_advance(state, MaturityState::ALL[index - 1]));
            }
            if index + 2 < MaturityState::ALL.len() {
                assert!(!can_advance(state, MaturityState::ALL[index + 2]));
            }
        }
        assert!(!can_advance(
            MaturityState::HumanGateRequired,
            MaturityState::InventoryBound
        ));
    }

    fn receipt(kind: ReceiptKind, reference: &str) -> BoundReceipt {
        BoundReceipt {
            kind,
            evidence_ref: reference.into(),
            source_sha256: "a".repeat(64),
            artifact_sha256: "b".repeat(64),
            grobid_profile: "structural".into(),
        }
    }

    fn set() -> FamilyEvidenceSet {
        FamilyEvidenceSet {
            family: FamilyId::Body,
            paper_id: "paper-1".into(),
            inventory: None,
            oracle: None,
            atom: None,
            sequence: None,
            shadow: None,
            review: None,
            human_decision_ref: None,
            freshness: None,
            stale_identity: false,
            denominator_drift: false,
            pooling: false,
            contamination: false,
            semantic_drift: false,
            conservation_loss: false,
            unsafe_production_call: false,
            failed_evidence: false,
            claims_promoted: false,
            deployed: false,
            import_eligible: false,
        }
    }

    fn reached(result: MaturityDerivation) -> (MaturityState, Vec<String>) {
        match result {
            MaturityDerivation::Reached {
                state,
                evidence_refs,
            } => (state, evidence_refs),
            other => panic!("expected reached result, got {other:?}"),
        }
    }

    fn blocked_reason(result: MaturityDerivation) -> BlockedReason {
        match result {
            MaturityDerivation::Blocked { reason, .. } => reason,
            other => panic!("expected blocked result, got {other:?}"),
        }
    }

    #[test]
    fn every_contiguous_prefix_derives_exact_state_and_ordered_refs() {
        let kinds = ReceiptKind::ALL;
        for prefix in 1..=kinds.len() {
            let mut family = set();
            for (index, kind) in kinds.into_iter().enumerate().take(prefix) {
                let value = Some(receipt(kind, &format!("ref-{index}")));
                match kind {
                    ReceiptKind::Inventory => family.inventory = value,
                    ReceiptKind::Oracle => family.oracle = value,
                    ReceiptKind::Atom => family.atom = value,
                    ReceiptKind::Sequence => family.sequence = value,
                    ReceiptKind::Shadow => family.shadow = value,
                    ReceiptKind::Review => family.review = value,
                }
            }
            let (state, refs) = reached(derive_maturity(&family));
            let expected_state = if prefix == kinds.len() {
                MaturityState::HumanGateRequired
            } else {
                MaturityState::ALL[prefix - 1]
            };
            assert_eq!(state, expected_state);
            assert_eq!(
                refs,
                (0..prefix).map(|i| format!("ref-{i}")).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn empty_and_sparse_sets_fail_closed() {
        let mut family = set();
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MissingEvidence
        );
        family.shadow = Some(receipt(ReceiptKind::Shadow, "shadow-ref"));
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::LadderGap
        );
        match derive_maturity(&family) {
            MaturityDerivation::Blocked { evidence_refs, .. } => assert!(evidence_refs.is_empty()),
            other => panic!("expected block, got {other:?}"),
        }
    }

    #[test]
    fn authority_and_integrity_flags_have_fixed_priority() {
        let mut family = set();
        family.inventory = Some(receipt(ReceiptKind::Inventory, "inventory-ref"));
        family.failed_evidence = true;
        family.pooling = true;
        family.claims_promoted = true;
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::AttemptedAutomaticPromotion
        );
        family.claims_promoted = false;
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::Pooling
        );
    }

    #[test]
    fn malformed_receipts_never_downgrade_to_a_reached_state() {
        let mut family = set();
        family.inventory = Some(receipt(ReceiptKind::Oracle, "wrong-kind"));
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MalformedReceipt
        );
        family.inventory = Some(receipt(ReceiptKind::Inventory, ""));
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MissingEvidence
        );
        family.inventory = Some(receipt(ReceiptKind::Inventory, "ref"));
        family.inventory.as_mut().unwrap().source_sha256 = "bad".into();
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MalformedReceipt
        );
        family.inventory = Some(receipt(ReceiptKind::Inventory, "ref"));
        family.oracle = Some(receipt(ReceiptKind::Oracle, "oracle-ref"));
        family.oracle.as_mut().unwrap().source_sha256 = "c".repeat(64);
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MalformedReceipt
        );
        family.oracle.as_mut().unwrap().source_sha256 = "a".repeat(64);
        family.oracle.as_mut().unwrap().grobid_profile = "unknown".into();
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::MalformedReceipt
        );
    }

    #[test]
    fn fsm_readiness_authorization_keeps_review_ready_pending() {
        let mut family = set();
        for (slot, kind) in [
            (&mut family.inventory, ReceiptKind::Inventory),
            (&mut family.oracle, ReceiptKind::Oracle),
            (&mut family.atom, ReceiptKind::Atom),
            (&mut family.sequence, ReceiptKind::Sequence),
            (&mut family.shadow, ReceiptKind::Shadow),
            (&mut family.review, ReceiptKind::Review),
        ] {
            *slot = Some(receipt(kind, kind.as_str()));
        }
        family.human_decision_ref = Some("diagnostic-only-ref".into());

        let readiness =
            derive_review_readiness(&family).expect("complete evidence is review-ready");
        assert_eq!(readiness.maturity, MaturityState::ReviewReady);
        assert_eq!(readiness.evidence_refs.len(), ReceiptKind::ALL.len());
        assert_eq!(readiness.authorization, HumanAuthorization::Pending);
        assert!(!readiness.authorizes_production_or_import());

        let authorized = ReviewReadiness {
            authorization: HumanAuthorization::Authorized {
                decision_ref: "human-approval-1".into(),
            },
            ..readiness.clone()
        };
        assert!(authorized.authorizes_production_or_import());
        assert!(
            !ReviewReadiness {
                authorization: HumanAuthorization::Authorized {
                    decision_ref: "  ".into(),
                },
                ..readiness
            }
            .authorizes_production_or_import()
        );
    }

    #[test]
    fn incomplete_evidence_is_not_review_ready_and_human_gate_is_not_granted() {
        let mut family = set();
        family.inventory = Some(receipt(ReceiptKind::Inventory, "inventory-ref"));
        let readiness = derive_review_readiness(&family).expect("inventory is a valid prefix");
        assert_eq!(readiness.maturity, MaturityState::InventoryBound);
        assert_eq!(readiness.authorization, HumanAuthorization::Pending);
        assert!(!readiness.authorizes_production_or_import());

        family.review = Some(receipt(ReceiptKind::Review, "review-ref"));
        assert!(matches!(
            derive_review_readiness(&family),
            Err(MaturityDerivation::Blocked {
                reason: BlockedReason::LadderGap,
                ..
            })
        ));
    }

    fn freshness_identity() -> FreshnessIdentity {
        FreshnessIdentity {
            target_revision: "arxiv:1234.5678v3".into(),
            observed_revision: "arxiv:1234.5678v3".into(),
            source_digest: "a".repeat(64),
            representation_digest: "b".repeat(64),
            parser_identity: "grobid-0.9.0-crf".into(),
            config_identity: "structural-v1".into(),
            cache_key: "cache-key-1".into(),
            observed_at: "2026-09-11T00:00:00Z".into(),
        }
    }

    #[test]
    fn freshness_identity_is_revision_bound_without_wall_clock_oracle() {
        let identity = freshness_identity();
        assert_eq!(
            assess_freshness(&identity, "arxiv:1234.5678v3"),
            Ok(FreshnessOutcome::Fresh)
        );

        let mut stale = identity.clone();
        stale.observed_revision = "arxiv:1234.5678v2".into();
        assert_eq!(
            assess_freshness(&stale, "arxiv:1234.5678v3"),
            Ok(FreshnessOutcome::Stale)
        );

        let mut superseded = identity;
        superseded.observed_revision = "arxiv:1234.5678v4".into();
        assert_eq!(
            assess_freshness(&superseded, "arxiv:1234.5678v3"),
            Ok(FreshnessOutcome::Superseded)
        );
    }

    #[test]
    fn equal_digests_do_not_rescue_stale_or_superseded_revision() {
        let mut identity = freshness_identity();
        identity.observed_revision = "arxiv:1234.5678v2".into();
        assert_eq!(identity.source_digest, "a".repeat(64));
        assert_eq!(identity.representation_digest, "b".repeat(64));
        assert_eq!(
            assess_freshness(&identity, "arxiv:1234.5678v3"),
            Ok(FreshnessOutcome::Stale)
        );
    }

    #[test]
    fn freshness_identity_fails_closed_and_tei_trust_is_explicit() {
        let mut identity = freshness_identity();
        identity.source_digest = "not-a-sha256".into();
        assert_eq!(
            assess_freshness(&identity, "arxiv:1234.5678v3"),
            Err(FreshnessIdentityError::InvalidDigest)
        );
        assert_eq!(
            assess_freshness(&freshness_identity(), "arxiv:1234.5678v4"),
            Err(FreshnessIdentityError::TargetRevisionMismatch)
        );

        let declared = DeclaredRevisionTrust::TeiDeclared {
            revision: "arxiv:1234.5678v3".into(),
        };
        let deferred = DeclaredRevisionTrust::DeferredCrossCheck {
            receipt_ref: "cross-check-1".into(),
        };
        assert_eq!(
            serde_json::to_value(declared).unwrap()["status"],
            "tei_declared"
        );
        assert_eq!(
            serde_json::to_value(deferred).unwrap()["status"],
            "deferred_cross_check"
        );
    }

    #[test]
    fn human_decision_is_diagnostic_only_and_containers_are_strict() {
        let mut family = set();
        family.human_decision_ref = Some("human-1".into());
        for (slot, kind) in [
            (&mut family.inventory, ReceiptKind::Inventory),
            (&mut family.oracle, ReceiptKind::Oracle),
            (&mut family.atom, ReceiptKind::Atom),
            (&mut family.sequence, ReceiptKind::Sequence),
            (&mut family.shadow, ReceiptKind::Shadow),
            (&mut family.review, ReceiptKind::Review),
        ] {
            *slot = Some(receipt(kind, kind.as_str()));
        }
        assert_eq!(
            reached(derive_maturity(&family)).0,
            MaturityState::HumanGateRequired
        );
        let mut json = serde_json::to_value(&family).unwrap();
        json["unexpected"] = true.into();
        assert!(serde_json::from_value::<FamilyEvidenceSet>(json).is_err());
    }

    fn bound_freshness(identity: FreshnessIdentity) -> FreshnessBinding {
        FreshnessBinding::new(
            identity.clone(),
            DeclaredRevisionTrust::TeiDeclared {
                revision: identity.target_revision.clone(),
            },
        )
        .expect("a matching TEI declaration is accepted")
    }

    #[test]
    fn freshness_binding_records_a_declaration_without_trusting_it() {
        assert_eq!(
            FreshnessBinding::new(
                freshness_identity(),
                DeclaredRevisionTrust::TeiDeclared {
                    revision: "arxiv:1234.5678v4".into(),
                },
            ),
            Err(FreshnessIdentityError::TargetRevisionMismatch)
        );
        assert_eq!(
            FreshnessBinding::new(
                freshness_identity(),
                DeclaredRevisionTrust::DeferredCrossCheck {
                    receipt_ref: "  ".into(),
                },
            ),
            Err(FreshnessIdentityError::EmptyField)
        );
        let deferred = FreshnessBinding::new(
            freshness_identity(),
            DeclaredRevisionTrust::DeferredCrossCheck {
                receipt_ref: "cross-check-1".into(),
            },
        )
        .expect("a non-TEI source may defer");
        assert!(matches!(
            deferred.declared_revision,
            DeclaredRevisionTrust::DeferredCrossCheck { .. }
        ));
    }

    #[test]
    fn freshness_is_bounded_and_rejects_config_shaped_identities() {
        let mut oversized = freshness_identity();
        oversized.cache_key = "k".repeat(MAX_FRESHNESS_IDENTITY_FIELD_LEN + 1);
        assert_eq!(
            assess_freshness(&oversized, "arxiv:1234.5678v3"),
            Err(FreshnessIdentityError::FieldTooLong)
        );

        for raw in [
            "GROBID_ENDPOINT=http://127.0.0.1:8070",
            "/etc/da/config.yaml",
            "{\"token\": \"abc\"}",
        ] {
            let mut identity = freshness_identity();
            identity.config_identity = raw.into();
            assert_eq!(
                assess_freshness(&identity, "arxiv:1234.5678v3"),
                Err(FreshnessIdentityError::UnsafeIdentityLabel),
                "a raw config value must not be readable as a measurement: {raw}"
            );
        }

        let mut label_shaped = freshness_identity();
        label_shaped.parser_identity = "RevisionMaterializationUseCase@revision-owned-v1".into();
        assert_eq!(
            assess_freshness(&label_shaped, "arxiv:1234.5678v3"),
            Ok(FreshnessOutcome::Fresh)
        );
    }

    #[test]
    fn a_bound_set_blocks_when_its_input_is_not_current() {
        let mut family = set();
        family.inventory = Some(receipt(ReceiptKind::Inventory, "inventory-ref"));

        family.freshness = Some(bound_freshness(freshness_identity()));
        assert_eq!(
            reached(derive_maturity(&family)).0,
            MaturityState::InventoryBound
        );
        assert!(assess_set_freshness(&family, None).is_fresh());

        let mut superseded = freshness_identity();
        superseded.observed_revision = "arxiv:1234.5678v4".into();
        family.freshness = Some(bound_freshness(superseded));
        assert_eq!(
            blocked_reason(derive_maturity(&family)),
            BlockedReason::StaleIdentity
        );
        assert_eq!(
            assess_set_freshness(&family, None),
            FreshnessAssessment::Superseded {
                target_revision: "arxiv:1234.5678v3".into(),
                observed_revision: "arxiv:1234.5678v4".into(),
            }
        );
        assert_eq!(
            blocked_reason(derive_maturity_against_target(&family, "arxiv:1234.5678v3")),
            BlockedReason::StaleIdentity
        );

        family.freshness = Some(bound_freshness(freshness_identity()));
        assert_eq!(
            blocked_reason(derive_maturity_against_target(&family, "arxiv:1234.5678v9")),
            BlockedReason::StaleIdentity
        );
        assert_eq!(
            assess_set_freshness(&family, Some("arxiv:1234.5678v9")).as_str(),
            "mismatched"
        );

        family.freshness = None;
        assert_eq!(
            assess_set_freshness(&family, None),
            FreshnessAssessment::Unattributed
        );
        assert_eq!(
            reached(derive_maturity(&family)).0,
            MaturityState::InventoryBound
        );
        assert_eq!(
            blocked_reason(derive_maturity_against_target(&family, "arxiv:1234.5678v3")),
            BlockedReason::StaleIdentity
        );
    }

    #[test]
    fn a_deserialized_contradictory_declaration_is_not_a_measurement() {
        let mut family = set();
        family.inventory = Some(receipt(ReceiptKind::Inventory, "inventory-ref"));
        family.freshness = Some(bound_freshness(freshness_identity()));
        let mut json = serde_json::to_value(&family).unwrap();
        json["freshness"]["declared_revision"]["revision"] = "arxiv:1234.5678v4".into();
        let contradiction: FamilyEvidenceSet = serde_json::from_value(json).unwrap();
        assert!(matches!(
            assess_set_freshness(&contradiction, None),
            FreshnessAssessment::Malformed {
                error: FreshnessIdentityError::TargetRevisionMismatch
            }
        ));
        assert_eq!(
            blocked_reason(derive_maturity(&contradiction)),
            BlockedReason::MalformedReceipt
        );
    }
}
