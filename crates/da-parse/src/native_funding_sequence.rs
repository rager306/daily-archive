//! Pure sequence runner and entity lift for the native GROBID
//! `funding-acknowledgement` CRF model.
//!
//! `FundingAcknowledgementParser.getExtractionResult` decodes the tokens of a
//! funding/acknowledgement paragraph with the **secondary**
//! `funding-acknowledgement` model (`models/funding-acknowledgement/model.wapiti`,
//! `#mdl#2#15784`, `#rdr#100/16/0`), groups the emitted labels with
//! `TaggingTokenClusteror` and lifts them into `Funding`/`Funder` objects. This
//! module is that second pass and its lift, and nothing else.
//!
//! ## The rows are the T05 funding ABI — 18 fields, window `0..=16`
//!
//! The printed row is [`crate::native_funding_features::FundingFeatureRow`]: the
//! 17 `FeaturesVectorFunding.printVector` feature columns plus a label slot at
//! index 17 that carries the literal `0` at inference time
//! ([`NATIVE_FUNDING_LABEL_COLUMN_DUMMY`]). The frozen model's templates
//! reference columns `0..=16` only, so a model reaching any other column index is
//! refused up front by [`check_funding_column_window`] — the guard is
//! load-bearing, because a 18-column row *does* carry column 17 and
//! `da_ports::map_input` would expand `%x[0,17]` cleanly.
//!
//! The label slot is checked as well ([`NativeFundingSequenceError::WrongLabelSlot`]).
//! Java's `printVector` prints the gold tag there when a label is supplied and the
//! literal `0` at inference; a row carrying anything else is a *training-shape*
//! row, which this inference band must not decode. The T05 renderer cannot build
//! such a row at all, so the refusal closes the loop structurally.
//!
//! ## The closed twenty-label dictionary
//!
//! `data/parse/native_visual_families.yaml` transcribes the frozen model's label
//! block verbatim in model order; [`NATIVE_FUNDING_LABELS`] mirrors that list 1:1
//! (the contract test fails if either side drifts). The block is compared as a
//! *set*, and the two divergences are reported separately (`missing` /
//! `unexpected`) so the diagnostic names the actual defect.
//!
//! `FundingAcknowledgementParser`'s Java label enum declares four further labels
//! that the frozen model block does **not** contain (`<funderAbbrv>`,
//! `<programAbbrv>`, `<projectAbbrv>`, `<url>`): no model branch can emit them and
//! no branch maps them. They are listed in
//! [`NATIVE_FUNDING_UNMODELED_JAVA_LABELS`] and reach the lift as a typed
//! [`NativeFundingSequenceError::UnmodeledJavaLabel`] refusal rather than being
//! silently dropped or silently mapped onto a neighbouring field.
//!
//! ## Grouping is the shared clusteror rule, never a second copy (D359)
//!
//! Funding labels are grouped by [`cluster_family_label_runs`] — GROBID's
//! `TaggingTokenClusteror`, where a token carrying the `I-` prefix *opens* a
//! cluster. `I-<grantNumber>` followed by `<grantNumber>` is therefore **two**
//! clusters while `<grantNumber>` followed by `<grantNumber>` is one. The D359
//! `base_label_runs` of the header/bibliography/fulltext bands answers the
//! opposite question and must not be used here; the divergence and its witness
//! live in [`crate::native_visual_family_cluster`]. Only the `I-` *stripping* is
//! shared ([`crate::native_bibliography_sequence::base_label`]), so every role
//! comparison is made against the base name.
//!
//! ## The entity lift is the Java branch table, role by role
//!
//! One cluster carries one role; the role decides the destination field:
//!
//! | base label | destination |
//! |---|---|
//! | `<funderName>` | `funder_name` |
//! | `<grantName>` | `grant_name` |
//! | `<grantNumber>` | `grant_number` |
//! | `<programName>` | `program_name` |
//! | `<projectName>` | `project_name` |
//! | `<person>` | `persons` (append) |
//! | `<affiliation>` | `affiliations` (append) |
//! | `<institution>` | `institutions` (append) |
//! | `<infrastructure>` | `institutions` **and** `infrastructures` (append) |
//! | `<other>` | counted in [`NativeFundingDecodeOutcome::Decoded::other_count`], never a field |
//!
//! The four list roles are pure accumulation, which is exactly what Java's
//! "push the previous accumulator, then overwrite" branch computes for non-blank
//! clusters: consecutive `<person>` clusters become two entries in the list, in
//! reading order. The `<infrastructure>` row is Java's
//! `Affiliation.setInfrastructure(true)` marker projected as a second list: Java
//! keeps one affiliation list plus a boolean, this band keeps the marked subset
//! addressable without carrying the flag type.
//!
//! ## The split rule is Java's `Funding.isValid()`, and the emission rule is not
//!
//! Each of the five scalar branches in Java reads
//! `if (isNotBlank(previous value) && funding.isValid()) { fundings.add(funding);
//! funding = new Funding(); }` **before** overwriting the field. So a repeat of a
//! scalar role closes the open record and opens the next one *only when the
//! record is already valid*; otherwise the repeat overwrites in place. That
//! `&& funding.isValid()` conjunct is in fact implied by the non-blank guard it
//! accompanies — every guarded field is itself a term of `isValid()`, so a filled
//! slot already satisfies it — and is reproduced because it is in the Java source,
//! not because it can change a verdict.
//! [`NativeFundingEntry::is_valid`] is the transcription of `Funding.isValid()`
//! and is used for that decision only.
//!
//! Emission is a different predicate. Java keeps persons, affiliations and
//! institutions in lists *outside* `Funding`, so a paragraph that carries only a
//! `<person>` yields `fundings: []` plus a non-empty person list. This band's
//! flattened [`NativeFundingEntry`] has no such side list, so the emission
//! predicate is [`NativeFundingEntry::carries_any_observation`] (any scalar
//! field, or any list): an observation is never dropped merely because it is not
//! a valid *funding record*. The two predicates are deliberately not the same,
//! and the contract test pins both.
//!
//! ## Absence is not zero (H12-N1)
//!
//! An empty decode is [`NativeFundingAbsence::NoFundingLabels`], and a decode
//! whose clusters all carried empty token text is
//! [`NativeFundingAbsence::NoLiftableContent`] — never `Decoded { entries: [] }`.
//! The two are exhaustive: a `Decoded` outcome always carries either an entry or
//! at least one counted `<other>`, so an empty entity list can never be read as
//! "zero funders were observed".
//!
//! ## Named transcription residuals
//!
//! These are Java behaviours this band does **not** reproduce. They are named
//! rather than silent, in the same style as the sibling runners' `offline_limit`s:
//!
//! 1. **`Funding.inferAcronyms()` is not implemented.** Java rewrites the funder
//!    name from an acronym candidate inside the full name and from the
//!    `Funder.prefixFounders` grant-number prefix table. That is an inference over
//!    a private lexicon (and the plan names it a non-goal), so it is an explicit
//!    `offline_limit` of this wave: the emitted `funder_name` is the text the
//!    model emitted, never a rewritten one.
//! 2. **The grant-number "factorization" copy is not applied.** Java copies the
//!    previous record's funder onto the new record when a repeated
//!    `<grantNumber>` has the same string length as the previous one. That is an
//!    inference across records; this band conserves observed spans and never
//!    invents a funder for a record the model did not label one for.
//! 3. **`Funding.setGrantNumber`'s `n˚` prefix strip is not applied.** Java
//!    removes a leading `n˚` from the observed number; this band emits the model's
//!    own text, so a consumer can always reproduce the cluster it came from.
//! 4. **Blank clusters are not observations.** Java assigns the empty string and
//!    thereby leaves a non-null field behind (`funder != null` makes `isValid()`
//!    true). Here an empty cluster contributes nothing and leaves the slot `None`,
//!    because "the model emitted an empty cluster" and "the cluster carried text"
//!    must stay distinguishable.
//! 5. **Token-level normalisation is the producer's.** Java pipes every token
//!    through `UnicodeUtil.normaliseText` and joins cluster tokens with
//!    `LayoutTokensUtil.normalizeText`; this band takes the already normalised
//!    column-0 text of the rendered rows, exactly as the figure/table runners do.
//! 6. **Layout positions are not carried.** Java also returns
//!    `OffsetPosition`s and `LayoutToken` lists per annotation; that is layout
//!    provenance, not this band's ABI.
//!
//! ## Redaction boundary
//!
//! [`NativeFundingEntry`] fields carry raw funder/grant/person/affiliation strings
//! (PII), because they *are* the lifted observation. Nothing here logs,
//! persists or reports them; [`NativeFundingEntry::digest`] is the only shape in
//! which an entry may leave the band into a fixture, report, or governor stamp.
//!
//! ## Quarantine
//!
//! Pure and additive: no filesystem, no environment, no clock, no logging, no
//! model bytes, no lexicon bytes, no absolute path. Every function is total on
//! hostile input — an empty sequence, a wrong-arity row, a training-shape label
//! slot, a foreign label block, a declared column window outside `0..=16`, a
//! malformed template, an over-long sequence, and an unmodelled Java label are
//! typed refusals, never a panic and never a silent truncation. `import_eligible`
//! stays false and nothing here is spliced into the production parse route.

use crate::native_funding_features::{
    FundingFeatureRow, NATIVE_FUNDING_LABEL_COLUMN_DUMMY, NATIVE_FUNDING_LABEL_COLUMN_INDEX,
    NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX, NATIVE_FUNDING_ROW_COLUMNS, render_funding_feature_text,
};
use crate::native_visual_family_cluster::{
    FamilyLabelCluster, NativeVisualFamilyClusterError, cluster_family_label_runs,
};
use da_ports::{CrfModel, CrfModelError, map_input, viterbi_decode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Schema identity of the decoded funding sequence surface.
pub const NATIVE_FUNDING_SEQUENCE_SCHEMA_VERSION: &str = "da-parse.native-funding-sequence/v1";

/// Rows admitted into one funding decode, mirroring the sibling runners' bound.
///
/// Provisional and fail-closed: an input beyond this bound is refused rather than
/// truncated, so no silently partial read can be mistaken for a complete one.
pub const MAX_NATIVE_FUNDING_ROWS: usize = 32_768;

/// The closed decoded-label vocabulary of
/// `models/funding-acknowledgement/model.wapiti`.
///
/// Transcribed in the frozen model's own order, mirroring
/// `families.funding.declared_labels` in
/// `data/parse/native_visual_families.yaml` 1:1 (ten base roles, each in both the
/// bare and the `I-` spelling). The order is a transcription fact and carries no
/// meaning to the decoder: the label-block check compares sets, and the loaded
/// model owns the order its weight blocks index.
pub const NATIVE_FUNDING_LABELS: [&str; 20] = [
    "I-<other>",
    "<other>",
    "I-<funderName>",
    "<funderName>",
    "I-<grantNumber>",
    "<grantNumber>",
    "I-<person>",
    "<person>",
    "I-<programName>",
    "<programName>",
    "I-<grantName>",
    "<grantName>",
    "I-<projectName>",
    "<projectName>",
    "I-<affiliation>",
    "<affiliation>",
    "I-<institution>",
    "<institution>",
    "I-<infrastructure>",
    "<infrastructure>",
];

/// The ten base roles, mirroring `families.funding.base_labels` in the DATA file.
pub const NATIVE_FUNDING_BASE_LABELS: [&str; 10] = [
    "<other>",
    "<funderName>",
    "<grantNumber>",
    "<person>",
    "<programName>",
    "<grantName>",
    "<projectName>",
    "<affiliation>",
    "<institution>",
    "<infrastructure>",
];

/// Labels `FundingAcknowledgementParser`'s Java enum declares but the frozen
/// model block does not contain.
///
/// `mirrors` `families.funding.java_declared_not_in_model`. No decode path can
/// produce them, and the lift refuses them by name instead of dropping them or
/// mapping them onto a neighbouring field.
pub const NATIVE_FUNDING_UNMODELED_JAVA_LABELS: [&str; 4] =
    ["<funderAbbrv>", "<programAbbrv>", "<projectAbbrv>", "<url>"];

/// The typed role one funding label carries.
///
/// The role is derived from the *base* label, so both spellings of one role map
/// here; the `I-` prefix only ever decides the cluster boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFundingRole {
    FunderName,
    GrantName,
    GrantNumber,
    ProgramName,
    ProjectName,
    Person,
    Affiliation,
    Institution,
    Infrastructure,
    Other,
}

impl NativeFundingRole {
    /// Every role, in the order [`NATIVE_FUNDING_BASE_LABELS`] spells.
    pub const ALL: [NativeFundingRole; 10] = [
        Self::Other,
        Self::FunderName,
        Self::GrantNumber,
        Self::Person,
        Self::ProgramName,
        Self::GrantName,
        Self::ProjectName,
        Self::Affiliation,
        Self::Institution,
        Self::Infrastructure,
    ];

    /// Wire spelling used in reports (`funder_name`, `grant_number`, …).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FunderName => "funder_name",
            Self::GrantName => "grant_name",
            Self::GrantNumber => "grant_number",
            Self::ProgramName => "program_name",
            Self::ProjectName => "project_name",
            Self::Person => "person",
            Self::Affiliation => "affiliation",
            Self::Institution => "institution",
            Self::Infrastructure => "infrastructure",
            Self::Other => "other",
        }
    }

    /// The base label (angle brackets included) this role groups.
    pub const fn base_label(self) -> &'static str {
        match self {
            Self::FunderName => "<funderName>",
            Self::GrantName => "<grantName>",
            Self::GrantNumber => "<grantNumber>",
            Self::ProgramName => "<programName>",
            Self::ProjectName => "<projectName>",
            Self::Person => "<person>",
            Self::Affiliation => "<affiliation>",
            Self::Institution => "<institution>",
            Self::Infrastructure => "<infrastructure>",
            Self::Other => "<other>",
        }
    }

    /// Classify a base label. `None` outside the closed ten, so the caller turns
    /// that into a typed refusal rather than a default.
    pub fn from_base_label(base: &str) -> Option<Self> {
        NATIVE_FUNDING_BASE_LABELS
            .iter()
            .position(|label| *label == base)
            .and_then(|index| Self::ALL.get(index).copied())
    }

    /// True for the five scalar roles that participate in the record split.
    pub const fn is_scalar(self) -> bool {
        matches!(
            self,
            Self::FunderName
                | Self::GrantName
                | Self::GrantNumber
                | Self::ProgramName
                | Self::ProjectName
        )
    }
}

/// Is `label` (either spelling) part of the closed native vocabulary?
pub fn funding_label_is_admissible(label: &str) -> bool {
    NATIVE_FUNDING_LABELS.contains(&label)
}

/// The role of a decoded label, in either spelling; `None` outside the closed
/// ten (including the four declared-but-unmodelled Java labels).
pub fn funding_role_of_label(label: &str) -> Option<NativeFundingRole> {
    NativeFundingRole::from_base_label(crate::native_bibliography_sequence::base_label(label))
}

/// One funding record lifted from a decoded sequence.
///
/// The flattened counterpart of Java's `Funding` plus the person/affiliation
/// accumulators that Java keeps beside it (see the module docs). A role the model
/// did not emit stays `None`/empty — never `Some("")` — so "the model emitted no
/// such cluster" and "the cluster was empty" stay distinguishable.
///
/// Every `String` here is raw funder/grant/person material (PII): see
/// [`Self::digest`] for the only shape that may leave the band.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFundingEntry {
    /// `<funderName>`.
    pub funder_name: Option<String>,
    /// `<grantName>`.
    pub grant_name: Option<String>,
    /// `<grantNumber>`.
    pub grant_number: Option<String>,
    /// `<programName>`.
    pub program_name: Option<String>,
    /// `<projectName>`.
    pub project_name: Option<String>,
    /// `<person>` clusters, in reading order.
    pub persons: Vec<String>,
    /// `<affiliation>` clusters, in reading order.
    pub affiliations: Vec<String>,
    /// `<institution>` and `<infrastructure>` clusters, in reading order.
    pub institutions: Vec<String>,
    /// The `<infrastructure>` subset of [`Self::institutions`], in reading order.
    pub infrastructures: Vec<String>,
}

impl NativeFundingEntry {
    /// Java's `Funding.isValid()`: does this carry a *funding record* field?
    ///
    /// This is the **split predicate** and nothing else. Java's version is
    /// `funder != null || grantNumber != null || grantName != null ||
    /// projectFullName != null || programFullName != null || url != null`, where
    /// `projectAbbreviatedName`, `programAbbreviatedName` and `url` are never
    /// assigned by this model (no branch sets them), so the three abbreviate/url
    /// terms drop out and the three never-assigned ones are unreachable.
    ///
    /// The person/affiliation/institution lists are deliberately **not** part of
    /// this predicate: in Java they live outside `Funding` entirely. Use
    /// [`Self::carries_any_observation`] for the emission decision.
    pub fn is_valid(&self) -> bool {
        self.funder_name.is_some()
            || self.grant_name.is_some()
            || self.grant_number.is_some()
            || self.program_name.is_some()
            || self.project_name.is_some()
    }

    /// Does this entry carry any lifted observation at all?
    ///
    /// The **emission predicate** of this band. It is the conservative superset of
    /// [`Self::is_valid`]: a paragraph whose only label is `<person>` is not a
    /// valid funding record in Java's sense (Java keeps the person in a separate
    /// list), but it *was* observed and this flattened shape has no separate list
    /// to conserve it in — so it is emitted here instead of vanishing.
    pub fn carries_any_observation(&self) -> bool {
        self.is_valid()
            || !self.persons.is_empty()
            || !self.affiliations.is_empty()
            || !self.institutions.is_empty()
            || !self.infrastructures.is_empty()
    }

    /// Redaction-safe identity of this entry: `sha256:` over a canonical
    /// serialization of every field.
    ///
    /// The canonical form tags presence explicitly and separates units with
    /// control characters (`0x1f` per field slot, `0x1e` per list member), so no
    /// raw value can collide with a separator and `None` cannot be confused with
    /// `Some("")`. This is the only shape in which an entry may cross into a
    /// fixture, report or governor stamp.
    pub fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        for slot in [
            self.funder_name.as_deref(),
            self.grant_name.as_deref(),
            self.grant_number.as_deref(),
            self.program_name.as_deref(),
            self.project_name.as_deref(),
        ] {
            match slot {
                Some(value) => {
                    hasher.update(*b"v");
                    hasher.update(value.as_bytes());
                }
                None => hasher.update(*b"-"),
            }
            hasher.update([0x1f]);
        }
        for list in [
            &self.persons,
            &self.affiliations,
            &self.institutions,
            &self.infrastructures,
        ] {
            for value in list {
                hasher.update(*b"v");
                hasher.update(value.as_bytes());
                hasher.update([0x1e]);
            }
            hasher.update([0x1f]);
        }
        format!("sha256:{:x}", hasher.finalize())
    }
}

/// Closed vocabulary of funding absences.
///
/// Both variants are H12-N1 witnesses: an observation that produced no entity is
/// an absence with a name, never a successful empty list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFundingAbsence {
    /// The decoded label sequence carried no funding-family label at all.
    NoFundingLabels,
    /// The decode carried funding-family clusters, but every one of them was
    /// empty and no `<other>` cluster carried text, so nothing could be lifted.
    NoLiftableContent,
}

/// Typed result of one funding decode: the lifted entries, or the absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativeFundingDecodeOutcome {
    Decoded {
        /// One decoded label per row, in reading order, spelling preserved.
        labels: Vec<String>,
        /// The lifted records, in reading order.
        entries: Vec<NativeFundingEntry>,
        /// How many `<other>` clusters carried text.
        ///
        /// `<other>` is not a field, so it would otherwise vanish: the counter is
        /// what keeps "the model observed material it could not place" visible.
        other_count: usize,
    },
    NotObserved {
        reason: NativeFundingAbsence,
    },
}

/// Typed refusal vocabulary for the funding sequence runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFundingSequenceError {
    /// No rows were supplied, so there is nothing to decode.
    EmptySequence,
    /// The sequence exceeds the bounded runner's admission.
    TooManyRows { actual: usize, maximum: usize },
    /// A rendered row does not present the printed 18-column width.
    WrongArity {
        row: usize,
        actual: usize,
        expected: usize,
    },
    /// A row's label slot does not carry the inference literal.
    ///
    /// Java prints the gold tag there when a label is supplied, so such a row is a
    /// *training-shape* row and this inference band refuses to decode it.
    WrongLabelSlot { row: usize, actual: String },
    /// The model declares a label block of the wrong cardinality.
    ModelLabelCount { expected: usize, actual: usize },
    /// The model declares the right number of labels but not the native set.
    ModelLabelMismatch {
        missing: Vec<String>,
        unexpected: Vec<String>,
    },
    /// The model's templates do not reference exactly columns `0..=16`.
    ///
    /// `declared` is the highest column index seen in a `%x[offset,column]`
    /// template, or `None` when no template references a column at all.
    DeclaredColumnMismatch {
        declared: Option<usize>,
        expected: usize,
    },
    /// A label outside the closed native vocabulary reached the lift.
    UnmodeledLabel { label: String, row: usize },
    /// A label the Java label enum declares but the frozen model block does not
    /// contain reached the lift.
    ///
    /// It has no branch in `FundingAcknowledgementParser` either, so mapping it
    /// onto a neighbouring field would invent an observation.
    UnmodeledJavaLabel { label: String, row: usize },
    /// The label/row parallelism the lift requires is broken.
    CountMismatch { expected: usize, actual: usize },
    /// The shared `TaggingTokenClusteror` grouping pass refused the labels.
    Grouping {
        error: NativeVisualFamilyClusterError,
    },
    /// The 18-column row serializer refused the rows handed to the runner.
    Feature { message: String },
    /// The scalar CRF itself refused the input.
    Model { message: String },
}

impl fmt::Display for NativeFundingSequenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySequence => write!(formatter, "native funding sequence is empty"),
            Self::TooManyRows { actual, maximum } => write!(
                formatter,
                "native funding sequence has {actual} rows; maximum is {maximum}"
            ),
            Self::WrongArity {
                row,
                actual,
                expected,
            } => write!(
                formatter,
                "native funding row {row} has {actual} columns; expected {expected}"
            ),
            Self::WrongLabelSlot { row, actual } => write!(
                formatter,
                "native funding row {row} carries {actual:?} in its label slot; the inference \
                 literal is {NATIVE_FUNDING_LABEL_COLUMN_DUMMY:?}"
            ),
            Self::ModelLabelCount { expected, actual } => write!(
                formatter,
                "funding model declares {actual} labels; the closed dictionary has {expected}"
            ),
            Self::ModelLabelMismatch {
                missing,
                unexpected,
            } => write!(
                formatter,
                "funding model label block diverges from the native dictionary \
                 (missing {missing:?}, unexpected {unexpected:?})"
            ),
            Self::DeclaredColumnMismatch { declared, expected } => write!(
                formatter,
                "funding model references column {declared:?}; the frozen window ends at \
                 {expected}"
            ),
            Self::UnmodeledLabel { label, row } => write!(
                formatter,
                "funding label {label:?} at row {row} is not part of the closed dictionary"
            ),
            Self::UnmodeledJavaLabel { label, row } => write!(
                formatter,
                "funding label {label:?} at row {row} is declared by the Java label enum but \
                 absent from the frozen model block, so it has no lift branch"
            ),
            Self::CountMismatch { expected, actual } => write!(
                formatter,
                "funding decode produced {actual} values for {expected} positions"
            ),
            Self::Grouping { error } => write!(formatter, "funding label grouping failed: {error}"),
            Self::Feature { message } => {
                write!(formatter, "funding row rendering failed: {message}")
            }
            Self::Model { message } => write!(formatter, "funding CRF failed: {message}"),
        }
    }
}

impl std::error::Error for NativeFundingSequenceError {}

/// Fail closed when a model's declared label block is not the native set.
///
/// Two distinct conditions are reported separately so the diagnostic names the
/// actual defect: a wrong cardinality, and a same-sized block that still diverges
/// from the dictionary. The comparison is order-independent, because the loaded
/// model owns the label ids its weight blocks index.
pub fn check_funding_label_block(model: &CrfModel) -> Result<(), NativeFundingSequenceError> {
    if model.labels.len() != NATIVE_FUNDING_LABELS.len() {
        return Err(NativeFundingSequenceError::ModelLabelCount {
            expected: NATIVE_FUNDING_LABELS.len(),
            actual: model.labels.len(),
        });
    }
    let mut missing: Vec<String> = NATIVE_FUNDING_LABELS
        .iter()
        .filter(|label| !model.labels.iter().any(|declared| declared == *label))
        .map(|label| (*label).to_string())
        .collect();
    let mut unexpected: Vec<String> = model
        .labels
        .iter()
        .filter(|declared| !NATIVE_FUNDING_LABELS.contains(&declared.as_str()))
        .cloned()
        .collect();
    if missing.is_empty() && unexpected.is_empty() {
        return Ok(());
    }
    missing.sort_unstable();
    missing.dedup();
    unexpected.sort_unstable();
    unexpected.dedup();
    Err(NativeFundingSequenceError::ModelLabelMismatch {
        missing,
        unexpected,
    })
}

/// Fail closed when the model's referenced column window is not the frozen one.
///
/// The frozen `funding-acknowledgement` model declares `#rdr#100/16/0`: its
/// templates reference columns `0..=16` of the printed row and never the label
/// slot at index 17. Because an 18-column row *does* carry column 17,
/// `da_ports::map_input` would expand `%x[0,17]` successfully — so this check is
/// the only thing standing between a drifted model and the silent consumption of
/// the label slot as a feature. The window is read off the model's own templates,
/// which is the only place a loaded [`CrfModel`] declares a column at all.
pub fn check_funding_column_window(model: &CrfModel) -> Result<(), NativeFundingSequenceError> {
    let declared = template_max_column_index(model).map_err(model_error)?;
    if declared == Some(NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX) {
        return Ok(());
    }
    Err(NativeFundingSequenceError::DeclaredColumnMismatch {
        declared,
        expected: NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX,
    })
}

/// Decode rendered funding rows through the supplied model and lift the entities.
///
/// No model loading, filesystem access, environment lookup, or fallback is
/// performed here. The rows are re-serialized through
/// [`render_funding_feature_text`] so the text the model sees is exactly the ABI
/// the T05 builder produced, and that text is what `da_ports::map_input` takes.
/// Equal inputs and model values always produce equal output.
pub fn decode_funding_sequence(
    rows: &[FundingFeatureRow],
    model: &CrfModel,
) -> Result<NativeFundingDecodeOutcome, NativeFundingSequenceError> {
    if rows.is_empty() {
        return Err(NativeFundingSequenceError::EmptySequence);
    }
    if rows.len() > MAX_NATIVE_FUNDING_ROWS {
        return Err(NativeFundingSequenceError::TooManyRows {
            actual: rows.len(),
            maximum: MAX_NATIVE_FUNDING_ROWS,
        });
    }
    check_funding_label_block(model)?;
    check_funding_column_window(model)?;
    for (row, feature_row) in rows.iter().enumerate() {
        if feature_row.width() != NATIVE_FUNDING_ROW_COLUMNS {
            return Err(NativeFundingSequenceError::WrongArity {
                row,
                actual: feature_row.width(),
                expected: NATIVE_FUNDING_ROW_COLUMNS,
            });
        }
        let slot = feature_row
            .columns
            .get(NATIVE_FUNDING_LABEL_COLUMN_INDEX)
            .map(String::as_str)
            .unwrap_or_default();
        if slot != NATIVE_FUNDING_LABEL_COLUMN_DUMMY {
            return Err(NativeFundingSequenceError::WrongLabelSlot {
                row,
                actual: slot.to_string(),
            });
        }
    }

    let input =
        render_funding_feature_text(rows).map_err(|error| NativeFundingSequenceError::Feature {
            message: error.to_string(),
        })?;
    let positions = map_input(model, &input).map_err(model_error)?;
    if positions.len() != rows.len() {
        return Err(NativeFundingSequenceError::CountMismatch {
            expected: rows.len(),
            actual: positions.len(),
        });
    }
    let decoded = viterbi_decode(model, &positions).map_err(model_error)?;
    if decoded.labels.len() != rows.len() {
        return Err(NativeFundingSequenceError::CountMismatch {
            expected: rows.len(),
            actual: decoded.labels.len(),
        });
    }
    funding_entries_from_decoded(&decoded.labels, rows)
}

/// Lift an already-decoded label sequence into funding entries.
///
/// This is the seam the decoder above calls, kept public because it owns the
/// H12-N1 rule and the Java split rule, and because a caller that assembled
/// labels elsewhere still needs the same lift. `rows` supplies the token text of
/// each decoded position (column 0 of the printed row).
///
/// The lift is the `FundingAcknowledgementParser.getExtractionResult` branch
/// table, role by role, with the two predicates the module docs describe: the
/// split uses [`NativeFundingEntry::is_valid`], emission uses
/// [`NativeFundingEntry::carries_any_observation`]. A label outside the closed
/// ten is a typed refusal — [`NativeFundingSequenceError::UnmodeledJavaLabel`]
/// for the four declared-but-unmodelled Java labels, `UnmodeledLabel` otherwise.
pub fn funding_entries_from_decoded(
    labels: &[String],
    rows: &[FundingFeatureRow],
) -> Result<NativeFundingDecodeOutcome, NativeFundingSequenceError> {
    if labels.is_empty() {
        return Ok(NativeFundingDecodeOutcome::NotObserved {
            reason: NativeFundingAbsence::NoFundingLabels,
        });
    }
    if labels.len() != rows.len() {
        return Err(NativeFundingSequenceError::CountMismatch {
            expected: labels.len(),
            actual: rows.len(),
        });
    }
    for (row, label) in labels.iter().enumerate() {
        if !funding_label_is_admissible(label) {
            return Err(unmodeled_label(label, row));
        }
    }
    for (row, feature_row) in rows.iter().enumerate() {
        if feature_row.width() != NATIVE_FUNDING_ROW_COLUMNS {
            return Err(NativeFundingSequenceError::WrongArity {
                row,
                actual: feature_row.width(),
                expected: NATIVE_FUNDING_ROW_COLUMNS,
            });
        }
    }
    let clusters = cluster_family_label_runs(labels)
        .map_err(|error| NativeFundingSequenceError::Grouping { error })?;

    let mut entries: Vec<NativeFundingEntry> = Vec::new();
    let mut entry = NativeFundingEntry::default();
    let mut other_count = 0usize;

    for cluster in &clusters {
        let text = cluster_text(rows, cluster)?;
        let Some(role) = NativeFundingRole::from_base_label(&cluster.base) else {
            // The admissibility guard above already proves this branch cannot be
            // taken for a dictionary label; it stays a typed refusal instead of a
            // panic so a hand-assembled sequence can never abort a caller.
            return Err(unmodeled_label(&cluster.base, cluster.start));
        };
        match role {
            NativeFundingRole::Other => {
                // Not a field. Counted rather than dropped, and only when it
                // carried text: an empty cluster observed nothing.
                if !text.is_empty() {
                    other_count += 1;
                }
            }
            NativeFundingRole::Person => {
                if !text.is_empty() {
                    entry.persons.push(text);
                }
            }
            NativeFundingRole::Affiliation => {
                if !text.is_empty() {
                    entry.affiliations.push(text);
                }
            }
            NativeFundingRole::Institution => {
                if !text.is_empty() {
                    entry.institutions.push(text);
                }
            }
            NativeFundingRole::Infrastructure => {
                if !text.is_empty() {
                    // Java marks the same affiliation `infrastructure = true`
                    // (`Affiliation.setInfrastructure`); the marked subset is kept
                    // addressable as its own list here.
                    entry.institutions.push(text.clone());
                    entry.infrastructures.push(text);
                }
            }
            scalar => {
                if text.is_empty() {
                    // A blank cluster is not an observation (see the module docs).
                    continue;
                }
                if scalar_slot_is_filled(&entry, scalar) && entry.is_valid() {
                    // Java: `if (isNotBlank(previous) && funding.isValid())` →
                    // close the record and open the next one. Both conditions are
                    // Java's; without the validity test a repeat overwrites.
                    entries.push(std::mem::take(&mut entry));
                }
                set_scalar_slot(&mut entry, scalar, text);
            }
        }
    }

    if entry.carries_any_observation() {
        entries.push(entry);
    }
    if entries.is_empty() && other_count == 0 {
        return Ok(NativeFundingDecodeOutcome::NotObserved {
            reason: NativeFundingAbsence::NoLiftableContent,
        });
    }
    Ok(NativeFundingDecodeOutcome::Decoded {
        labels: labels.to_vec(),
        entries,
        other_count,
    })
}

/// Type the refusal for a label outside the closed ten.
///
/// `label` is reported exactly as it was observed (spelling preserved, `I-`
/// included), because the diagnostic must name what the model emitted. The
/// *classification* looks through the spelling prefix, so `I-<funderAbbrv>` is
/// recognised as a declared-but-unmodelled Java label just like the bare form.
fn unmodeled_label(label: &str, row: usize) -> NativeFundingSequenceError {
    let base = crate::native_bibliography_sequence::base_label(label);
    if NATIVE_FUNDING_UNMODELED_JAVA_LABELS.contains(&base) {
        NativeFundingSequenceError::UnmodeledJavaLabel {
            label: label.to_string(),
            row,
        }
    } else {
        NativeFundingSequenceError::UnmodeledLabel {
            label: label.to_string(),
            row,
        }
    }
}

/// Is the scalar slot of `role` already carrying text?
fn scalar_slot_is_filled(entry: &NativeFundingEntry, role: NativeFundingRole) -> bool {
    match role {
        NativeFundingRole::FunderName => entry.funder_name.is_some(),
        NativeFundingRole::GrantName => entry.grant_name.is_some(),
        NativeFundingRole::GrantNumber => entry.grant_number.is_some(),
        NativeFundingRole::ProgramName => entry.program_name.is_some(),
        NativeFundingRole::ProjectName => entry.project_name.is_some(),
        // Only the five scalar roles reach here; naming them keeps the match total
        // without a catch-all that could hide a new role.
        NativeFundingRole::Person
        | NativeFundingRole::Affiliation
        | NativeFundingRole::Institution
        | NativeFundingRole::Infrastructure
        | NativeFundingRole::Other => false,
    }
}

/// Write one scalar role's text into its slot (Java's `set*`, an overwrite).
fn set_scalar_slot(entry: &mut NativeFundingEntry, role: NativeFundingRole, text: String) {
    match role {
        NativeFundingRole::FunderName => entry.funder_name = Some(text),
        NativeFundingRole::GrantName => entry.grant_name = Some(text),
        NativeFundingRole::GrantNumber => entry.grant_number = Some(text),
        NativeFundingRole::ProgramName => entry.program_name = Some(text),
        NativeFundingRole::ProjectName => entry.project_name = Some(text),
        NativeFundingRole::Person
        | NativeFundingRole::Affiliation
        | NativeFundingRole::Institution
        | NativeFundingRole::Infrastructure
        | NativeFundingRole::Other => {}
    }
}

/// The printed token text of one cluster: column 0 of every row in its range,
/// joined by a single space (GROBID's own `TextUtilities.join` separator).
///
/// Empty tokens are skipped, so an all-empty cluster yields the empty string.
fn cluster_text(
    rows: &[FundingFeatureRow],
    cluster: &FamilyLabelCluster,
) -> Result<String, NativeFundingSequenceError> {
    let mut text = String::new();
    for row in cluster.start..cluster.end {
        let Some(feature_row) = rows.get(row) else {
            return Err(NativeFundingSequenceError::CountMismatch {
                expected: cluster.end,
                actual: rows.len(),
            });
        };
        let token = feature_row
            .columns
            .first()
            .map(String::as_str)
            .unwrap_or_default();
        if token.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(token);
    }
    Ok(text)
}

/// The highest `%x[offset,column]` column index any template references.
///
/// `None` means no template references a column at all. A template whose
/// `%x[...]` spec is malformed is a [`CrfModelError::MalformedTemplate`], the same
/// refusal `da_ports::map_input` would raise — a malformed template must not be
/// skipped, or it could hide a column reference the window check exists to catch.
///
/// This is the same private reader the figure and table runners carry; the three
/// families each read the window off their own model, and consolidating the three
/// copies is a follow-up rather than a behaviour.
fn template_max_column_index(model: &CrfModel) -> Result<Option<usize>, CrfModelError> {
    let mut maximum: Option<usize> = None;
    for pattern in &model.patterns {
        let source = pattern.source.as_str();
        let mut cursor = 0usize;
        while let Some(relative) = source[cursor..].find("%x[") {
            let start = cursor + relative;
            let end = source[start + 3..]
                .find(']')
                .map(|offset| start + 3 + offset)
                .ok_or(CrfModelError::MalformedTemplate)?;
            let spec = &source[start + 3..end];
            let column = spec
                .split_once(',')
                .filter(|(_, column)| !column.contains(','))
                .map(|(_, column)| column)
                .ok_or(CrfModelError::MalformedTemplate)?;
            let column = column
                .parse::<usize>()
                .map_err(|_| CrfModelError::MalformedTemplate)?;
            maximum = Some(maximum.map_or(column, |current| current.max(column)));
            cursor = end + 1;
        }
    }
    Ok(maximum)
}

fn model_error(error: CrfModelError) -> NativeFundingSequenceError {
    NativeFundingSequenceError::Model {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::scalar_crf::CrfPattern;
    use std::collections::BTreeMap;

    /// One 18-column row whose first column carries the token text and whose label
    /// slot carries the inference literal; the middle columns are inert.
    fn row(value: &str) -> FundingFeatureRow {
        let columns: Vec<String> = std::iter::once(value.to_string())
            .chain((1..NATIVE_FUNDING_LABEL_COLUMN_INDEX).map(|_| "0".to_string()))
            .chain(std::iter::once(
                NATIVE_FUNDING_LABEL_COLUMN_DUMMY.to_string(),
            ))
            .collect();
        assert_eq!(columns.len(), NATIVE_FUNDING_ROW_COLUMNS);
        FundingFeatureRow { columns }
    }

    /// The template set of the frozen model: column 0 carries the token, column 16
    /// closes the declared window. Without the second template the synthetic model
    /// would declare a narrower window and be refused.
    fn frozen_templates() -> Vec<CrfPattern> {
        vec![
            CrfPattern {
                source: "u:%x[0,0]".into(),
                kind: 'u',
            },
            CrfPattern {
                source: "u:%x[0,16]".into(),
                kind: 'u',
            },
        ]
    }

    /// Synthetic one-token-per-row model over the closed twenty-label dictionary
    /// that forces `labels[row]` at row `row`: every position owns a distinct
    /// unigram observation and that observation weights exactly one label strictly
    /// higher than the rest (no `b:` observation is declared, so transitions are
    /// zero).
    fn model_emitting(labels: &[&str]) -> (Vec<FundingFeatureRow>, CrfModel) {
        let rows: Vec<FundingFeatureRow> = (0..labels.len())
            .map(|index| row(&format!("t{index}")))
            .collect();
        let vocabulary: Vec<String> = NATIVE_FUNDING_LABELS
            .iter()
            .map(|label| (*label).to_string())
            .collect();
        let label_count = vocabulary.len();
        let observations = (0..labels.len())
            .map(|index| format!("u:t{index}"))
            .collect::<Vec<_>>();
        let mut u_offsets = Vec::with_capacity(observations.len());
        let mut weights = BTreeMap::new();
        for (position, label) in labels.iter().enumerate() {
            let offset = position * label_count;
            u_offsets.push(Some(offset));
            let target = NATIVE_FUNDING_LABELS
                .iter()
                .position(|known| known == label)
                .expect("requested label is part of the closed dictionary");
            weights.insert(offset + target, 3.0);
        }
        let model = CrfModel {
            model_type: 2,
            labels: vocabulary,
            observations,
            patterns: frozen_templates(),
            u_offsets,
            b_offsets: vec![None; labels.len()],
            weights,
            nftr: labels.len() * label_count,
        };
        (rows, model)
    }

    fn decoded(labels: &[&str]) -> NativeFundingDecodeOutcome {
        let (rows, model) = model_emitting(labels);
        decode_funding_sequence(&rows, &model).expect("synthetic sequence must decode")
    }

    fn entries(labels: &[&str]) -> Vec<NativeFundingEntry> {
        match decoded(labels) {
            NativeFundingDecodeOutcome::Decoded { entries, .. } => entries,
            other => panic!("a funding sequence must decode, got {other:?}"),
        }
    }

    #[test]
    fn the_dictionary_is_the_ten_native_roles_in_both_spellings() {
        assert_eq!(NATIVE_FUNDING_LABELS.len(), 20);
        assert_eq!(NATIVE_FUNDING_BASE_LABELS.len(), 10);
        assert_eq!(NativeFundingRole::ALL.len(), 10);
        for label in NATIVE_FUNDING_LABELS {
            assert!(funding_label_is_admissible(label));
        }
        assert!(!funding_label_is_admissible("<funderAbbrv>"));
        assert!(!funding_label_is_admissible(""));
        // Every base role is spelled exactly twice (bare + `I-`).
        for base in NATIVE_FUNDING_BASE_LABELS {
            assert_eq!(
                NATIVE_FUNDING_LABELS
                    .iter()
                    .filter(|label| **label == base
                        || crate::native_bibliography_sequence::base_label(label) == base)
                    .count(),
                2,
                "role {base} must carry a bare and an I- spelling"
            );
        }
        assert_eq!(NATIVE_FUNDING_UNMODELED_JAVA_LABELS.len(), 4);
        for label in NATIVE_FUNDING_UNMODELED_JAVA_LABELS {
            assert!(!funding_label_is_admissible(label));
            assert!(funding_role_of_label(label).is_none());
        }
    }

    #[test]
    fn the_role_table_round_trips_every_base_label() {
        for base in NATIVE_FUNDING_BASE_LABELS {
            let role = funding_role_of_label(base).expect("base label classifies");
            assert_eq!(role.base_label(), base);
            assert_eq!(funding_role_of_label(&format!("I-{base}")), Some(role));
        }
        assert_eq!(
            funding_role_of_label("<grantNumber>"),
            Some(NativeFundingRole::GrantNumber)
        );
        assert_eq!(funding_role_of_label("<not-a-role>"), None);
        let scalars: Vec<&str> = NativeFundingRole::ALL
            .iter()
            .filter(|role| role.is_scalar())
            .map(|role| role.base_label())
            .collect();
        assert_eq!(
            scalars,
            [
                "<funderName>",
                "<grantNumber>",
                "<programName>",
                "<grantName>",
                "<projectName>"
            ]
        );
    }

    #[test]
    fn every_scalar_and_list_role_maps_to_its_own_field() {
        let entry = entries(&[
            "<funderName>",
            "<grantName>",
            "<grantNumber>",
            "<programName>",
            "<projectName>",
            "<person>",
            "<affiliation>",
            "<institution>",
        ]);
        assert_eq!(entry.len(), 1);
        let entry = &entry[0];
        assert_eq!(entry.funder_name.as_deref(), Some("t0"));
        assert_eq!(entry.grant_name.as_deref(), Some("t1"));
        assert_eq!(entry.grant_number.as_deref(), Some("t2"));
        assert_eq!(entry.program_name.as_deref(), Some("t3"));
        assert_eq!(entry.project_name.as_deref(), Some("t4"));
        assert_eq!(entry.persons, ["t5"]);
        assert_eq!(entry.affiliations, ["t6"]);
        assert_eq!(entry.institutions, ["t7"]);
        assert!(entry.infrastructures.is_empty());
        assert!(entry.is_valid());
    }

    /// The two predicates are deliberately different: a person-only paragraph is
    /// not a valid `Funding` record but it *is* an observation, and the flattened
    /// entry is the only place to keep it.
    #[test]
    fn a_list_only_record_is_emitted_though_it_is_not_a_valid_funding_record() {
        let observed = entries(&["<person>"]);
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].persons, ["t0"]);
        assert!(
            !observed[0].is_valid(),
            "person is not part of Java's Funding.isValid()"
        );
        assert!(observed[0].carries_any_observation());

        let empty = NativeFundingEntry::default();
        assert!(!empty.is_valid());
        assert!(!empty.carries_any_observation());
    }

    /// The split is Java's branch shape: a repeat of a scalar role whose own slot
    /// already carries text closes the open record and opens the next one.
    ///
    /// Java's trailing `&& funding.isValid()` cannot be told apart from that guard
    /// by any input (every guarded field is itself a term of `isValid()`), so it is
    /// kept for fidelity and this test pins the observable verdict only.
    #[test]
    fn a_repeated_scalar_field_splits_and_a_filled_list_does_not() {
        // Two grant-number clusters (the `I-` spelling opens the second one): the
        // repeat closes the valid record and opens the next.
        let split = entries(&["<grantNumber>", "I-<grantNumber>"]);
        assert_eq!(split.len(), 2);
        assert_eq!(split[0].grant_number.as_deref(), Some("t0"));
        assert_eq!(split[1].grant_number.as_deref(), Some("t1"));
        // The new record copied no funder: Java's grant-number "factorization"
        // copy is a named residual of this band.
        assert_eq!(split[1].funder_name, None);

        // A repeat after a *different* scalar still splits on the repeated slot.
        let after_other = entries(&["<grantNumber>", "<grantName>", "I-<grantNumber>"]);
        assert_eq!(after_other.len(), 2);
        assert_eq!(after_other[0].grant_number.as_deref(), Some("t0"));
        assert_eq!(after_other[0].grant_name.as_deref(), Some("t1"));
        assert_eq!(after_other[1].grant_number.as_deref(), Some("t2"));
        assert_eq!(after_other[1].grant_name, None);

        // A filled list slot does not split: only the five scalar roles do.
        let list_first = entries(&["<person>", "I-<grantNumber>"]);
        assert_eq!(list_first.len(), 1);
        assert_eq!(list_first[0].persons, ["t0"]);
        assert_eq!(list_first[0].grant_number.as_deref(), Some("t1"));

        // An identical bare repeat is one cluster under the clusteror rule, so it
        // joins into one value instead of splitting.
        let merged = entries(&["<grantNumber>", "<grantNumber>", "<programName>"]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].grant_number.as_deref(), Some("t0 t1"));
        assert_eq!(merged[0].program_name.as_deref(), Some("t2"));
    }

    /// A repeat of one scalar role while a *different* scalar is open does not
    /// split: the condition is the repeated slot's own content, not "any field".
    #[test]
    fn a_repeat_of_a_different_scalar_role_overwrites_without_splitting() {
        let single = entries(&["<grantNumber>", "<grantName>", "<programName>"]);
        assert_eq!(single.len(), 1);
        assert_eq!(single[0].grant_number.as_deref(), Some("t0"));
        assert_eq!(single[0].grant_name.as_deref(), Some("t1"));
        assert_eq!(single[0].program_name.as_deref(), Some("t2"));
    }

    /// A list role never splits the record: two `<person>` clusters accumulate.
    #[test]
    fn repeated_list_roles_accumulate_instead_of_splitting() {
        let entry = entries(&[
            "<funderName>",
            "<person>",
            "I-<person>",
            "<affiliation>",
            "I-<affiliation>",
        ]);
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].persons, ["t1", "t2"]);
        assert_eq!(entry[0].affiliations, ["t3", "t4"]);
    }

    /// `<infrastructure>` is Java's `setInfrastructure(true)`: the same text lands
    /// in the institution list and is addressable as the infrastructure subset.
    #[test]
    fn infrastructure_marks_the_institution_it_projects() {
        let entry = entries(&["<institution>", "<infrastructure>"]);
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].institutions, ["t0", "t1"]);
        assert_eq!(entry[0].infrastructures, ["t1"]);
    }

    #[test]
    fn other_is_counted_and_never_becomes_a_field() {
        let (rows, model) = model_emitting(&["<other>", "<funderName>"]);
        let outcome = decode_funding_sequence(&rows, &model).unwrap();
        match outcome {
            NativeFundingDecodeOutcome::Decoded {
                entries,
                other_count,
                ..
            } => {
                assert_eq!(other_count, 1);
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].funder_name.as_deref(), Some("t1"));
            }
            other => panic!("expected a decode, got {other:?}"),
        }

        // The count is what keeps an unplaceable observation from vanishing: with
        // no field at all the decode is still a `Decoded`, not an absence.
        // (`<other>` carries no field, so the alternative would be an entity list
        // that is empty for a family the model positively labelled.)
        let only_other = decoded(&["<other>"]);
        assert_eq!(
            only_other,
            NativeFundingDecodeOutcome::Decoded {
                labels: vec!["<other>".to_string()],
                entries: Vec::new(),
                other_count: 1,
            }
        );
    }

    #[test]
    fn an_empty_decode_is_not_observed_rather_than_zero_funders() {
        let rows: Vec<FundingFeatureRow> = Vec::new();
        assert_eq!(
            funding_entries_from_decoded(&[], &rows),
            Ok(NativeFundingDecodeOutcome::NotObserved {
                reason: NativeFundingAbsence::NoFundingLabels,
            })
        );
    }

    /// A decode whose clusters all carried empty text is an absence with its own
    /// name, never a successful empty entity list.
    ///
    /// The assertion is label-agnostic: whichever label the model emits, every
    /// role needs non-empty cluster text to produce a field (or to be counted), so
    /// an all-blank decode can never become a successful empty entity list.
    #[test]
    fn a_decode_with_no_liftable_content_is_an_absence() {
        let (mut rows, model) = model_emitting(&["<funderName>", "<other>"]);
        for row in &mut rows {
            row.columns[0] = String::new();
        }
        assert_eq!(
            decode_funding_sequence(&rows, &model),
            Ok(NativeFundingDecodeOutcome::NotObserved {
                reason: NativeFundingAbsence::NoLiftableContent,
            })
        );
    }

    #[test]
    fn refuses_empty_sequence_before_any_model_work() {
        let (_, model) = model_emitting(&["<other>"]);
        assert_eq!(
            decode_funding_sequence(&[], &model),
            Err(NativeFundingSequenceError::EmptySequence)
        );
    }

    #[test]
    fn refuses_rows_that_do_not_carry_the_18_column_abi() {
        let (_, model) = model_emitting(&["<other>"]);
        for width in [17usize, 19] {
            let mut bad = row("t0");
            while bad.width() < width {
                bad.columns.push("0".into());
            }
            bad.columns.truncate(width);
            assert_eq!(
                decode_funding_sequence(&[bad], &model),
                Err(NativeFundingSequenceError::WrongArity {
                    row: 0,
                    actual: width,
                    expected: NATIVE_FUNDING_ROW_COLUMNS,
                })
            );
        }
    }

    /// The label slot must carry the inference literal: a gold tag there would be a
    /// training-shape row, and the T05 renderer cannot build one.
    #[test]
    fn refuses_a_training_shape_label_slot() {
        let (_, model) = model_emitting(&["<other>"]);
        let mut bad = row("t0");
        bad.columns[NATIVE_FUNDING_LABEL_COLUMN_INDEX] = "<grantNumber>".into();
        assert_eq!(
            decode_funding_sequence(&[bad], &model),
            Err(NativeFundingSequenceError::WrongLabelSlot {
                row: 0,
                actual: "<grantNumber>".to_string(),
            })
        );
    }

    #[test]
    fn refuses_a_model_whose_label_block_has_the_wrong_size() {
        let (rows, mut model) = model_emitting(&["<other>"]);
        model.labels.pop();
        assert_eq!(
            decode_funding_sequence(&rows, &model),
            Err(NativeFundingSequenceError::ModelLabelCount {
                expected: 20,
                actual: 19,
            })
        );
    }

    #[test]
    fn refuses_a_same_sized_block_that_diverges_from_the_dictionary() {
        let (rows, mut model) = model_emitting(&["<other>"]);
        let index = model
            .labels
            .iter()
            .position(|label| label == "<other>")
            .expect("the dictionary carries <other>");
        model.labels[index] = "<funderAbbrv>".into();
        assert_eq!(model.labels.len(), 20);
        assert_eq!(
            decode_funding_sequence(&rows, &model),
            Err(NativeFundingSequenceError::ModelLabelMismatch {
                missing: vec!["<other>".to_string()],
                unexpected: vec!["<funderAbbrv>".to_string()],
            })
        );
    }

    /// The frozen window is `0..=16`: a narrower model and a model that reaches the
    /// label slot are both refused, and the refusal happens *before* the decode —
    /// the 17-case would otherwise expand cleanly, because an 18-column row really
    /// does carry column 17.
    #[test]
    fn refuses_a_model_whose_referenced_window_is_not_the_frozen_one() {
        for declared in [15usize, 17] {
            let (rows, mut model) = model_emitting(&["<other>"]);
            model.patterns[1] = CrfPattern {
                source: format!("u:%x[0,{declared}]"),
                kind: 'u',
            };
            assert_eq!(
                decode_funding_sequence(&rows, &model),
                Err(NativeFundingSequenceError::DeclaredColumnMismatch {
                    declared: Some(declared),
                    expected: NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX,
                })
            );
        }

        // The guard is load-bearing: the decoder would accept this row.
        let (rows, mut model) = model_emitting(&["<other>"]);
        model.patterns[1] = CrfPattern {
            source: "u:%x[0,17]".into(),
            kind: 'u',
        };
        let rendered = render_funding_feature_text(&rows).unwrap();
        assert_eq!(da_ports::map_input(&model, &rendered).unwrap().len(), 1);

        // A model that references no column at all cannot be the frozen model.
        let (rows, mut model) = model_emitting(&["<other>"]);
        model.patterns = vec![CrfPattern {
            source: "u:literal".into(),
            kind: 'u',
        }];
        assert_eq!(
            decode_funding_sequence(&rows, &model),
            Err(NativeFundingSequenceError::DeclaredColumnMismatch {
                declared: None,
                expected: NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX,
            })
        );
    }

    /// A malformed template is a typed `Model` refusal, not a panic and not a
    /// silently skipped reference.
    #[test]
    fn surfaces_a_malformed_template_as_a_typed_refusal() {
        for source in ["u:%x[0,16", "u:%x[0]", "u:%x[0,1,6]", "u:%x[0,x]"] {
            let (rows, mut model) = model_emitting(&["<other>"]);
            model.patterns[1] = CrfPattern {
                source: source.into(),
                kind: 'u',
            };
            assert!(
                matches!(
                    decode_funding_sequence(&rows, &model),
                    Err(NativeFundingSequenceError::Model { .. })
                ),
                "{source} must be a typed model refusal"
            );
        }
    }

    /// The four declared-but-unmodelled Java labels are refused *by name*, so a
    /// consumer can tell "the model block drifted" from "a foreign label arrived".
    #[test]
    fn refuses_the_unmodeled_java_labels_by_name() {
        let rows = vec![row("t0")];
        for label in NATIVE_FUNDING_UNMODELED_JAVA_LABELS {
            assert_eq!(
                funding_entries_from_decoded(&[label.to_string()], &rows),
                Err(NativeFundingSequenceError::UnmodeledJavaLabel {
                    label: label.to_string(),
                    row: 0,
                })
            );
            // The `I-` spelling classifies the same while reporting what was seen.
            assert_eq!(
                funding_entries_from_decoded(&[format!("I-{label}")], &rows),
                Err(NativeFundingSequenceError::UnmodeledJavaLabel {
                    label: format!("I-{label}"),
                    row: 0,
                })
            );
        }
        assert_eq!(
            funding_entries_from_decoded(&["I-<funderFull>".to_string()], &rows),
            Err(NativeFundingSequenceError::UnmodeledLabel {
                label: "I-<funderFull>".to_string(),
                row: 0,
            })
        );
        assert_eq!(
            funding_entries_from_decoded(&["<other>".to_string()], &[]),
            Err(NativeFundingSequenceError::CountMismatch {
                expected: 1,
                actual: 0,
            })
        );
    }

    /// The run cap refuses instead of truncating.
    #[test]
    fn the_row_cap_refuses_rather_than_truncating() {
        let (_, model) = model_emitting(&["<other>"]);
        let over = vec![row("t0"); MAX_NATIVE_FUNDING_ROWS + 1];
        assert_eq!(
            decode_funding_sequence(&over, &model),
            Err(NativeFundingSequenceError::TooManyRows {
                actual: MAX_NATIVE_FUNDING_ROWS + 1,
                maximum: MAX_NATIVE_FUNDING_ROWS,
            })
        );
    }

    /// Equal inputs produce an equal decode, byte for byte through the stable serde
    /// projection, and the digest is a stable redaction-safe identity.
    #[test]
    fn the_decode_is_deterministic_and_the_digest_is_stable() {
        let labels = ["<funderName>", "I-<grantNumber>", "<person>"];
        let first = decoded(&labels);
        let second = decoded(&labels);
        assert_eq!(first, second);
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );

        let entry = entries(&["<funderName>", "<person>"]);
        let digest = entry[0].digest();
        assert_eq!(digest.len(), 71);
        assert!(digest.starts_with("sha256:"));
        assert_eq!(
            digest,
            NativeFundingEntry {
                funder_name: Some("t0".into()),
                persons: vec!["t1".into()],
                ..NativeFundingEntry::default()
            }
            .digest()
        );
        let other = entries(&["<funderName>", "<affiliation>"]);
        assert_ne!(
            digest,
            other[0].digest(),
            "a different list must relocate the digest"
        );
        assert!(
            !digest.contains("t0") && !digest.contains("t1"),
            "the digest must not embed the raw value"
        );
    }

    #[test]
    fn the_schema_version_names_the_surface() {
        assert_eq!(
            NATIVE_FUNDING_SEQUENCE_SCHEMA_VERSION,
            "da-parse.native-funding-sequence/v1"
        );
    }
}
