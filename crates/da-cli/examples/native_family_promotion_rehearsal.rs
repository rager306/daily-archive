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
//!
//! S06/T02 adds the DECLARED SUBSTRATE harness on top of that primitive.  The
//! default substrate is the hermetic stub (the declared dev fixture of T01, no
//! network, no repository artifact); the opt-in `live-grobid` substrate sends
//! one `structural`-profile `POST processFulltextDocument` per document of the
//! pinned `layout_bakeoff.yaml#corpus`, verifies every PDF sha256 BEFORE it is
//! sent, checks the live sidecar against the manifest's `declared_release`, and
//! builds the native header band from the real `<teiHeader>` carriers through
//! the public typed parsers.  A document whose band cannot be admitted is a
//! typed per-document refusal, never an invented value and never a run error.
//!
//! The report is closed (`native-family-promotion-rehearsal.v1`), atomically
//! written, redaction-checked, and guarded by a fail-closed validator that
//! `--self-check` proves can fire on corrupted copies.  `--self-check` is
//! hermetic and writes nothing.
//!
//! Scoped claim, stated in the report itself: `claim_scope = rehearsal_only`
//! and `pooled_quality_claim = forbidden`.  A rehearsal is not a parity
//! measurement, and the numbers it produces must never be pooled into one.

use da_parse::{
    AdmissionDisposition, AffiliationSpan, FulltextAdmissionReceipt, FulltextComponent,
    FulltextCompositionIdentity, GROBID_HTTP_BACKEND_ID, GrobidHttpBackend, GrobidProfile,
    HeaderAuthorInput, HeaderCompositionInput, HeaderCompositionOutcome,
    NATIVE_FULLTEXT_FALLBACK_TARGET, NATIVE_FULLTEXT_LANE_FALLBACK_LANE_CLOSURE,
    NATIVE_FULLTEXT_LANE_FALLBACK_POOLING, NATIVE_FULLTEXT_LANE_FALLBACK_RUST_RUN_MAY_NOT_CANCEL,
    NativeAffiliationAuthorEvidence, NativeAffiliationLink, NativeAffiliationLinkOutcome,
    NativeAffiliationOutcome, NativeAffiliationParser, NativeDateParser,
    NativeFulltextCompositionInput, NativeFulltextCompositionOutcome, NativeHeaderFields,
    NativePersonalNameParser, ParseBackend, PersonalNameContext, ReferenceCompositionOutcome,
    TeiEnvelope, TeiIdentity, compose_header_with_fields, compose_native_fulltext,
    tei_element_bytes,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

/// Default report for the hermetic stub substrate.  `target/` is not part of
/// the committed artifact population, so a stub run can never look like the
/// committed live artifact.
pub const DEFAULT_REPORT_STUB: &str = "target/native_family_promotion_rehearsal_stub.json";
/// Default report for the live Java GROBID substrate (committed in T03).
pub const DEFAULT_REPORT_LIVE: &str =
    "artifacts/parse_compat/native_family_promotion_rehearsal.json";

/// Declared live inputs.  The endpoint is a CLI default and is NEVER published:
/// the report carries the logical [`ENDPOINT_SCOPE`] instead of a host or port.
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8070";
pub const DEFAULT_CORPUS: &str = "data/parse/layout_bakeoff.yaml";
pub const DEFAULT_MANIFEST: &str = "data/parse/acceptance_manifest.yaml";

/// The 96-page document of the declared corpus costs roughly 62s under the
/// structural profile, so the bounded interactive default is far too tight.
pub const LIVE_REQUEST_TIMEOUT_SECS: u64 = 900;
/// The live profile is `structural` (D204): no Crossref consolidation wait can
/// land inside the rehearsal window.
pub const LIVE_PROFILE_NAME: &str = "structural";
/// The logical (never physical) name of the declared corpus.
pub const CORPUS_SCOPE: &str = "layout_bakeoff_declared_corpus";
/// The logical (never host or port) name of the live sidecar.
pub const ENDPOINT_SCOPE: &str = "declared_local_sidecar";
/// Why the stub's per-document record carries no PDF digest: nothing was sent.
pub const STUB_PDF_ABSENT_REASON: &str = "the hermetic stub substrate replays a declared dev fixture, so no corpus PDF was sent and no PDF digest exists";

/// The closed vocabulary of guards `self_check` must prove it can fire.  The
/// consumer test asserts the fired set EQUALS its own literal copy (S04 shipped
/// two parallel lists that had drifted 11 vs 10).
pub const REQUIRED_SELF_CHECK_GUARDS: [&str; 15] = [
    "rehearsed-lane-not-independently-selectable",
    "pooling-in-rehearsal",
    "rust-run-cancelled-fallback",
    "missing-rollback-target",
    "returned-emission-differs-from-baseline",
    "lane-movement-outside-rehearsed-lane",
    "zero-switched-documents-is-vacuous",
    "policy-lock-lifted",
    "carrier-sha256-changed-after-run",
    "promotion-receipt-present",
    "explicit-decision-present",
    "forbidden-token-in-report",
    "native-side-admitted-without-switched-difference",
    "native-side-refused-without-reason",
    "measurement-scope-absent",
];

/// Redaction denylist for the SERIALIZED report (defence in depth; the primary
/// control is that no producer ever writes these).  Mirror of the S04 list.
pub const FORBIDDEN_REPORT_TOKENS: [&str; 15] = [
    "<TEI",
    "<tei",
    "%PDF-",
    "data/sources",
    "file://",
    "/root/",
    "/home/",
    "target/debug",
    "target/release",
    "executable",
    "hostname",
    "/sys/",
    "docker-",
    ".scope",
    "daily-archive-grobid",
];

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
    /// The live Java TEI carries no `<teiHeader>` element at all.
    LiveTeiHeaderAbsent,
    /// The live Java TEI is not UTF-8, so no header carrier can be read.
    LiveTeiNotUtf8,
    /// The real `<teiHeader>` declares no dated carrier (MEM1786: a structural
    /// run does not guarantee a header date), so the band has no date atom.
    HeaderDateAbsent,
    /// The real `<titleStmt>` admits no personal-name carrier.
    HeaderAuthorAbsent,
}

impl NativeSideRefusal {
    pub const fn token(self) -> &'static str {
        match self {
            Self::NativeOutcomeRejected => "native_outcome_rejected",
            Self::NativeOutcomeAbsent => "native_outcome_absent",
            Self::LiveTeiHeaderAbsent => "live_tei_header_absent",
            Self::LiveTeiNotUtf8 => "live_tei_not_utf8",
            Self::HeaderDateAbsent => "header_date_absent",
            Self::HeaderAuthorAbsent => "header_author_absent",
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
    /// A logical name, never a physical corpus path.
    pub corpus_scope: Option<String>,
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
            corpus_scope: None,
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
    /// The digest of the PDF that was SENT, on the live substrate only.  A PDF
    /// digest and a TEI digest are never the same value and are never mixed.
    pub source_pdf_sha256: Option<String>,
    /// Why `source_pdf_sha256` is absent, when it is (stub substrate only).
    pub source_pdf_sha256_absent_reason: Option<String>,
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
        "explicit_decision": serde_json::Value::Null,
        "carrier_sha256": table.carrier_sha256.iter()
            .map(|(rel, digest)| (rel.clone(), serde_json::Value::String(digest.clone())))
            .collect::<serde_json::Map<String, serde_json::Value>>(),
    })
}

fn native_side_json(side: &AdmittedLaneOutcome) -> serde_json::Value {
    match side {
        AdmittedLaneOutcome::Admitted(_) => {
            serde_json::json!({ "outcome": "admitted", "reason": serde_json::Value::Null })
        }
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
        "source_pdf_sha256": document.source_pdf_sha256,
        "source_pdf_sha256_absent_reason": document.source_pdf_sha256_absent_reason,
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
        "rehearsal_table": rehearsal_table_json(run),
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
        // The closed vocabulary `--self-check` proves it can fire, published in
        // the artifact so the schema and the self-check cannot drift apart.
        "guards": {
            "fail_closed": REQUIRED_SELF_CHECK_GUARDS,
        },
    })
}

// ---------------------------------------------------------------------------
// Fail-closed validation and self-check
// ---------------------------------------------------------------------------

/// Admissibility of a rehearsal report.  `Err` carries a stable, greppable
/// finding token; `--self-check` proves every token can fire.
pub fn validate_report(report: &serde_json::Value) -> Result<(), String> {
    // ── scope: the report must say what it may NOT be read as ───────────
    match report
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
    {
        Some(REHEARSAL_SCHEMA_VERSION) => {}
        Some(other) => return Err(format!("schema-version-drift:{other}")),
        None => return Err("schema-version-absent".to_string()),
    }
    match report
        .get("claim_scope")
        .and_then(serde_json::Value::as_str)
    {
        Some(CLAIM_SCOPE) => {}
        Some(other) => return Err(format!("measurement-scope-drift:{other}")),
        None => return Err("measurement-scope-absent".to_string()),
    }
    match report
        .get("pooled_quality_claim")
        .and_then(serde_json::Value::as_str)
    {
        Some(POOLED_QUALITY_CLAIM) => {}
        Some(other) => return Err(format!("pooled-quality-claim-drift:{other}")),
        None => return Err("pooled-quality-claim-absent".to_string()),
    }

    let identity = report
        .get("identity")
        .and_then(serde_json::Value::as_object)
        .ok_or("identity-absent")?;
    if identity
        .get("rehearsed_lane")
        .and_then(serde_json::Value::as_str)
        != Some(REHEARSAL_LANE.token())
    {
        return Err("rehearsed-lane-drift".to_string());
    }
    if identity.get("episode").and_then(serde_json::Value::as_str) != Some(REHEARSAL_EPISODE) {
        return Err("episode-drift".to_string());
    }

    let before = report
        .get("declared_route_table_before")
        .and_then(serde_json::Value::as_object)
        .ok_or("declared-route-table-before-absent")?;
    let after = report
        .get("declared_route_table_after")
        .and_then(serde_json::Value::as_object)
        .ok_or("declared-route-table-after-absent")?;

    // ── the declared table keeps every policy lock, on BOTH sides ───────
    for (side, table) in [("before", before), ("after", after)] {
        let lanes = table
            .get("lanes")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("declared-route-table-{side}-lanes-absent"))?;
        if lanes.len() != 2 {
            return Err(format!("declared-lane-count-drift:{side}"));
        }
        for lane in lanes {
            let lane = lane
                .as_object()
                .ok_or_else(|| format!("declared-lane-not-object:{side}"))?;
            if lane.get("independently_selectable") != Some(&serde_json::Value::Bool(true)) {
                return Err("rehearsed-lane-not-independently-selectable".to_string());
            }
            match lane
                .get("rollback_target")
                .and_then(serde_json::Value::as_str)
            {
                Some("") | None => return Err("missing-rollback-target".to_string()),
                Some(target) if target != NATIVE_FULLTEXT_FALLBACK_TARGET => {
                    return Err(format!("rollback-target-drift:{target}"));
                }
                Some(_) => {}
            }
        }
        if table.get("pooling").and_then(serde_json::Value::as_str)
            != Some(NATIVE_FULLTEXT_LANE_FALLBACK_POOLING)
        {
            return Err("pooling-in-rehearsal".to_string());
        }
        if table.get("rust_run_may_not_cancel_fallback") != Some(&serde_json::Value::Bool(true)) {
            return Err("rust-run-cancelled-fallback".to_string());
        }
        if table
            .get("auto_promote")
            .and_then(serde_json::Value::as_str)
            != Some("forbidden")
        {
            return Err("auto-promote-in-rehearsal".to_string());
        }
        // No explicit promotion decision may ride along inside a declared route
        // table: the switch stays in-memory and undeclared-to-prod.
        if !matches!(table.get("explicit_decision"), Some(value) if value.is_null()) {
            return Err("explicit-decision-present".to_string());
        }
    }

    // ── the carriers were not rewritten: an executed, not declared, claim ─
    let before_carriers = before
        .get("carrier_sha256")
        .and_then(serde_json::Value::as_object)
        .ok_or("carrier-sha256-absent")?;
    let after_carriers = after
        .get("carrier_sha256")
        .and_then(serde_json::Value::as_object)
        .ok_or("carrier-sha256-absent")?;
    for digest in after_carriers.values() {
        let text = digest.as_str().ok_or("carrier-sha256-not-a-string")?;
        if !(text.starts_with("sha256:") && text.len() == "sha256:".len() + 64) {
            return Err("carrier-sha256-shape".to_string());
        }
    }
    if before_carriers != after_carriers {
        return Err("carrier-sha256-changed-after-run".to_string());
    }
    if before != after {
        return Err("declared-route-table-changed-after-run".to_string());
    }

    // ── the in-memory rehearsal table moved EXACTLY the rehearsed lane ──
    let rehearsal_table = report
        .get("rehearsal_table")
        .and_then(serde_json::Value::as_object)
        .ok_or("rehearsal-table-absent")?;
    if rehearsal_table.get("equality_with_declared_after") != Some(&serde_json::Value::Bool(true)) {
        return Err("rehearsal-table-drift".to_string());
    }
    let declared_lanes = after
        .get("lanes")
        .and_then(serde_json::Value::as_array)
        .ok_or("declared-route-table-after-lanes-absent")?;
    let rehearsal_lanes = rehearsal_table
        .get("lanes")
        .and_then(serde_json::Value::as_array)
        .ok_or("rehearsal-table-lanes-absent")?;
    if declared_lanes.len() != rehearsal_lanes.len() {
        return Err("lane-movement-outside-rehearsed-lane".to_string());
    }
    for (declared_lane, rehearsal_lane) in declared_lanes.iter().zip(rehearsal_lanes.iter()) {
        let lane = declared_lane
            .get("lane")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let expected = if lane == REHEARSAL_LANE.token() {
            "native"
        } else {
            "java"
        };
        if rehearsal_lane
            .get("state")
            .and_then(serde_json::Value::as_str)
            != Some(expected)
        {
            return Err("lane-movement-outside-rehearsed-lane".to_string());
        }
    }

    // ── per document: the band moved where it may, and returned byte-equal ─
    let documents = report
        .get("documents")
        .and_then(serde_json::Value::as_array)
        .ok_or("documents-absent")?;
    if documents.is_empty() {
        return Err("zero-switched-documents-is-vacuous".to_string());
    }
    for document in documents {
        let document = document.as_object().ok_or("document-not-object")?;
        let movement = document
            .get("lane_movement")
            .and_then(serde_json::Value::as_object)
            .ok_or("document-lane-movement-absent")?;
        if movement.get("references_changed") != Some(&serde_json::Value::Bool(false)) {
            return Err("lane-movement-outside-rehearsed-lane".to_string());
        }
        let native_side = document
            .get("native_side")
            .and_then(serde_json::Value::as_object)
            .ok_or("native-side-absent")?;
        match native_side
            .get("outcome")
            .and_then(serde_json::Value::as_str)
        {
            Some("admitted") => {
                if movement.get("header_changed") != Some(&serde_json::Value::Bool(true)) {
                    return Err("native-side-admitted-without-switched-difference".to_string());
                }
            }
            Some("refused") => {
                let reason = native_side
                    .get("reason")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                if reason.is_empty() {
                    return Err("native-side-refused-without-reason".to_string());
                }
            }
            other => return Err(format!("native-side-outcome-drift:{other:?}")),
        }
        let rollback = document
            .get("rollback")
            .and_then(serde_json::Value::as_object)
            .ok_or("document-rollback-absent")?;
        if rollback.get("byte_identity") != Some(&serde_json::Value::Bool(true)) {
            return Err("returned-emission-differs-from-baseline".to_string());
        }
        if rollback.get("table_returned_to_declared") != Some(&serde_json::Value::Bool(true)) {
            return Err("rollback-table-not-returned".to_string());
        }
        let baseline = document
            .get("baseline")
            .and_then(serde_json::Value::as_object)
            .ok_or("document-baseline-absent")?;
        let returned = document
            .get("returned")
            .and_then(serde_json::Value::as_object)
            .ok_or("document-returned-absent")?;
        if baseline.get("sha256") != returned.get("sha256") {
            return Err("returned-emission-differs-from-baseline".to_string());
        }
    }

    // ── the aggregate must be non-vacuous and lane-closed ───────────────
    let rehearsal = report
        .get("rehearsal")
        .and_then(serde_json::Value::as_object)
        .ok_or("rehearsal-aggregate-absent")?;
    if rehearsal
        .get("documents_switched")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        < 1
    {
        return Err("zero-switched-documents-is-vacuous".to_string());
    }
    if rehearsal.get("rollback_byte_identical_all") != Some(&serde_json::Value::Bool(true)) {
        return Err("rollback-not-byte-identical-all".to_string());
    }
    if rehearsal.get("references_never_moved") != Some(&serde_json::Value::Bool(true)) {
        return Err("lane-movement-outside-rehearsed-lane".to_string());
    }
    if rehearsal.get("switch_and_return_non_vacuous") != Some(&serde_json::Value::Bool(true)) {
        return Err("rehearsal-vacuous".to_string());
    }
    if rehearsal.get("carriers_unchanged") != Some(&serde_json::Value::Bool(true)) {
        return Err("carrier-sha256-changed-after-run".to_string());
    }

    // ── the named gaps stay visible (never pooled away) ─────────────────
    let gaps = report
        .get("named_gaps")
        .and_then(serde_json::Value::as_object)
        .ok_or("named-gaps-absent")?;
    if gaps
        .get("java_header_raw")
        .and_then(serde_json::Value::as_u64)
        != Some(NAMED_GAP_JAVA_HEADER_RAW)
    {
        return Err("named-gap-drift:java_header_raw".to_string());
    }
    if gaps
        .get("native_accepted_header")
        .and_then(serde_json::Value::as_u64)
        != Some(NAMED_GAP_NATIVE_ACCEPTED_HEADER)
    {
        return Err("named-gap-drift:native_accepted_header".to_string());
    }

    // ── every policy lock, and no promotion artefact ────────────────────
    let locks = report
        .get("locks")
        .and_then(serde_json::Value::as_object)
        .ok_or("locks-absent")?;
    if locks.get("deployed") != Some(&serde_json::Value::Bool(false)) {
        return Err("policy-lock-lifted:deployed".to_string());
    }
    if locks.get("import_eligible") != Some(&serde_json::Value::Bool(false)) {
        return Err("policy-lock-lifted:import_eligible".to_string());
    }
    for key in ["production_route", "graph_write", "auto_promote"] {
        if locks.get(key).and_then(serde_json::Value::as_str) != Some("forbidden") {
            return Err(format!("policy-lock-lifted:{key}"));
        }
    }
    if locks
        .get("this_wave_promotes")
        .and_then(serde_json::Value::as_str)
        != Some("none")
    {
        return Err("policy-lock-lifted:this_wave_promotes".to_string());
    }
    if !matches!(locks.get("promotion_receipt"), Some(value) if value.is_null()) {
        return Err("promotion-receipt-present".to_string());
    }
    if !matches!(locks.get("explicit_decision"), Some(value) if value.is_null()) {
        return Err("explicit-decision-present".to_string());
    }

    // ── redaction markers and the serialized denylist scan ──────────────
    let redaction = report
        .get("redaction")
        .and_then(serde_json::Value::as_object)
        .ok_or("redaction-absent")?;
    if redaction.is_empty() {
        return Err("redaction-empty".to_string());
    }
    for (key, value) in redaction {
        if value != &serde_json::Value::Bool(false) {
            return Err(format!("redaction-marker-not-false:{key}"));
        }
    }

    // ── the report publishes the closed fail-closed guard vocabulary ────
    // The same vocabulary `--self-check` proves it can fire, so the artifact's
    // schema and the self-check expectation cannot drift apart unnoticed.
    let guards = report
        .get("guards")
        .and_then(serde_json::Value::as_object)
        .ok_or("guards-absent")?;
    let published_guards = guards
        .get("fail_closed")
        .and_then(serde_json::Value::as_array)
        .ok_or("guards-fail-closed-absent")?;
    let mut published_guards: Vec<&str> = published_guards
        .iter()
        .map(|value| value.as_str().ok_or("guards-name-not-a-string"))
        .collect::<Result<_, _>>()?;
    published_guards.sort_unstable();
    let mut required_guards: Vec<&str> = REQUIRED_SELF_CHECK_GUARDS.to_vec();
    required_guards.sort_unstable();
    if published_guards != required_guards {
        return Err("guards-vocabulary-drift".to_string());
    }

    // ── the substrate block is typed and self-consistent ────────────────
    let substrate = report
        .get("substrate")
        .and_then(serde_json::Value::as_object)
        .ok_or("substrate-absent")?;
    match substrate.get("kind").and_then(serde_json::Value::as_str) {
        Some("local_deterministic_stub") => {
            if substrate.get("live_grobid_participated") != Some(&serde_json::Value::Bool(false)) {
                return Err("stub-claims-live-participation".to_string());
            }
        }
        Some("live_grobid_http") => {
            if substrate.get("live_grobid_participated") != Some(&serde_json::Value::Bool(true)) {
                return Err("live-substrate-without-participation".to_string());
            }
            if substrate.get("profile").and_then(serde_json::Value::as_str)
                != Some(LIVE_PROFILE_NAME)
            {
                return Err("live-substrate-profile-drift".to_string());
            }
            if substrate
                .get("sidecar_version")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .is_empty()
            {
                return Err("live-substrate-sidecar-version-absent".to_string());
            }
        }
        other => return Err(format!("substrate-kind-drift:{other:?}")),
    }

    let text = serde_json::to_string(report)
        .map_err(|error| format!("report-serialize-failed:{error}"))?;
    for token in FORBIDDEN_REPORT_TOKENS {
        if text.contains(token) {
            return Err(format!("forbidden-token-in-report:{token}"));
        }
    }

    Ok(())
}

type Mutation = (&'static str, fn(&mut serde_json::Value), &'static str);

/// Every guard, as a deliberate corruption of the REAL report plus the finding
/// token the validator must produce.  The list is the executable definition of
/// [`REQUIRED_SELF_CHECK_GUARDS`], in evaluation order.
fn self_check_mutations() -> Vec<Mutation> {
    vec![
        (
            "rehearsed-lane-not-independently-selectable",
            |report: &mut serde_json::Value| {
                report["declared_route_table_after"]["lanes"][0]["independently_selectable"] =
                    serde_json::Value::Bool(false);
            },
            "rehearsed-lane-not-independently-selectable",
        ),
        (
            "pooling-in-rehearsal",
            |report: &mut serde_json::Value| {
                report["declared_route_table_after"]["pooling"] =
                    serde_json::Value::String("allowed".to_string());
            },
            "pooling-in-rehearsal",
        ),
        (
            "rust-run-cancelled-fallback",
            |report: &mut serde_json::Value| {
                report["declared_route_table_after"]["rust_run_may_not_cancel_fallback"] =
                    serde_json::Value::Bool(false);
            },
            "rust-run-cancelled-fallback",
        ),
        (
            "missing-rollback-target",
            |report: &mut serde_json::Value| {
                report["declared_route_table_after"]["lanes"][0]["rollback_target"] =
                    serde_json::Value::String(String::new());
            },
            "missing-rollback-target",
        ),
        (
            "returned-emission-differs-from-baseline",
            |report: &mut serde_json::Value| {
                report["documents"][0]["rollback"]["byte_identity"] =
                    serde_json::Value::Bool(false);
            },
            "returned-emission-differs-from-baseline",
        ),
        (
            "lane-movement-outside-rehearsed-lane",
            |report: &mut serde_json::Value| {
                report["documents"][0]["lane_movement"]["references_changed"] =
                    serde_json::Value::Bool(true);
            },
            "lane-movement-outside-rehearsed-lane",
        ),
        (
            "zero-switched-documents-is-vacuous",
            |report: &mut serde_json::Value| {
                report["rehearsal"]["documents_switched"] = serde_json::Value::from(0_u64);
            },
            "zero-switched-documents-is-vacuous",
        ),
        (
            "policy-lock-lifted",
            |report: &mut serde_json::Value| {
                report["locks"]["deployed"] = serde_json::Value::Bool(true);
            },
            "policy-lock-lifted",
        ),
        (
            "carrier-sha256-changed-after-run",
            |report: &mut serde_json::Value| {
                let changed = format!("sha256:{}", "0".repeat(64));
                report["declared_route_table_after"]["carrier_sha256"][DATA_COMPOSITION] =
                    serde_json::Value::String(changed);
            },
            "carrier-sha256-changed-after-run",
        ),
        (
            "promotion-receipt-present",
            |report: &mut serde_json::Value| {
                report["locks"]["promotion_receipt"] = serde_json::json!({ "receipt": "injected" });
            },
            "promotion-receipt-present",
        ),
        (
            "explicit-decision-present",
            |report: &mut serde_json::Value| {
                let injected = serde_json::json!({ "decision": "injected" });
                report["locks"]["explicit_decision"] = injected.clone();
                report["declared_route_table_after"]["explicit_decision"] = injected;
            },
            "explicit-decision-present",
        ),
        (
            "forbidden-token-in-report",
            |report: &mut serde_json::Value| {
                report["substrate"]["note"] =
                    serde_json::Value::String("leaked file:///root/private".to_string());
            },
            "forbidden-token-in-report",
        ),
        (
            "native-side-admitted-without-switched-difference",
            |report: &mut serde_json::Value| {
                report["documents"][0]["native_side"]["outcome"] =
                    serde_json::Value::String("admitted".to_string());
                report["documents"][0]["lane_movement"]["header_changed"] =
                    serde_json::Value::Bool(false);
            },
            "native-side-admitted-without-switched-difference",
        ),
        (
            "native-side-refused-without-reason",
            |report: &mut serde_json::Value| {
                report["documents"][0]["native_side"] = serde_json::json!({ "outcome": "refused" });
            },
            "native-side-refused-without-reason",
        ),
        (
            "measurement-scope-absent",
            |report: &mut serde_json::Value| {
                if let Some(object) = report.as_object_mut() {
                    object.remove("claim_scope");
                }
            },
            "measurement-scope-absent",
        ),
    ]
}

/// Prove every fail-closed guard fires on a deliberately corrupted copy of the
/// REAL (hermetic stub) report, and return the guard names that fired.  Nothing
/// is written: this is the `--self-check` entry point.
pub fn self_check() -> Result<Vec<&'static str>, String> {
    let root =
        default_root().ok_or_else(|| "the declared route carriers were not found".to_string())?;
    let run = stub_run(&root)?;
    let report = rehearsal_report(&run);
    validate_report(&report).map_err(|finding| {
        format!("the stub report was rejected by its own validator: {finding}")
    })?;

    let mut fired: Vec<&'static str> = Vec::new();
    for (name, mutate, expected) in self_check_mutations() {
        let mut corrupted = report.clone();
        mutate(&mut corrupted);
        match validate_report(&corrupted) {
            Ok(()) => return Err(format!("guard `{name}` did not fire on its corrupted copy")),
            Err(finding) if finding.contains(expected) => fired.push(name),
            Err(finding) => {
                return Err(format!(
                    "guard `{name}` produced `{finding}`, expected `{expected}`"
                ));
            }
        }
    }

    let fired_set: std::collections::BTreeSet<&str> = fired.iter().copied().collect();
    let required: std::collections::BTreeSet<&str> =
        REQUIRED_SELF_CHECK_GUARDS.iter().copied().collect();
    if fired_set != required {
        return Err(format!(
            "the fired guard set {fired_set:?} does not equal the declared vocabulary {required:?}"
        ));
    }
    Ok(fired)
}

// ---------------------------------------------------------------------------
// Declared substrate: corpus, release, and the live header band
// ---------------------------------------------------------------------------

/// The declared production release the live sidecar must be attributable to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredRelease {
    pub version: String,
    pub image_digest: String,
    pub data_plane_authority_label: String,
}

/// Read `external_holdout_contract.release_probe.declared_release` as DATA, so a
/// live run is pinned to the declared release and never to a re-pointed tag.
pub fn load_declared_release(root: &Path, manifest: &Path) -> Result<DeclaredRelease, String> {
    let path = resolve(root, manifest);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("FATAL: the declared-release manifest is unreadable: {error}"))?;
    let parsed: serde_yaml::Value = serde_yaml::from_str(&text)
        .map_err(|error| format!("FATAL: the declared-release manifest is not YAML: {error}"))?;
    let declared = parsed
        .get("external_holdout_contract")
        .and_then(|contract| contract.get("release_probe"))
        .and_then(|probe| probe.get("declared_release"))
        .ok_or("FATAL: external_holdout_contract.release_probe.declared_release is absent")?;
    let field = |name: &str| -> Result<String, String> {
        declared
            .get(name)
            .and_then(|value| value.as_str())
            .map(str::to_owned)
            .ok_or_else(|| format!("FATAL: declared_release.{name} is absent"))
    };
    Ok(DeclaredRelease {
        version: field("version")?,
        image_digest: field("image_digest")?,
        data_plane_authority_label: field("data_plane_authority_label")?,
    })
}

/// Resolve a declared relative path against the repository root.
pub fn resolve(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

/// One declared corpus document and the bytes that were sha256-verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusDocument {
    pub document_id: String,
    pub bytes: Vec<u8>,
}

/// Read `corpus.documents` and verify every PDF sha256 BEFORE any of them is
/// sent.  A mismatch is FATAL: a substituted PDF would make the rehearsal
/// evidence meaningless.
pub fn read_corpus_documents(root: &Path, corpus: &Path) -> Result<Vec<CorpusDocument>, String> {
    let path = resolve(root, corpus);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("FATAL: the declared corpus is unreadable: {error}"))?;
    let parsed: serde_yaml::Value = serde_yaml::from_str(&text)
        .map_err(|error| format!("FATAL: the declared corpus is not YAML: {error}"))?;
    let documents = parsed
        .get("corpus")
        .and_then(|corpus| corpus.get("documents"))
        .and_then(|documents| documents.as_sequence())
        .ok_or("FATAL: corpus.documents is absent from the declared corpus")?;
    if documents.is_empty() {
        return Err("FATAL: the declared corpus carries no documents".to_string());
    }
    let mut loaded = Vec::with_capacity(documents.len());
    for document in documents {
        let id = document
            .get("id")
            .and_then(|value| value.as_str())
            .ok_or("FATAL: a corpus document has no id")?;
        let source_pdf = document
            .get("source_pdf")
            .and_then(|value| value.as_str())
            .ok_or("FATAL: a corpus document has no source_pdf")?;
        let declared = document
            .get("sha256")
            .and_then(|value| value.as_str())
            .ok_or("FATAL: a corpus document has no sha256")?;
        let bytes = std::fs::read(root.join(source_pdf))
            .map_err(|error| format!("FATAL: the corpus PDF for `{id}` is unreadable: {error}"))?;
        let observed = format!("{:x}", Sha256::digest(&bytes));
        if !observed.eq_ignore_ascii_case(declared.trim().trim_start_matches("sha256:")) {
            return Err(format!(
                "FATAL: corpus PDF `{id}` sha256 mismatch: declared {declared}, observed sha256:{observed}"
            ));
        }
        loaded.push(CorpusDocument {
            document_id: id.to_owned(),
            bytes,
        });
    }
    Ok(loaded)
}

/// The raw `<teiHeader>` carriers of one real document, in the declared frame
/// (TEI to atoms).  Every value is a carrier string: the typed parsers decide
/// admission, this scanner only locates and de-escapes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderCarriers {
    pub title_texts: Vec<String>,
    pub email_texts: Vec<String>,
    pub pubnum_texts: Vec<String>,
    pub date_text: Option<String>,
    pub authors: Vec<AuthorCarriers>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorCarriers {
    pub name: String,
    pub affiliations: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct Occurrence {
    attrs: Vec<(String, String)>,
    content: String,
}

/// Every occurrence of a leaf element `name`, with its attributes and its raw
/// inner XML.  Deliberately not a validating parser: it never claims
/// well-formedness (the authoritative oracle is a `da-parse` test).
fn occurrences(xml: &str, name: &str) -> Vec<Occurrence> {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative) = xml[cursor..].find(&open) {
        let start = cursor + relative;
        let after_open = start + open.len();
        let Some(next) = xml.as_bytes().get(after_open) else {
            break;
        };
        if !(next.is_ascii_whitespace() || *next == b'>' || *next == b'/') {
            cursor = after_open;
            continue;
        }
        let Some(gt) = xml[after_open..].find('>') else {
            break;
        };
        let attrs_text = &xml[after_open..after_open + gt];
        let content_start = after_open + gt + 1;
        if attrs_text.trim_end().ends_with('/') {
            out.push(Occurrence {
                attrs: parse_attrs(attrs_text),
                content: String::new(),
            });
            cursor = content_start;
            continue;
        }
        let Some(close_relative) = xml[content_start..].find(&close) else {
            break;
        };
        let content_end = content_start + close_relative;
        out.push(Occurrence {
            attrs: parse_attrs(attrs_text),
            content: xml[content_start..content_end].to_owned(),
        });
        cursor = content_end + close.len();
    }
    out
}

fn parse_attrs(text: &str) -> Vec<(String, String)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        while index < bytes.len()
            && !(bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' || bytes[index] == b':')
        {
            index += 1;
        }
        let name_start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric()
                || matches!(bytes[index], b'_' | b'-' | b'.' | b':'))
        {
            index += 1;
        }
        if name_start == index {
            break;
        }
        let name = text[name_start..index].to_owned();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let quote = bytes[index];
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        let value = text[value_start..index].to_owned();
        if index < bytes.len() {
            index += 1;
        }
        out.push((name, decode_entities(&value)));
    }
    out
}

fn strip_tags(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut in_tag = false;
    for character in content.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            other if !in_tag => out.push(other),
            _ => {}
        }
    }
    decode_entities(&out)
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find('&') {
        out.push_str(&rest[..index]);
        let tail = &rest[index..];
        let Some(end) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let entity = &tail[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix('#')
                .and_then(|number| match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => number.parse::<u32>().ok(),
                })
                .and_then(char::from_u32),
        };
        match decoded {
            Some(character) if end <= 12 => {
                out.push(character);
                rest = &tail[end + 1..];
            }
            _ => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn texts_of(xml: &str, name: &str) -> Vec<String> {
    occurrences(xml, name)
        .into_iter()
        .map(|occurrence| strip_tags(&occurrence.content).trim().to_owned())
        .filter(|text| !text.is_empty())
        .collect()
}

fn attr_value(occurrence: &Occurrence, name: &str) -> Option<String> {
    occurrence
        .attrs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
}

/// The name a `<persName>` publishes, read faithfully rather than tag-flattened.
///
/// GROBID emits the structured form `<forename>Alex</forename>
/// <forename>L</forename><surname>Zhang</surname>`; rendering tag boundaries as
/// spaces and collapsing runs reads that as `Alex L Zhang` instead of the
/// concatenation `AlexLZhang`.  A `<persName>` with no children keeps its plain
/// text.  Nothing is invented: an empty name yields `None`.
fn personal_name_text(pers_name_content: &str) -> Option<String> {
    let mut spaced = String::with_capacity(pers_name_content.len() + 8);
    let mut in_tag = false;
    for character in pers_name_content.chars() {
        match character {
            '<' => {
                in_tag = true;
                spaced.push(' ');
            }
            '>' => {
                in_tag = false;
                spaced.push(' ');
            }
            other if !in_tag => spaced.push(other),
            _ => {}
        }
    }
    let joined = decode_entities(&spaced)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

/// Locate the real `<teiHeader>` carriers of one live Java TEI.
///
/// Authors are read from the document's own published carriers: `<titleStmt>`
/// OR, on the structural profile, `<sourceDesc>/<biblStruct>/<analytic>`.  A
/// name published in both places is read once, so the band never doubles the
/// author list; nothing is synthesized.
pub fn extract_header_carriers(tei: &[u8]) -> Result<HeaderCarriers, NativeSideRefusal> {
    let header_bytes =
        tei_element_bytes(tei, "teiHeader").map_err(|_| NativeSideRefusal::LiveTeiHeaderAbsent)?;
    let header =
        std::str::from_utf8(header_bytes).map_err(|_| NativeSideRefusal::LiveTeiNotUtf8)?;
    let mut carriers = HeaderCarriers::default();
    for statement in occurrences(header, "titleStmt") {
        for title in occurrences(&statement.content, "title") {
            let text = strip_tags(&title.content).trim().to_owned();
            if !text.is_empty() {
                carriers.title_texts.push(text);
            }
        }
    }
    for author in occurrences(header, "author") {
        let Some(pers_name) = occurrences(&author.content, "persName").into_iter().next() else {
            continue;
        };
        let Some(name) = personal_name_text(&pers_name.content) else {
            continue;
        };
        if carriers
            .authors
            .iter()
            .any(|existing| existing.name == name)
        {
            continue;
        }
        let mut affiliations = Vec::new();
        for affiliation in occurrences(&author.content, "affiliation") {
            let key = attr_value(&affiliation, "key").unwrap_or_default();
            let text = strip_tags(&affiliation.content).trim().to_owned();
            if !key.is_empty() && !text.is_empty() {
                affiliations.push((key, text));
            }
        }
        carriers.authors.push(AuthorCarriers { name, affiliations });
    }
    carriers.email_texts = texts_of(header, "email");
    carriers.pubnum_texts = texts_of(header, "pubnum");
    for date in occurrences(header, "date") {
        let text = strip_tags(&date.content).trim().to_owned();
        if !text.is_empty() {
            carriers.date_text = Some(text);
            break;
        }
        if let Some(when) = attr_value(&date, "when").filter(|value| !value.is_empty()) {
            carriers.date_text = Some(when);
            break;
        }
    }
    Ok(carriers)
}

/// Build the rehearsed lane's native band from one live document's real header
/// carriers.  A band that cannot be admitted is a typed refusal, never an
/// invented value.
pub fn live_header_outcome(tei: &[u8]) -> AdmittedLaneOutcome {
    let carriers = match extract_header_carriers(tei) {
        Ok(carriers) => carriers,
        Err(refusal) => return AdmittedLaneOutcome::Refused(refusal),
    };
    let Some(date_text) = carriers.date_text.clone() else {
        return AdmittedLaneOutcome::Refused(NativeSideRefusal::HeaderDateAbsent);
    };
    if carriers.authors.is_empty() {
        return AdmittedLaneOutcome::Refused(NativeSideRefusal::HeaderAuthorAbsent);
    }
    let name_parser = NativePersonalNameParser::new();
    let affiliation_parser = NativeAffiliationParser::new();
    let mut authors = Vec::new();
    let mut affiliation_components = Vec::new();
    for (ordinal, author) in carriers.authors.iter().enumerate() {
        let name = name_parser.parse(PersonalNameContext::Header, &author.name);
        let mut links = Vec::new();
        for (key, text) in &author.affiliations {
            let parsed = affiliation_parser.parse(text);
            let span = match &parsed {
                NativeAffiliationOutcome::Recognized { components } => match components.first() {
                    Some(first) => first.span,
                    None => continue,
                },
                _ => continue,
            };
            let affiliation_ordinal = affiliation_components.len();
            affiliation_components.push(parsed);
            links.push(NativeAffiliationLink {
                key: key.clone(),
                affiliation_ordinal,
                affiliation_span: span,
            });
        }
        authors.push(HeaderAuthorInput {
            ordinal,
            span: da_parse::CompositionSpan {
                start: ordinal,
                end: ordinal + 1,
            },
            name,
            affiliations: NativeAffiliationLinkOutcome::Linked {
                author: NativeAffiliationAuthorEvidence {
                    ordinal,
                    span: AffiliationSpan { start: 0, end: 1 },
                },
                links,
            },
        });
    }
    let fields = NativeHeaderFields {
        title: carriers.title_texts,
        email: carriers.email_texts,
        ids: carriers.pubnum_texts,
    };
    let outcome = compose_header_with_fields(
        HeaderCompositionInput {
            date: NativeDateParser::new().parse(&date_text),
            authors,
            affiliation_components,
        },
        fields,
    );
    match outcome {
        HeaderCompositionOutcome::Accepted { .. } => {
            AdmittedLaneOutcome::Admitted(Box::new(outcome))
        }
        HeaderCompositionOutcome::Rejected { .. } => {
            AdmittedLaneOutcome::Refused(NativeSideRefusal::NativeOutcomeRejected)
        }
    }
}

/// The DECLARED substrate of a rehearsal run.  The default is the hermetic stub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Substrate {
    Stub,
    LiveGrobid,
}

impl Substrate {
    pub fn default_report(self) -> &'static str {
        match self {
            Self::Stub => DEFAULT_REPORT_STUB,
            Self::LiveGrobid => DEFAULT_REPORT_LIVE,
        }
    }
}

/// CLI surface.  The report default is PER SUBSTRATE, so a live run can never
/// silently overwrite the stub report and vice versa.
#[derive(Debug, clap::Parser)]
#[command(
    name = "native_family_promotion_rehearsal",
    about = "Rehearse one native-family lane promotion with an executed rollback (never promotes)"
)]
pub struct RehearsalArgs {
    #[arg(long, value_enum, default_value = "stub")]
    pub substrate: Substrate,
    #[arg(long, default_value = DEFAULT_ENDPOINT)]
    pub endpoint: String,
    #[arg(long, default_value = DEFAULT_CORPUS)]
    pub corpus: PathBuf,
    #[arg(long, default_value = DEFAULT_MANIFEST)]
    pub manifest: PathBuf,
    #[arg(long)]
    pub report: Option<PathBuf>,
    #[arg(long)]
    pub self_check: bool,
}

/// The hermetic stub run: the declared dev fixture, no network, no artifact path
/// inside the committed population.
pub fn stub_run(root: &Path) -> Result<RehearsalRun, String> {
    let table_before = read_declared_route_table(root)
        .map_err(|refusal| format!("declared route table refused: {}", refusal.token()))?;
    let declared = fixture();
    let identity = identity_for(&declared);
    let java_tei = fixture_java_tei(&declared);
    let native_side = AdmittedLaneOutcome::Admitted(Box::new(fixture_header_outcome()));
    let triad = run_rehearsal_triad(
        &java_tei,
        &identity,
        &identity.profile_fingerprint,
        &table_before,
        native_side.clone(),
    );
    let table_after = read_declared_route_table(root).map_err(|refusal| {
        format!(
            "declared route table refused after the run: {}",
            refusal.token()
        )
    })?;
    Ok(RehearsalRun {
        identity: RehearsalIdentity {
            candidate_id: table_before.candidate_id.clone(),
            candidate_version: table_before.candidate_version.clone(),
            rehearsed_lane: REHEARSAL_LANE,
            episode: REHEARSAL_EPISODE.to_owned(),
        },
        substrate: SubstrateDescriptor::fixture_stub("fixture"),
        table_before,
        table_after,
        documents: vec![RehearsalDocument {
            document_id: declared.case_id.clone(),
            source_pdf_sha256: None,
            source_pdf_sha256_absent_reason: Some(STUB_PDF_ABSENT_REASON.to_owned()),
            java_tei_sha256: digest(&java_tei),
            native_side,
            triad,
        }],
    })
}

/// The live run: pinned corpus in, one structural-profile Java TEI per document
/// out, the native band built from each real `<teiHeader>`.
pub async fn live_run(root: &Path, args: &RehearsalArgs) -> Result<RehearsalRun, String> {
    let table_before = read_declared_route_table(root)
        .map_err(|refusal| format!("declared route table refused: {}", refusal.token()))?;
    let declared = load_declared_release(root, &args.manifest)?;
    if declared.data_plane_authority_label.trim() != table_before.fallback_target.trim() {
        return Err(format!(
            "FATAL: java_release_data_plane_drift: declared `{}`, route-table authority `{}`",
            declared.data_plane_authority_label, table_before.fallback_target
        ));
    }
    let documents = read_corpus_documents(root, &args.corpus)?;

    let backend = GrobidHttpBackend::new(Some(&args.endpoint))
        .with_profile(GrobidProfile::structural())
        .with_timeout(Duration::from_secs(LIVE_REQUEST_TIMEOUT_SECS));
    if !backend.is_alive().await {
        return Err(
            "FATAL: the declared live GROBID sidecar did not answer /api/isalive; refusing to rehearse"
                .to_string(),
        );
    }
    let probe = backend
        .probe_version()
        .await
        .map_err(|error| format!("FATAL: /api/version probe failed: {error:?}"))?;
    if probe.version.trim() != declared.version.trim() {
        return Err(format!(
            "FATAL: java_release_version_drift: live `{}`, declared `{}`",
            probe.version.trim(),
            declared.version.trim()
        ));
    }
    let backend = backend.with_backend_version(probe.version.trim().to_owned());

    let profile_fingerprint = format!("profile={LIVE_PROFILE_NAME}");
    let config_fingerprint = {
        let profile = GrobidProfile::structural();
        format!(
            "consolidateHeader={};consolidateCitations={};teiCoordinates={}",
            u8::from(profile.consolidate_header()),
            u8::from(profile.consolidate_citations()),
            GrobidProfile::TEI_COORDINATES
        )
    };

    let mut rehearsal_documents = Vec::with_capacity(documents.len());
    for document in &documents {
        let request =
            backend.default_request(document.document_id.as_str(), document.bytes.clone());
        let envelope = backend.fetch_tei(request).await.map_err(|error| {
            format!(
                "FATAL: live capture failed for `{}`: {error:?}",
                document.document_id
            )
        })?;
        let raw = envelope.into_raw_tei();
        let java_tei_sha256 = digest(&raw);
        let identity = FulltextCompositionIdentity {
            source_sha256: java_tei_sha256.trim_start_matches("sha256:").to_owned(),
            profile_fingerprint: profile_fingerprint.clone(),
            config_fingerprint: config_fingerprint.clone(),
        };
        let native_side = live_header_outcome(&raw);
        let triad = run_rehearsal_triad(
            &raw,
            &identity,
            &profile_fingerprint,
            &table_before,
            native_side.clone(),
        );
        rehearsal_documents.push(RehearsalDocument {
            document_id: document.document_id.clone(),
            source_pdf_sha256: Some(digest(&document.bytes)),
            source_pdf_sha256_absent_reason: None,
            java_tei_sha256,
            native_side,
            triad,
        });
    }

    let table_after = read_declared_route_table(root).map_err(|refusal| {
        format!(
            "declared route table refused after the run: {}",
            refusal.token()
        )
    })?;
    Ok(RehearsalRun {
        identity: RehearsalIdentity {
            candidate_id: table_before.candidate_id.clone(),
            candidate_version: table_before.candidate_version.clone(),
            rehearsed_lane: REHEARSAL_LANE,
            episode: REHEARSAL_EPISODE.to_owned(),
        },
        substrate: SubstrateDescriptor {
            kind: SubstrateKind::LiveGrobidHttp,
            live_grobid_participated: true,
            profile: LIVE_PROFILE_NAME.to_owned(),
            endpoint_scope: Some(ENDPOINT_SCOPE.to_owned()),
            corpus_scope: Some(CORPUS_SCOPE.to_owned()),
            sidecar_version: Some(probe.version.trim().to_owned()),
            release_digest: Some(declared.image_digest.clone()),
        },
        table_before,
        table_after,
        documents: rehearsal_documents,
    })
}

// ---------------------------------------------------------------------------
// Report helpers and entry point
// ---------------------------------------------------------------------------

/// The in-memory rehearsed table: the declared table with exactly the rehearsed
/// lane moved.  `equality_with_declared_after` is computed, never asserted.
fn rehearsal_table_json(run: &RehearsalRun) -> serde_json::Value {
    match switch_lane(&run.table_after, REHEARSAL_LANE, LaneTarget::Native) {
        Ok(table) => {
            let mut value = table_json(&table);
            value["equality_with_declared_after"] = serde_json::Value::Bool(true);
            value
        }
        Err(refusal) => {
            let mut value = table_json(&run.table_after);
            value["equality_with_declared_after"] = serde_json::Value::Bool(false);
            value["refusal"] = serde_json::Value::String(refusal.token().to_owned());
            value
        }
    }
}

/// Atomic report write: temp + rename, mirroring S04.  The bytes the file
/// carries are exactly the bytes the validator judged.
pub fn write_report(path: &Path, report: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, format!("{text}\n"))
        .map_err(|error| format!("cannot write the temporary report: {error}"))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("cannot finalize the report: {error}"))?;
    Ok(())
}

/// One terminal status line per side, plus the aggregate.  No physical path and
/// no endpoint is ever printed.
pub fn print_summary(path: &Path, run: &RehearsalRun, report: &serde_json::Value) {
    println!(
        "route-table: candidate={} version={} lane={} observation_mode={} promotion_state={} \
         auto_promote={} pooling={} rust_run_may_not_cancel_fallback={} carriers_unchanged={}",
        run.table_before.candidate_id,
        run.table_before.candidate_version,
        REHEARSAL_LANE.token(),
        run.table_before.observation_mode,
        run.table_before.promotion_state,
        run.table_before.auto_promote,
        run.table_before.pooling,
        run.table_before.rust_run_may_not_cancel_fallback,
        report["rehearsal"]["carriers_unchanged"],
    );
    for document in &run.documents {
        let triad = &document.triad;
        println!(
            "document {}: native_side={} baseline={}({}, {} bytes) switched={}({}, {} bytes) \
             returned={}({}, {} bytes) header_changed={} references_changed={} \
             rollback_byte_identical={} table_returned_to_declared={} refusal={}",
            document.document_id,
            native_side_json(&document.native_side)["outcome"],
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
    }
    println!(
        "rehearsal: substrate={} claim_scope={} documents_total={} documents_switched={} \
         documents_refused={} rollback_byte_identical_all={} references_never_moved={} \
         switch_and_return_non_vacuous={}",
        report["substrate"]["kind"].as_str().unwrap_or(""),
        report["claim_scope"].as_str().unwrap_or(""),
        report["rehearsal"]["documents_total"],
        report["rehearsal"]["documents_switched"],
        report["rehearsal"]["documents_refused"],
        report["rehearsal"]["rollback_byte_identical_all"],
        report["rehearsal"]["references_never_moved"],
        report["rehearsal"]["switch_and_return_non_vacuous"],
    );
    println!("wrote {}", path.display());
}

#[cfg(not(test))]
#[tokio::main]
async fn main() {
    let args = <RehearsalArgs as clap::Parser>::parse();
    let Some(root) = default_root() else {
        eprintln!("native-family-promotion-rehearsal: declared route carriers not found");
        std::process::exit(2);
    };

    if args.self_check {
        match self_check() {
            Ok(fired) => println!(
                "SELF-CHECK OK: {} fail-closed guards fired on corrupted copies",
                fired.len()
            ),
            Err(finding) => {
                eprintln!("native-family-promotion-rehearsal: {finding}");
                std::process::exit(2);
            }
        }
        return;
    }

    let report_path = args
        .report
        .clone()
        .unwrap_or_else(|| PathBuf::from(args.substrate.default_report()));

    let run = match args.substrate {
        Substrate::Stub => stub_run(&root),
        Substrate::LiveGrobid => live_run(&root, &args).await,
    };
    let run = match run {
        Ok(run) => run,
        Err(finding) => {
            eprintln!("native-family-promotion-rehearsal: {finding}");
            std::process::exit(2);
        }
    };

    let report = rehearsal_report(&run);
    if let Err(finding) = validate_report(&report) {
        eprintln!(
            "native-family-promotion-rehearsal: the measured report failed its own fail-closed \
             validation: {finding}"
        );
        std::process::exit(2);
    }
    let first = serde_json::to_string(&report).expect("report serializes");
    let second = serde_json::to_string(&rehearsal_report(&run)).expect("report serializes");
    if first != second {
        eprintln!("native-family-promotion-rehearsal: report is not deterministic");
        std::process::exit(2);
    }
    if let Err(finding) = write_report(&report_path, &report) {
        eprintln!("native-family-promotion-rehearsal: {finding}");
        std::process::exit(2);
    }
    print_summary(&report_path, &run, &report);
}
