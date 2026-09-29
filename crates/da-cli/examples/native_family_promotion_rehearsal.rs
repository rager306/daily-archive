//! Offline rehearsal of a one-lane native-family promotion with an executed
//! rollback, as a pure primitive.
//!
//! This standalone example is not a runtime route: it is not registered in
//! `da-cli`'s library, it is not reachable from `ParserPort`, `source_cli` or
//! any `da` subcommand, and `main` writes nothing into the repository.  The
//! switch it exercises lives only in this process's memory, so both declared
//! route carriers (`data/parse/comparison_contract.yaml` and
//! `data/parse/native_fulltext_composition.yaml`) are read-only inputs and their
//! sha256 is taken from disk before and after the run.
//!
//! The primitive is deliberately falsifiable rather than declarative:
//!
//! * `baseline`  — the rehearsed lane keeps its Java fallback (`Fallback`);
//! * `switched`  — the same band, but the lane's receipt is `Passed`, so the
//!   composer splices the native `<teiHeader>`;
//! * `returned`  — the same code on the same input with `Fallback` again.
//!
//! The executed-rollback claim is the byte equality `returned == baseline`, and
//! it is asserted on the bytes — a switch that leaked a native fragment into the
//! rollback would fail it.  The lane closure claim is `references_changed ==
//! false`, computed by re-reading the emitted document's own subtrees.
//!
//! Local adaptation of the S06/T01 task plan: a declared carrier that cannot be
//! read, or whose declared shape drifted, is a broken checkout and therefore a
//! hard failure, not a rehearsal refusal — the closed vocabulary of
//! [`RehearsalRefusal`] keeps exactly the ten policy refusals the plan names.

use da_parse::{
    AdmissionDisposition, AffiliationSpan, FulltextAdmissionReceipt, FulltextComponent,
    FulltextCompositionIdentity, GROBID_HTTP_BACKEND_ID, HeaderAuthorInput, HeaderCompositionInput,
    HeaderCompositionOutcome, NATIVE_FULLTEXT_FALLBACK_TARGET,
    NATIVE_FULLTEXT_LANE_FALLBACK_LANE_CLOSURE, NATIVE_FULLTEXT_LANE_FALLBACK_POOLING,
    NATIVE_FULLTEXT_LANE_FALLBACK_RUST_RUN_MAY_NOT_CANCEL, NativeAffiliationAuthorEvidence,
    NativeAffiliationLink, NativeAffiliationLinkOutcome, NativeAffiliationParser, NativeDateParser,
    NativeFulltextCompositionInput, NativeFulltextCompositionOutcome, NativeHeaderFields,
    NativePersonalNameParser, PersonalNameContext, ReferenceCompositionOutcome, TeiEnvelope,
    TeiIdentity, compose_header_with_fields, compose_native_fulltext, tei_element_bytes,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Identity of the report this primitive feeds.  Kept deliberately out of
/// `population_contract.report_paths` (that rule covers `*_shadow.json` only):
/// this is an executed rehearsal, not a shadow population member.
pub const REHEARSAL_SCHEMA_VERSION: &str = "native-family-promotion-rehearsal.v1";
/// The executed-rollback episode marker: this run rehearses, it does not release.
pub const REHEARSAL_EPISODE: &str = "rehearsal";
pub const CLAIM_SCOPE: &str = "rehearsal_only";
pub const POOLED_QUALITY_CLAIM: &str = "forbidden";

/// The two declared route carriers this primitive reads and never writes.
pub const DATA_COMPARISON: &str = "data/parse/comparison_contract.yaml";
pub const DATA_COMPOSITION: &str = "data/parse/native_fulltext_composition.yaml";

/// The single rehearsed lane (S06 research/recommendation): header inside the
/// declared candidate `da-parse.native_header_reference_composition`.
pub const REHEARSAL_LANE: RehearsalLane = RehearsalLane::Header;

// The header lane's named gap, restated so a report cannot hide it.
pub const NAMED_GAP_JAVA_HEADER_RAW: u64 = 21;
pub const NAMED_GAP_NATIVE_ACCEPTED_HEADER: u64 = 20;

const FIXTURE: &str =
    include_str!("../../da-parse/tests/fixtures/native_fulltext_composition_oracle.yaml");

// ---------------------------------------------------------------------------
// Closed refusal vocabulary
// ---------------------------------------------------------------------------

/// Every way a rehearsal switch is refused, in evaluation order.
///
/// The vocabulary is closed and its serde wire names are the `snake_case` of
/// each variant (asserted by the consumer test, so a rename cannot silently move
/// a governor-visible token).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RehearsalRefusal {
    /// The rehearsed lane is not a row of the declared route table at all.
    UnknownLane,
    /// The lane is declared but not independently selectable (R125).
    LaneNotIndependentlySelectable,
    /// `lane_fallback_policy.pooling` is not `forbidden`.
    PoolingForbidden,
    /// `rust_run_may_not_cancel_fallback` is not `true`: a Rust run would be able
    /// to cancel a lane's fallback right.
    RustRunMayNotCancelFallback,
    /// The lane names no rollback target, so the switch would be irreversible.
    MissingRollbackTarget,
    /// The lane's rollback target is not the declared Java authority.
    RollbackTargetDrift,
    /// `auto_promote` is not `forbidden`.
    AutoPromoteNotForbidden,
    /// The lane is already native: a second promotion without an executed
    /// rollback is refused rather than silently idempotent.
    SecondPromotionWithoutRollback,
    /// `observation_mode` is not `shadow`.
    ObservationModeNotShadow,
    /// `promotion_state` is not `candidate`.
    PromotionStateNotCandidate,
}

impl RehearsalRefusal {
    /// Every class, in the order [`switch_lane`] evaluates them.
    pub const ALL: [RehearsalRefusal; 10] = [
        Self::UnknownLane,
        Self::LaneNotIndependentlySelectable,
        Self::PoolingForbidden,
        Self::RustRunMayNotCancelFallback,
        Self::MissingRollbackTarget,
        Self::RollbackTargetDrift,
        Self::AutoPromoteNotForbidden,
        Self::SecondPromotionWithoutRollback,
        Self::ObservationModeNotShadow,
        Self::PromotionStateNotCandidate,
    ];

    /// The `snake_case` wire token, identical to the serde rename.
    pub const fn token(self) -> &'static str {
        match self {
            Self::UnknownLane => "unknown_lane",
            Self::LaneNotIndependentlySelectable => "lane_not_independently_selectable",
            Self::PoolingForbidden => "pooling_forbidden",
            Self::RustRunMayNotCancelFallback => "rust_run_may_not_cancel_fallback",
            Self::MissingRollbackTarget => "missing_rollback_target",
            Self::RollbackTargetDrift => "rollback_target_drift",
            Self::AutoPromoteNotForbidden => "auto_promote_not_forbidden",
            Self::SecondPromotionWithoutRollback => "second_promotion_without_rollback",
            Self::ObservationModeNotShadow => "observation_mode_not_shadow",
            Self::PromotionStateNotCandidate => "promotion_state_not_candidate",
        }
    }
}

impl fmt::Display for RehearsalRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// Why the native side of the rehearsed lane could not be admitted.
///
/// Distinct from [`RehearsalRefusal`]: a policy refusal keeps the route table
/// unchanged, while this one says the lane is *allowed* native substitution but
/// the typed band did not produce an admissible outcome.  `switched` is then
/// byte-equal to `baseline` and carries this token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeSideRefusal {
    /// The lane's typed composition outcome was `Rejected{..}`.
    NativeOutcomeRejected,
    /// No typed composition outcome was produced for the lane at all.
    NativeOutcomeAbsent,
}

impl NativeSideRefusal {
    pub const fn token(self) -> &'static str {
        match self {
            Self::NativeOutcomeRejected => "native_outcome_rejected",
            Self::NativeOutcomeAbsent => "native_outcome_absent",
        }
    }
}

/// The typed reason a triad's switch step did not move the lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchRefusal {
    Policy(RehearsalRefusal),
    NativeSide(NativeSideRefusal),
}

impl SwitchRefusal {
    pub const fn token(self) -> &'static str {
        match self {
            Self::Policy(refusal) => refusal.token(),
            Self::NativeSide(refusal) => refusal.token(),
        }
    }
}

// ---------------------------------------------------------------------------
// Lanes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RehearsalLane {
    Header,
    References,
}

impl RehearsalLane {
    pub const ALL: [RehearsalLane; 2] = [Self::Header, Self::References];

    pub const fn token(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::References => "references",
        }
    }
}

/// A lane's route state: Java (the rollback authority) or native (the rehearsal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneState {
    Java,
    Native,
}

impl LaneState {
    pub const fn token(self) -> &'static str {
        match self {
            Self::Java => "java",
            Self::Native => "native",
        }
    }
}

/// The target of a switch.  The same closed set as a lane's state: a rehearsal
/// may only name a state one of the two routes already is.
pub type LaneTarget = LaneState;

/// The per-lane route states of one declared (or rehearsed) table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneStates {
    pub header: LaneState,
    pub references: LaneState,
}

impl LaneStates {
    /// Both lanes on the Java authority: the declared production state.
    pub const fn java() -> Self {
        Self {
            header: LaneState::Java,
            references: LaneState::Java,
        }
    }

    pub const fn state(self, lane: RehearsalLane) -> LaneState {
        match lane {
            RehearsalLane::Header => self.header,
            RehearsalLane::References => self.references,
        }
    }

    pub const fn with(self, lane: RehearsalLane, state: LaneState) -> Self {
        match lane {
            RehearsalLane::Header => Self {
                header: state,
                ..self
            },
            RehearsalLane::References => Self {
                references: state,
                ..self
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Declared route table
// ---------------------------------------------------------------------------

/// One lane row of the declared route table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredLane {
    pub lane: RehearsalLane,
    /// The declared route state (Java: the rollback authority, not the candidate).
    pub state: LaneState,
    /// The lane's own rollback target (`comparison_contract.yaml#rollback`).
    pub rollback_target: String,
    /// `independently_selectable_lanes` membership (R125).
    pub independently_selectable: bool,
    /// The lane's disposition in `admission.current_matrix` (`passed`, ...).
    pub admission_state: String,
}

/// The declared route table, read from exactly two DATA carriers and never
/// written back.
///
/// It carries per-lane state, per-lane rollback target and the policy locks
/// (`lane_closure`, `pooling`, the no-cancel rule) the switch must honour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredRouteTable {
    pub candidate_id: String,
    pub candidate_version: String,
    pub observation_mode: String,
    pub promotion_state: String,
    pub auto_promote: String,
    pub lanes: Vec<DeclaredLane>,
    pub lane_closure: String,
    pub pooling: String,
    pub terminal_receipt_is_pass: bool,
    pub rust_run_may_not_cancel_fallback: bool,
    pub fallback_target: String,
    /// Declared carrier path -> `sha256:<hex>`, taken from disk.
    pub carrier_sha256: BTreeMap<String, String>,
}

impl DeclaredRouteTable {
    pub fn lane(&self, lane: RehearsalLane) -> Option<&DeclaredLane> {
        self.lanes.iter().find(|row| row.lane == lane)
    }

    pub fn states(&self) -> LaneStates {
        let mut states = LaneStates::java();
        for row in &self.lanes {
            states = states.with(row.lane, row.state);
        }
        states
    }
}

// ---------------------------------------------------------------------------
// Carriers and the declared table reader
// ---------------------------------------------------------------------------

/// `sha256:<hex>` of a declared carrier, read from disk.
///
/// A carrier that cannot be read is a broken checkout, not a rehearsal refusal,
/// so it fails loudly instead of returning a sentinel a caller could mistake for
/// a real digest.
pub fn carrier_sha256(root: &Path, rel: &str) -> String {
    let bytes = std::fs::read(root.join(rel))
        .unwrap_or_else(|error| panic!("declared route carrier {rel} is unreadable: {error}"));
    format!("sha256:{:x}", Sha256::digest(&bytes))
}

/// Walk up from `start` until both declared route carriers exist.
pub fn locate_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        if dir.join(DATA_COMPARISON).is_file() && dir.join(DATA_COMPOSITION).is_file() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

/// The working directory, else this example's own manifest directory.
///
/// Public because the consumer test asserts the two resolutions agree, so `main`
/// and the test can never disagree about which checkout they are reading.
pub fn default_root() -> Option<PathBuf> {
    let from_cwd = std::env::current_dir()
        .ok()
        .and_then(|cwd| locate_repo_root(&cwd));
    from_cwd.or_else(|| locate_repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))))
}

fn carrier_yaml(root: &Path, rel: &str) -> serde_yaml::Value {
    let text = std::fs::read_to_string(root.join(rel))
        .unwrap_or_else(|error| panic!("declared route carrier {rel} is unreadable: {error}"));
    serde_yaml::from_str(&text)
        .unwrap_or_else(|error| panic!("declared route carrier {rel} is not YAML: {error}"))
}

fn field<'a>(node: &'a serde_yaml::Value, key: &str) -> &'a serde_yaml::Value {
    node.get(key)
        .unwrap_or_else(|| panic!("declared route carrier is missing `{key}`"))
}

fn text(node: &serde_yaml::Value, key: &str) -> String {
    field(node, key)
        .as_str()
        .unwrap_or_else(|| panic!("declared route field `{key}` is not a string"))
        .to_owned()
}

fn bool_field(node: &serde_yaml::Value, key: &str) -> bool {
    field(node, key)
        .as_bool()
        .unwrap_or_else(|| panic!("declared route field `{key}` is not a boolean"))
}

fn text_list(node: &serde_yaml::Value, key: &str) -> Vec<String> {
    field(node, key)
        .as_sequence()
        .unwrap_or_else(|| panic!("declared route field `{key}` is not a sequence"))
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .unwrap_or_else(|| panic!("declared route field `{key}` has a non-string entry"))
                .to_owned()
        })
        .collect()
}

/// Read the declared route table from the two carriers, refusing any declared
/// state that would make a rehearsal switch a promotion.
pub fn read_declared_route_table(root: &Path) -> Result<DeclaredRouteTable, RehearsalRefusal> {
    let comparison = carrier_yaml(root, DATA_COMPARISON);
    let composition = carrier_yaml(root, DATA_COMPOSITION);

    let candidate = field(&comparison, "native_header_reference_composition_shadow");
    let admission = field(&composition, "admission");
    let policy = field(&composition, "lane_fallback_policy");
    let matrix = field(admission, "current_matrix");

    // Policy reads, in the same order `switch_lane` evaluates them.
    let observation_mode = text(candidate, "observation_mode");
    if observation_mode != "shadow" {
        return Err(RehearsalRefusal::ObservationModeNotShadow);
    }
    let promotion_state = text(candidate, "promotion_state");
    if promotion_state != "candidate" {
        return Err(RehearsalRefusal::PromotionStateNotCandidate);
    }
    let auto_promote = text(candidate, "auto_promote");
    if auto_promote != "forbidden" {
        return Err(RehearsalRefusal::AutoPromoteNotForbidden);
    }
    let pooling = text(policy, "pooling");
    if pooling != NATIVE_FULLTEXT_LANE_FALLBACK_POOLING {
        return Err(RehearsalRefusal::PoolingForbidden);
    }
    let lane_closure = text(policy, "lane_closure");
    if lane_closure != NATIVE_FULLTEXT_LANE_FALLBACK_LANE_CLOSURE {
        panic!(
            "declared lane_fallback_policy.lane_closure is {lane_closure:?}, not the declared \
             {:?}: one lane's success could cancel a sibling's fallback",
            NATIVE_FULLTEXT_LANE_FALLBACK_LANE_CLOSURE
        );
    }
    let rust_run_may_not_cancel_fallback = bool_field(policy, "rust_run_may_not_cancel_fallback");
    if rust_run_may_not_cancel_fallback != NATIVE_FULLTEXT_LANE_FALLBACK_RUST_RUN_MAY_NOT_CANCEL {
        return Err(RehearsalRefusal::RustRunMayNotCancelFallback);
    }
    let fallback_target = text(admission, "fallback_target");
    let terminal_receipt_is_pass = bool_field(admission, "terminal_receipt_is_pass");

    // The three carriers that publish the selectable lanes must agree; a
    // disagreement is a broken checkout, not a rehearsal decision.
    let selectable = text_list(candidate, "independently_selectable_lanes");
    let admission_selectable = text_list(admission, "independently_selectable");
    let policy_selectable = text_list(policy, "independently_selectable_lanes");
    if selectable != admission_selectable || selectable != policy_selectable {
        panic!(
            "declared carriers disagree on independently_selectable lanes: {selectable:?} vs \
             {admission_selectable:?} vs {policy_selectable:?}"
        );
    }

    let rollback = field(candidate, "rollback");
    let mut lanes = Vec::with_capacity(selectable.len());
    for lane_token in &selectable {
        let lane = match lane_token.as_str() {
            "header" => RehearsalLane::Header,
            "references" => RehearsalLane::References,
            other => {
                panic!("declared route carrier publishes an unknown selectable lane {other:?}")
            }
        };
        let rollback_target = text(rollback, lane.token());
        if rollback_target.is_empty() {
            return Err(RehearsalRefusal::MissingRollbackTarget);
        }
        if rollback_target != NATIVE_FULLTEXT_FALLBACK_TARGET {
            return Err(RehearsalRefusal::RollbackTargetDrift);
        }
        lanes.push(DeclaredLane {
            lane,
            // The declared production state is Java: the candidate is shadow and
            // the rollback authority is the frozen Java envelope.
            state: LaneState::Java,
            rollback_target,
            independently_selectable: true,
            admission_state: text(matrix, lane.token()),
        });
    }

    let mut carrier_digests = BTreeMap::new();
    for rel in [DATA_COMPARISON, DATA_COMPOSITION] {
        carrier_digests.insert(rel.to_owned(), carrier_sha256(root, rel));
    }

    Ok(DeclaredRouteTable {
        candidate_id: text(candidate, "candidate_id"),
        candidate_version: text(candidate, "candidate_version"),
        observation_mode,
        promotion_state,
        auto_promote,
        lanes,
        lane_closure,
        pooling,
        terminal_receipt_is_pass,
        rust_run_may_not_cancel_fallback,
        fallback_target,
        carrier_sha256: carrier_digests,
    })
}

// ---------------------------------------------------------------------------
// The switch
// ---------------------------------------------------------------------------

/// Move exactly one declared lane, returning a NEW table and never mutating the
/// one it was given.
pub fn switch_lane(
    table: &DeclaredRouteTable,
    lane: RehearsalLane,
    target: LaneTarget,
) -> Result<DeclaredRouteTable, RehearsalRefusal> {
    let row = table
        .lane(lane)
        .ok_or(RehearsalRefusal::UnknownLane)?
        .clone();
    if !row.independently_selectable {
        return Err(RehearsalRefusal::LaneNotIndependentlySelectable);
    }
    if table.pooling != NATIVE_FULLTEXT_LANE_FALLBACK_POOLING {
        return Err(RehearsalRefusal::PoolingForbidden);
    }
    if table.rust_run_may_not_cancel_fallback
        != NATIVE_FULLTEXT_LANE_FALLBACK_RUST_RUN_MAY_NOT_CANCEL
    {
        return Err(RehearsalRefusal::RustRunMayNotCancelFallback);
    }
    if row.rollback_target.is_empty() {
        return Err(RehearsalRefusal::MissingRollbackTarget);
    }
    if row.rollback_target != NATIVE_FULLTEXT_FALLBACK_TARGET {
        return Err(RehearsalRefusal::RollbackTargetDrift);
    }
    if table.auto_promote != "forbidden" {
        return Err(RehearsalRefusal::AutoPromoteNotForbidden);
    }
    // Only a move onto the native route is a promotion; the return to Java is the
    // rollback and stays available.
    if matches!(target, LaneState::Native) && matches!(row.state, LaneState::Native) {
        return Err(RehearsalRefusal::SecondPromotionWithoutRollback);
    }
    if table.observation_mode != "shadow" {
        return Err(RehearsalRefusal::ObservationModeNotShadow);
    }
    if table.promotion_state != "candidate" {
        return Err(RehearsalRefusal::PromotionStateNotCandidate);
    }

    let mut next = table.clone();
    for candidate in &mut next.lanes {
        if candidate.lane == lane {
            candidate.state = target;
        }
    }
    Ok(next)
}

// ---------------------------------------------------------------------------
// The rehearsal triad
// ---------------------------------------------------------------------------

/// What the composer did to one emission in the triad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmissionOutcome {
    Accepted,
    Rejected,
    /// The rehearsal's own variant: the switch was refused, so this emission *is*
    /// the Java baseline and carries the refusal token.  The composer itself has
    /// only `Accepted`/`Rejected`.
    Refused,
}

/// One emission of the triad: the composer's outcome plus the bytes it returned.
///
/// `bytes` is kept so byte equality is asserted on the document and never on a
/// summary field; it is not serialized into the report (which publishes
/// `sha256` and the byte count instead).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehearsalEmission {
    pub outcome: EmissionOutcome,
    pub error_token: Option<String>,
    pub sha256: String,
    pub bytes: Vec<u8>,
    pub lane_states: LaneStates,
}

impl RehearsalEmission {
    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }

    pub fn outcome_token(&self) -> &'static str {
        match self.outcome {
            EmissionOutcome::Accepted => "accepted",
            EmissionOutcome::Rejected => "rejected",
            EmissionOutcome::Refused => "refused",
        }
    }
}

/// Which band the switch actually moved, derived from the emitted subtrees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneMovement {
    pub header_changed: bool,
    pub references_changed: bool,
}

/// The executed-rollback checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RollbackCheck {
    /// `sha256` of `returned` equals `sha256` of `baseline`.
    pub returned_equals_baseline: bool,
    /// Full byte identity — the falsifiable form of the same claim.
    pub byte_identity: bool,
    /// The in-memory route table came back to the declared one.
    pub table_returned_to_declared: bool,
}

/// The native band's typed outcome, as produced by the lane's leaf parsers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmittedLaneOutcome {
    /// The lane may be admitted: its `Passed` receipt is built and the composer
    /// splices the native band.
    Admitted(Box<HeaderCompositionOutcome>),
    /// The lane is refused before the composer: `switched` is byte-equal to
    /// `baseline` and carries this typed reason.
    Refused(NativeSideRefusal),
}

/// The whole rehearsal of one document: three emissions on one input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriadOutcome {
    pub baseline: RehearsalEmission,
    pub switched: RehearsalEmission,
    pub returned: RehearsalEmission,
    pub lane_movement: LaneMovement,
    pub rollback: RollbackCheck,
    /// The table as the switch left it (the rehearsed state, in memory).
    pub rehearsal_table: DeclaredRouteTable,
    /// The table after the return; equality with the declared one is the
    /// table-level executed rollback.
    pub returned_table: DeclaredRouteTable,
    pub switch_refusal: Option<SwitchRefusal>,
}

impl TriadOutcome {
    /// The switch moved the emission bytes rather than only a declared flag.
    pub fn switched_performed(&self) -> bool {
        self.switch_refusal.is_none() && self.switched.bytes != self.baseline.bytes
    }

    pub fn byte_delta(&self) -> usize {
        self.switched
            .bytes
            .len()
            .abs_diff(self.baseline.bytes.len())
    }
}

/// `sha256:<hex>` of arbitrary bytes.  Public so the consumer test builds the
/// same `java_tei_sha256` the report carries without duplicating the format.
pub fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// A stable, `snake_case` token for a composer refusal.
pub fn error_token(error: &da_parse::NativeFulltextCompositionError) -> &'static str {
    use da_parse::NativeFulltextCompositionError as E;
    match error {
        E::InvalidIdentity(_) => "invalid_identity",
        E::WrongBackend => "wrong_backend",
        E::Oversized { .. } => "oversized",
        E::MissingAdmission(_) => "missing_admission",
        E::StaleAdmission(_) => "stale_admission",
        E::InvalidAdmission(_) => "invalid_admission",
        E::InvalidSchema(_) => "invalid_schema",
        E::MalformedXml => "malformed_xml",
        E::ForbiddenExternalEntity => "forbidden_external_entity",
        E::DuplicateId => "duplicate_id",
        E::UnbalancedXml => "unbalanced_xml",
        E::AmbiguousSplice(_) => "ambiguous_splice",
        E::BibliographyDomainMismatch(_) => "bibliography_domain_mismatch",
        E::BibliographyKeyCollision(_) => "bibliography_key_collision",
        E::UnresolvedBibliographyTarget(_) => "unresolved_bibliography_target",
        E::InconservableBibliographyChild(_) => "inconservable_bibliography_child",
        E::SemanticFieldLoss { .. } => "semantic_field_loss",
        E::ComposedOutputNotWellFormed(_) => "composed_output_not_well_formed",
        E::InvalidUtf8 => "invalid_utf8",
        E::ChangedReadingOrder => "changed_reading_order",
        E::ArtifactDigestMismatch { .. } => "artifact_digest_mismatch",
    }
}

fn receipt(
    component: FulltextComponent,
    disposition: AdmissionDisposition,
    evidence_state: &str,
    source_sha256: &str,
) -> FulltextAdmissionReceipt {
    FulltextAdmissionReceipt {
        component,
        producer: "S12".into(),
        disposition,
        evidence_state: evidence_state.into(),
        schema_version: "native-composition.outcome/v1".into(),
        source_sha256: source_sha256.into(),
        artifact_sha256: String::new(),
        fallback_target: NATIVE_FULLTEXT_FALLBACK_TARGET.into(),
    }
}

/// The seven admission receipts the composer requires, with the rehearsed lane's
/// disposition chosen by `header_disposition`.  The twin lane and the five
/// non-rehearsed components keep the declared `current_matrix` dispositions.
fn rehearsed_receipts(
    identity: &FulltextCompositionIdentity,
    header_disposition: AdmissionDisposition,
) -> Vec<FulltextAdmissionReceipt> {
    let source = identity.source_sha256.as_str();
    vec![
        receipt(
            FulltextComponent::Header,
            header_disposition,
            "passing",
            source,
        ),
        receipt(
            FulltextComponent::References,
            AdmissionDisposition::Fallback,
            "passing",
            source,
        ),
        receipt(
            FulltextComponent::DocumentSegmentation,
            AdmissionDisposition::Fallback,
            "invalid_evidence",
            source,
        ),
        receipt(
            FulltextComponent::Figure,
            AdmissionDisposition::Fallback,
            "unsupported",
            source,
        ),
        receipt(
            FulltextComponent::Table,
            AdmissionDisposition::Fallback,
            "not_observed",
            source,
        ),
        receipt(
            FulltextComponent::Formula,
            AdmissionDisposition::Fallback,
            "unsupported",
            source,
        ),
        receipt(
            FulltextComponent::LearningPolicy,
            AdmissionDisposition::NotObserved,
            "not_observed",
            source,
        ),
    ]
}

/// Compose one emission of the triad.
fn emission(
    java_tei: &[u8],
    identity: &FulltextCompositionIdentity,
    profile_fingerprint: &str,
    header: &HeaderCompositionOutcome,
    header_disposition: AdmissionDisposition,
    lane_states: LaneStates,
) -> RehearsalEmission {
    let input = NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            java_tei,
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, profile_fingerprint),
        ),
        identity: identity.clone(),
        header: header.clone(),
        // The references lane is not rehearsed in S06/T01: no native bibliography
        // band is produced, so the lane stays Java in every emission.  The
        // composer never inspects this value unless the lane is admitted.
        references: ReferenceCompositionOutcome::Rejected {
            error: da_parse::CompositionError::RejectedComponent {
                component: da_parse::CompositionComponent::References,
            },
        },
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: rehearsed_receipts(identity, header_disposition),
        s17_lineage: None,
    };
    match compose_native_fulltext(input) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => {
            let bytes = candidate.envelope.raw_tei().to_vec();
            RehearsalEmission {
                outcome: EmissionOutcome::Accepted,
                error_token: None,
                sha256: digest(&bytes),
                bytes,
                lane_states,
            }
        }
        NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
            let bytes = fallback.raw_tei().to_vec();
            RehearsalEmission {
                outcome: EmissionOutcome::Rejected,
                error_token: Some(error_token(&error).to_owned()),
                sha256: digest(&bytes),
                bytes,
                lane_states,
            }
        }
    }
}

/// A Java-baseline emission that carries a refused switch's token.
fn refused_emission(baseline: &RehearsalEmission, token: &str) -> RehearsalEmission {
    RehearsalEmission {
        outcome: EmissionOutcome::Refused,
        error_token: Some(token.to_owned()),
        sha256: baseline.sha256.clone(),
        bytes: baseline.bytes.clone(),
        lane_states: baseline.lane_states,
    }
}

/// A document's own subtree, or `None` when the element is absent.
fn subtree(document: &[u8], name: &str) -> Option<Vec<u8>> {
    tei_element_bytes(document, name).ok().map(<[u8]>::to_vec)
}

/// Run the triad on ONE input: `baseline` (lane on Java), `switched` (lane on the
/// native route, if admitted), `returned` (lane back on Java).
pub fn run_rehearsal_triad(
    java_tei: &[u8],
    identity: &FulltextCompositionIdentity,
    profile_fingerprint: &str,
    table_before: &DeclaredRouteTable,
    admitted: AdmittedLaneOutcome,
) -> TriadOutcome {
    let lane = REHEARSAL_LANE;
    let fallback_states = table_before.states();
    let header = match &admitted {
        AdmittedLaneOutcome::Admitted(outcome) => outcome.as_ref().clone(),
        // Nothing was produced, so nothing may be spliced; the placeholder is
        // never inspected because the lane is not admitted.
        AdmittedLaneOutcome::Refused(_) => HeaderCompositionOutcome::Rejected {
            error: da_parse::CompositionError::RejectedComponent {
                component: da_parse::CompositionComponent::Names,
            },
        },
    };

    let baseline = emission(
        java_tei,
        identity,
        profile_fingerprint,
        &header,
        AdmissionDisposition::Fallback,
        fallback_states,
    );

    let policy_switch = switch_lane(table_before, lane, LaneTarget::Native);
    let (switched, rehearsal_table, switch_refusal) = match (policy_switch, &admitted) {
        (Err(refusal), _) => (
            refused_emission(&baseline, refusal.token()),
            table_before.clone(),
            Some(SwitchRefusal::Policy(refusal)),
        ),
        (Ok(_), AdmittedLaneOutcome::Refused(refusal)) => (
            refused_emission(&baseline, refusal.token()),
            table_before.clone(),
            Some(SwitchRefusal::NativeSide(*refusal)),
        ),
        (Ok(next), AdmittedLaneOutcome::Admitted(_)) => (
            emission(
                java_tei,
                identity,
                profile_fingerprint,
                &header,
                AdmissionDisposition::Passed,
                fallback_states.with(lane, LaneState::Native),
            ),
            next,
            None,
        ),
    };

    let returned_table = switch_lane(&rehearsal_table, lane, LaneTarget::Java)
        .unwrap_or_else(|_| table_before.clone());
    let returned = emission(
        java_tei,
        identity,
        profile_fingerprint,
        &header,
        AdmissionDisposition::Fallback,
        returned_table.states(),
    );

    TriadOutcome {
        lane_movement: LaneMovement {
            header_changed: subtree(&baseline.bytes, "teiHeader")
                != subtree(&switched.bytes, "teiHeader"),
            references_changed: subtree(&baseline.bytes, "listBibl")
                != subtree(&switched.bytes, "listBibl"),
        },
        rollback: RollbackCheck {
            returned_equals_baseline: returned.sha256 == baseline.sha256,
            byte_identity: returned.bytes == baseline.bytes,
            table_returned_to_declared: returned_table == *table_before,
        },
        baseline,
        switched,
        returned,
        rehearsal_table,
        returned_table,
        switch_refusal,
    }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// The hermetic, dev-only fixture the stub substrate replays.
#[derive(Debug, Clone, Deserialize)]
pub struct Fixture {
    pub schema_version: String,
    pub case_id: String,
    pub source_sha256: String,
    pub profile_fingerprint: String,
    pub config_fingerprint: String,
    pub source_xml: String,
}

/// The declared fixture, parsed.
pub fn fixture() -> Fixture {
    serde_yaml::from_str(FIXTURE).expect("frozen fixture")
}

pub fn identity_for(fixture: &Fixture) -> FulltextCompositionIdentity {
    FulltextCompositionIdentity {
        source_sha256: fixture.source_sha256.clone(),
        profile_fingerprint: fixture.profile_fingerprint.clone(),
        config_fingerprint: fixture.config_fingerprint.clone(),
    }
}

/// The rehearsed lane's native band for the hermetic fixture.
///
/// Stub-level by design (S06/T01): the values below are deliberately unlike
/// anything the fixture's own `<teiHeader>` carries, so "the admitted band
/// reached the emission" is decided by the admitted band and never by conserved
/// source text.  T02 replaces this with a band built from the live document's
/// typed fields.
pub fn fixture_header_outcome() -> HeaderCompositionOutcome {
    const ADMITTED_TITLE: &str = "S06 rehearsal admitted title";
    const ADMITTED_EMAIL: &str = "s06-rehearsal@example.invalid";
    const ADMITTED_ID: &str = "S06-REHEARSAL-ID-0001";
    const ADMITTED_AFFILIATION_KEY: &str = "s06-rehearsal-aff-0";
    const ADMITTED_AFFILIATION: &str = "S06 Rehearsal Admitted University";
    compose_header_with_fields(
        HeaderCompositionInput {
            date: NativeDateParser::new().parse("2024-02-29"),
            authors: vec![HeaderAuthorInput {
                ordinal: 0,
                span: da_parse::CompositionSpan { start: 1, end: 8 },
                name: NativePersonalNameParser::new()
                    .parse(PersonalNameContext::Header, "J. Smith"),
                affiliations: NativeAffiliationLinkOutcome::Linked {
                    author: NativeAffiliationAuthorEvidence {
                        ordinal: 0,
                        span: AffiliationSpan { start: 0, end: 1 },
                    },
                    links: vec![NativeAffiliationLink {
                        key: ADMITTED_AFFILIATION_KEY.into(),
                        affiliation_ordinal: 0,
                        affiliation_span: AffiliationSpan {
                            start: 0,
                            end: ADMITTED_AFFILIATION.len(),
                        },
                    }],
                },
            }],
            affiliation_components: vec![
                NativeAffiliationParser::new().parse(ADMITTED_AFFILIATION),
            ],
        },
        NativeHeaderFields {
            title: vec![ADMITTED_TITLE.into()],
            email: vec![ADMITTED_EMAIL.into()],
            ids: vec![ADMITTED_ID.into()],
        },
    )
}

/// The declared fixture's Java TEI.  `identity.source_sha256` must equal its
/// sha256, which is asserted before any emission is composed.
pub fn fixture_java_tei(fixture: &Fixture) -> Vec<u8> {
    let bytes = fixture.source_xml.as_bytes().to_vec();
    let observed = digest(&bytes);
    let observed = observed.strip_prefix("sha256:").unwrap_or(&observed);
    assert_eq!(
        observed, fixture.source_sha256,
        "declared fixture sha256 and its bytes disagree"
    );
    bytes
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubstrateKind {
    LocalDeterministicStub,
    LiveGrobidHttp,
}

/// What the emissions were produced from.  T01 only has the hermetic stub; T02
/// fills the live fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubstrateDescriptor {
    pub kind: SubstrateKind,
    pub live_grobid_participated: bool,
    pub profile: String,
    /// A logical name, never a host or port.
    pub endpoint_scope: Option<String>,
    pub sidecar_version: Option<String>,
    pub release_digest: Option<String>,
}

impl SubstrateDescriptor {
    pub fn fixture_stub(profile: &str) -> Self {
        Self {
            kind: SubstrateKind::LocalDeterministicStub,
            live_grobid_participated: false,
            profile: profile.to_owned(),
            endpoint_scope: None,
            sidecar_version: None,
            release_digest: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RehearsalIdentity {
    pub candidate_id: String,
    pub candidate_version: String,
    pub rehearsed_lane: RehearsalLane,
    pub episode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehearsalDocument {
    pub document_id: String,
    pub java_tei_sha256: String,
    pub native_side: AdmittedLaneOutcome,
    pub triad: TriadOutcome,
}

/// Everything the report publishes; `main` builds exactly one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehearsalRun {
    pub identity: RehearsalIdentity,
    pub substrate: SubstrateDescriptor,
    pub table_before: DeclaredRouteTable,
    pub table_after: DeclaredRouteTable,
    pub documents: Vec<RehearsalDocument>,
}

fn emission_json(emission: &RehearsalEmission) -> serde_json::Value {
    serde_json::json!({
        "outcome": emission.outcome,
        "error_token": emission.error_token,
        "sha256": emission.sha256,
        "bytes": emission.byte_len(),
        "lane_states": {
            "header": emission.lane_states.header.token(),
            "references": emission.lane_states.references.token(),
        },
    })
}

fn table_json(table: &DeclaredRouteTable) -> serde_json::Value {
    serde_json::json!({
        "candidate_id": table.candidate_id,
        "candidate_version": table.candidate_version,
        "observation_mode": table.observation_mode,
        "promotion_state": table.promotion_state,
        "auto_promote": table.auto_promote,
        "lane_closure": table.lane_closure,
        "pooling": table.pooling,
        "terminal_receipt_is_pass": table.terminal_receipt_is_pass,
        "rust_run_may_not_cancel_fallback": table.rust_run_may_not_cancel_fallback,
        "fallback_target": table.fallback_target,
        "lanes": table.lanes.iter().map(|row| serde_json::json!({
            "lane": row.lane.token(),
            "state": row.state.token(),
            "rollback_target": row.rollback_target,
            "independently_selectable": row.independently_selectable,
            "admission_state": row.admission_state,
        })).collect::<Vec<_>>(),
        "carrier_sha256": table.carrier_sha256.iter()
            .map(|(rel, digest)| (rel.clone(), serde_json::Value::String(digest.clone())))
            .collect::<serde_json::Map<String, serde_json::Value>>(),
    })
}

fn native_side_json(side: &AdmittedLaneOutcome) -> serde_json::Value {
    match side {
        AdmittedLaneOutcome::Admitted(_) => serde_json::json!({ "outcome": "admitted" }),
        AdmittedLaneOutcome::Refused(refusal) => serde_json::json!({
            "outcome": "refused",
            "reason": refusal.token(),
        }),
    }
}

fn document_json(document: &RehearsalDocument) -> serde_json::Value {
    let triad = &document.triad;
    serde_json::json!({
        "document_id": document.document_id,
        "java_tei_sha256": document.java_tei_sha256,
        "native_side": native_side_json(&document.native_side),
        "baseline": emission_json(&triad.baseline),
        "switched": emission_json(&triad.switched),
        "returned": emission_json(&triad.returned),
        "lane_movement": {
            "header_changed": triad.lane_movement.header_changed,
            "references_changed": triad.lane_movement.references_changed,
        },
        "rollback": {
            "returned_equals_baseline": triad.rollback.returned_equals_baseline,
            "byte_identity": triad.rollback.byte_identity,
            "table_returned_to_declared": triad.rollback.table_returned_to_declared,
        },
        "switch_refusal": triad.switch_refusal.map(SwitchRefusal::token),
    })
}

/// Build the deterministic rehearsal report.  Pure: two calls on the same run
/// serialize to byte-equal JSON.
pub fn rehearsal_report(run: &RehearsalRun) -> serde_json::Value {
    let documents_total = run.documents.len();
    let documents_switched = run
        .documents
        .iter()
        .filter(|document| document.triad.switched_performed())
        .count();
    let documents_refused = run
        .documents
        .iter()
        .filter(|document| document.triad.switch_refusal.is_some())
        .count();
    let switched_byte_delta_total: usize = run
        .documents
        .iter()
        .map(|document| document.triad.byte_delta())
        .sum();
    let references_never_moved = run
        .documents
        .iter()
        .all(|document| !document.triad.lane_movement.references_changed);
    let rollback_byte_identical_all = run
        .documents
        .iter()
        .all(|document| document.triad.rollback.byte_identity);
    let switch_and_return_non_vacuous =
        documents_switched >= 1 && rollback_byte_identical_all && references_never_moved;
    let carriers_unchanged = run.table_before.carrier_sha256 == run.table_after.carrier_sha256;

    serde_json::json!({
        "schema_version": REHEARSAL_SCHEMA_VERSION,
        "claim_scope": CLAIM_SCOPE,
        "pooled_quality_claim": POOLED_QUALITY_CLAIM,
        "identity": {
            "candidate_id": run.identity.candidate_id,
            "candidate_version": run.identity.candidate_version,
            "rehearsed_lane": run.identity.rehearsed_lane.token(),
            "episode": run.identity.episode,
        },
        "substrate": run.substrate,
        "declared_route_table_before": table_json(&run.table_before),
        "declared_route_table_after": table_json(&run.table_after),
        "rehearsal_table": table_json(&run.documents.first()
            .map(|document| document.triad.rehearsal_table.clone())
            .unwrap_or_else(|| run.table_before.clone())),
        "documents": run.documents.iter().map(document_json).collect::<Vec<_>>(),
        "rehearsal": {
            "documents_total": documents_total,
            "documents_switched": documents_switched,
            "documents_refused": documents_refused,
            "switched_byte_delta_total": switched_byte_delta_total,
            "references_never_moved": references_never_moved,
            "rollback_byte_identical_all": rollback_byte_identical_all,
            "switch_and_return_non_vacuous": switch_and_return_non_vacuous,
            "carriers_unchanged": carriers_unchanged,
        },
        "named_gaps": {
            "java_header_raw": NAMED_GAP_JAVA_HEADER_RAW,
            "native_accepted_header": NAMED_GAP_NATIVE_ACCEPTED_HEADER,
            "pooling": NATIVE_FULLTEXT_LANE_FALLBACK_POOLING,
        },
        "locks": {
            "deployed": false,
            "import_eligible": false,
            "production_route": "forbidden",
            "graph_write": "forbidden",
            "auto_promote": "forbidden",
            "promotion_receipt": serde_json::Value::Null,
            "this_wave_promotes": "none",
            "observation_mode": "shadow",
            "promotion_state": "candidate",
            "explicit_decision": serde_json::Value::Null,
        },
        "redaction": {
            "physical_paths_published": false,
            "raw_tei_published": false,
        },
    })
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[cfg(not(test))]
fn main() {
    let Some(root) = default_root() else {
        eprintln!("native-family-promotion-rehearsal: declared route carriers not found");
        std::process::exit(2);
    };
    let table_before = match read_declared_route_table(&root) {
        Ok(table) => table,
        Err(refusal) => {
            eprintln!(
                "native-family-promotion-rehearsal: declared route table refused: {}",
                refusal.token()
            );
            std::process::exit(2);
        }
    };

    let declared = fixture();
    let identity = identity_for(&declared);
    let java_tei = fixture_java_tei(&declared);
    let triad = run_rehearsal_triad(
        &java_tei,
        &identity,
        &identity.profile_fingerprint,
        &table_before,
        AdmittedLaneOutcome::Admitted(Box::new(fixture_header_outcome())),
    );

    // The carriers are re-read from disk AFTER the run: an executed statement of
    // immutability, not a static declaration.
    let table_after = read_declared_route_table(&root).unwrap_or_else(|refusal| {
        eprintln!(
            "native-family-promotion-rehearsal: carriers drifted: {}",
            refusal.token()
        );
        std::process::exit(2);
    });

    let run = RehearsalRun {
        identity: RehearsalIdentity {
            candidate_id: table_before.candidate_id.clone(),
            candidate_version: table_before.candidate_version.clone(),
            rehearsed_lane: REHEARSAL_LANE,
            episode: REHEARSAL_EPISODE.to_owned(),
        },
        substrate: SubstrateDescriptor::fixture_stub(&identity.profile_fingerprint),
        table_before: table_before.clone(),
        table_after: table_after.clone(),
        documents: vec![RehearsalDocument {
            document_id: declared.case_id.clone(),
            java_tei_sha256: digest(&java_tei),
            native_side: AdmittedLaneOutcome::Admitted(Box::new(fixture_header_outcome())),
            triad: triad.clone(),
        }],
    };

    let report = rehearsal_report(&run);
    let first = serde_json::to_string(&report).expect("report serializes");
    let second = serde_json::to_string(&rehearsal_report(&run)).expect("report serializes");
    if first != second {
        eprintln!("native-family-promotion-rehearsal: report is not deterministic");
        std::process::exit(2);
    }

    let carriers_unchanged = table_before.carrier_sha256 == table_after.carrier_sha256;
    println!(
        "route-table: candidate={} version={} lane={} observation_mode={} promotion_state={} \
         auto_promote={} pooling={} rust_run_may_not_cancel_fallback={} carriers_unchanged={}",
        table_before.candidate_id,
        table_before.candidate_version,
        REHEARSAL_LANE.token(),
        table_before.observation_mode,
        table_before.promotion_state,
        table_before.auto_promote,
        table_before.pooling,
        table_before.rust_run_may_not_cancel_fallback,
        carriers_unchanged,
    );
    println!(
        "document {}: baseline={}({}, {} bytes) switched={}({}, {} bytes) \
         returned={}({}, {} bytes) header_changed={} references_changed={} \
         rollback_byte_identical={} table_returned_to_declared={} refusal={}",
        declared.case_id,
        triad.baseline.outcome_token(),
        triad.baseline.sha256,
        triad.baseline.byte_len(),
        triad.switched.outcome_token(),
        triad.switched.sha256,
        triad.switched.byte_len(),
        triad.returned.outcome_token(),
        triad.returned.sha256,
        triad.returned.byte_len(),
        triad.lane_movement.header_changed,
        triad.lane_movement.references_changed,
        triad.rollback.byte_identity,
        triad.rollback.table_returned_to_declared,
        triad
            .switch_refusal
            .map(SwitchRefusal::token)
            .unwrap_or("none"),
    );
    println!(
        "rehearsal: documents_total={} documents_switched={} documents_refused={} \
         rollback_byte_identical_all={} references_never_moved={} \
         switch_and_return_non_vacuous={}",
        report["rehearsal"]["documents_total"],
        report["rehearsal"]["documents_switched"],
        report["rehearsal"]["documents_refused"],
        report["rehearsal"]["rollback_byte_identical_all"],
        report["rehearsal"]["references_never_moved"],
        report["rehearsal"]["switch_and_return_non_vacuous"],
    );

    // Fail closed: a rehearsal that did not switch, did not return, or that
    // rewrote a carrier is not evidence of anything.
    let vacuous = !triad.switched_performed()
        || !triad.rollback.byte_identity
        || !triad.rollback.table_returned_to_declared
        || !carriers_unchanged;
    if vacuous {
        eprintln!("native-family-promotion-rehearsal: rehearsal is vacuous or unsafe");
        std::process::exit(2);
    }
}
