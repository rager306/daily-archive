//! S03/T04 (M514-1lss2s) — typed receipt for the declared Java-free slice (R140).
//!
//! The milestone success criterion is "per-stage Java calls = 0 on the declared
//! vertical slice". A declaration is not a measurement, so this module turns the
//! existing DATA declaration (`data/parse/layout_bakeoff.yaml#java_free_vertical_slice`)
//! into a *measurable* contract and a *reviewable* receipt:
//!
//! - the slice boundary is read from DATA (this module never restates it),
//! - stage ids are resolved by pointer from
//!   `data/parse/grobid_compatibility.yaml#defaults.cascade_stages`,
//! - the stage set is checked to be exactly the cascade stage set, so no stage
//!   can be dropped silently,
//! - every stage carries a **typed** disposition: {@link JavaStageCount::Observed}
//!   (a real measurement under an installed guard), {@link JavaStageCount::NotObserved}
//!   (a typed absence) or {@link JavaStageCount::OutOfSlice} (a typed reason).
//!
//! Three rules are load-bearing and mirrored from the rest of the measurement
//! plane:
//!
//! 1. **A zero is only reachable as `Observed { attempts: 0 }` under an installed
//!    guard.** An absent guard yields `NotObserved`, never a fabricated zero —
//!    the same discipline as `JavaInvocationObservation::NotObserved` and
//!    `BATCH_BUDGET_ZERO_IS_NOT_A_MEASUREMENT`.
//! 2. **An out-of-slice stage carries a reason, never a count.** A numeric
//!    disposition on a stage outside the slice is a refusal, not a zero.
//! 3. **A document outside the pinned identities, or a sha256 that drifted, is a
//!    typed refusal.** The receipt is bound to the corpus pin and to the
//!    revision of the declaration carrier, so it cannot be replayed against a
//!    different corpus.
//!
//! The module is pure `std`/`serde`/`serde_yaml`/`sha2`: it compiles in the
//! default feature set (no `pdf-layout`) and owns no route, no Java and no
//! graph write. It is a diagnostic receipt, not a product claim (D499).

use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

/// Schema version of the emitted receipt artifact.
pub const RECEIPT_SCHEMA_VERSION: &str = "java-free-vertical-slice-receipt.v1";
/// Repository-relative carrier of the slice declaration.
pub const SLICE_DECLARATION_DATA_REL: &str = "data/parse/layout_bakeoff.yaml";
/// Pointer to the declaration block inside the carrier.
pub const SLICE_DECLARATION_POINTER: &str = "java_free_vertical_slice";
/// Repository-relative carrier of the cascade stage ids.
pub const COMPATIBILITY_DATA_REL: &str = "data/parse/grobid_compatibility.yaml";
/// Default artifact path of the emitted receipt.
pub const DEFAULT_ARTIFACT_REL: &str = "artifacts/parse_compat/java_free_vertical_slice.json";
/// Declared schema version of the slice block.
pub const SLICE_SCHEMA_VERSION: &str = "java-free-vertical-slice.v1";
/// The only claim scope the slice may carry. Anything else is a product claim.
pub const CLAIM_SCOPE_DIAGNOSTIC_ONLY: &str = "diagnostic_only";
/// The only product-coverage effect the slice may declare.
pub const PRODUCT_COVERAGE_EFFECT_NONE: &str = "none";
/// In-slice disposition DATA must carry for a counted zero.
pub const DISPOSITION_OBSERVED_ZERO_UNDER_GUARD: &str = "observed_zero_under_guard";
/// Reason for a stage the slice does not count (the admission boundary).
pub const NOT_OBSERVED_BOUNDARY_ADMISSION_NOT_COUNTED: &str = "boundary_admission_not_counted";
/// Reason an in-slice stage cannot report a measured zero.
pub const NOT_OBSERVED_GUARD_NOT_INSTALLED: &str = "guard_not_installed";
/// Reason a lane's lattice was rejected by native segmentation. Typed absence.
pub const SEGMENTATION_NOT_OBSERVED_REJECTED: &str = "segmenter_rejected_lattice";
/// The Java seam the guard observes (`ParseBackend::fetch_tei`).
pub const OBSERVER_KIND: &str = "java_transport_fetch_tei";
/// Locks that must stay exactly as declared; the slice never relaxes one.
pub const REQUIRED_LOCKS: [(&str, &str); 5] = [
    ("deployed", "false"),
    ("import_eligible", "false"),
    ("auto_promote", "forbidden"),
    ("production_route", "forbidden"),
    ("graph_write", "forbidden"),
];
/// Markers that must never reach the emitted artifact. Payload text, TEI bytes,
/// service endpoints and host filesystem layouts are all represented here.
pub const LEAK_MARKERS: [&str; 12] = [
    "<TEI",
    "<tei",
    "http://",
    "https://",
    "127.0.0.1",
    "localhost",
    "/root",
    "/home",
    "/tmp",
    ".wapiti",
    "#mdl#",
    "SENTINEL-NOT-FOR-ARTIFACT",
];

// ───────────────────────────── errors ─────────────────────────────

/// Typed refusals. Every variant is a refusal, never a silent downgrade: the
/// receipt either justifies what it says or says why it cannot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaFreeSliceError {
    /// A carrier could not be read or parsed.
    DataUnreadable { path: String, reason: String },
    /// A required field is absent. Loss is not silence.
    DataFieldMissing { path: String },
    /// A field is present but not the expected shape.
    DataFieldType { path: String, expected: String },
    /// The declaration block is absent from its carrier.
    BlockMissing { path: String },
    /// A declared identity does not match what the contract requires.
    SchemaDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// The slice tried to widen its claim beyond `diagnostic_only`.
    ClaimScopeDrift { found: String },
    /// The slice declared a product-coverage effect while claiming to have none.
    ProductCoverageClaimed { found: String },
    /// A lock the slice never relaxes was relaxed.
    LockDrift {
        lock: String,
        expected: String,
        found: String,
    },
    /// The authority block drifted from the carrier it points at.
    AuthorityDrift {
        field: String,
        expected: String,
        found: String,
    },
    /// A stage name pointer is not the cascade DATA pointer it must reuse.
    StageIdSourceDrift {
        stage: String,
        expected: String,
        found: String,
    },
    /// The declaration names a stage the cascade does not declare.
    UnknownStage { stage: String },
    /// The cascade declares a stage the declaration never classifies.
    UnclassifiedStage { stage: String },
    /// The same stage is classified twice.
    DuplicateStage { stage: String },
    /// A stage name pointer resolved to nothing.
    StageNameMissing { stage: String, pointer: String },
    /// The slice declared Java as required for an in-slice stage.
    InSliceJavaRequired { stage: String },
    /// An out-of-slice reason is outside the closed vocabulary.
    VocabularyDrift { reason: String },
    /// An in-slice stage carries a disposition other than the counted zero.
    DispositionDrift { stage: String, disposition: String },
    /// A lane is not one of the donors the slice may execute.
    UnknownLane { lane: String },
    /// The declaration's in-slice lane is not in the carrier's required lanes.
    LaneNotDeclared { lane: String },
    /// A declared corpus document is not consistent internally or with its pin.
    CorpusBindingMismatch { document: String, reason: String },
    /// The run observed a document the declaration does not pin.
    UnknownDocument { document: String },
    /// The declaration pins a document the run never observed.
    MissingDocument { document: String },
    /// The observed page count drifted from the pinned one.
    PageDenominatorDrift {
        document: String,
        expected: u64,
        found: u64,
    },
    /// The observed lane set for a document is not the declared in-slice set.
    LaneSetMismatch { document: String },
    /// A corpus page total drifted from the document pages it restates.
    CorpusPageDrift { expected: u64, found: u64 },
    /// The zero was claimed without an installed guard.
    GuardNotInstalled,
    /// The guard would have rotated a cache key, so its zero is unusable.
    GuardIdentityNotIsolated,
    /// Java was consulted during the slice; the slice is not Java-free.
    ObservedJavaOnSlice { attempts: u64 },
    /// A stage reports a measurement while the guard was not installed.
    ObservedWithoutGuard { stage: String },
    /// An out-of-slice stage carries a numeric count instead of a reason.
    OutOfSliceStageCounted { stage: String },
    /// A receipt stage row drifted from the declared stage matrix.
    StageMatrixMismatch { stage: String, reason: String },
    /// The artifact leaked a payload marker.
    RedactionViolation { marker: String },
    /// The artifact could not be serialized.
    ArtifactSerialization { reason: String },
}

impl fmt::Display for JavaFreeSliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataUnreadable { path, reason } => {
                write!(f, "java-free slice: `{path}` is unreadable: {reason}")
            }
            Self::DataFieldMissing { path } => {
                write!(f, "java-free slice: required field `{path}` is missing")
            }
            Self::DataFieldType { path, expected } => {
                write!(f, "java-free slice: field `{path}` must be {expected}")
            }
            Self::BlockMissing { path } => write!(
                f,
                "java-free slice: `{path}` is missing; a slice with no boundary is not a slice"
            ),
            Self::SchemaDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "java-free slice: `{field}` is `{found}` but `{expected}` is required"
            ),
            Self::ClaimScopeDrift { found } => write!(
                f,
                "java-free slice: claim_scope is `{found}`; only \
                 `{CLAIM_SCOPE_DIAGNOSTIC_ONLY}` is allowed (D499)"
            ),
            Self::ProductCoverageClaimed { found } => write!(
                f,
                "java-free slice: product_coverage_effect is `{found}`; \
                 a scoped diagnostic must declare `{PRODUCT_COVERAGE_EFFECT_NONE}`"
            ),
            Self::LockDrift {
                lock,
                expected,
                found,
            } => write!(
                f,
                "java-free slice: lock `{lock}` is `{found}` but `{expected}` is required"
            ),
            Self::AuthorityDrift {
                field,
                expected,
                found,
            } => write!(
                f,
                "java-free slice: authority `{field}` is `{found}` but `{expected}` is required"
            ),
            Self::StageIdSourceDrift {
                stage,
                expected,
                found,
            } => write!(
                f,
                "java-free slice: stage `{stage}` name pointer is `{found}` \
                 but must reuse `{expected}` (no second stage list)"
            ),
            Self::UnknownStage { stage } => write!(
                f,
                "java-free slice: stage `{stage}` is not declared by the cascade"
            ),
            Self::UnclassifiedStage { stage } => write!(
                f,
                "java-free slice: cascade stage `{stage}` has no disposition; \
                 a dropped stage is not silence"
            ),
            Self::DuplicateStage { stage } => {
                write!(f, "java-free slice: stage `{stage}` is classified twice")
            }
            Self::StageNameMissing { stage, pointer } => write!(
                f,
                "java-free slice: stage `{stage}` name pointer `{pointer}` resolves to nothing"
            ),
            Self::InSliceJavaRequired { stage } => write!(
                f,
                "java-free slice: in-slice stage `{stage}` declares java_required; \
                 the slice is Java-free by construction"
            ),
            Self::VocabularyDrift { reason } => write!(
                f,
                "java-free slice: out-of-slice reason `{reason}` is outside the closed vocabulary"
            ),
            Self::DispositionDrift { stage, disposition } => write!(
                f,
                "java-free slice: in-slice stage `{stage}` disposition is `{disposition}` \
                 but `{DISPOSITION_OBSERVED_ZERO_UNDER_GUARD}` is required"
            ),
            Self::UnknownLane { lane } => write!(
                f,
                "java-free slice: lane `{lane}` is not a donor this slice may execute"
            ),
            Self::LaneNotDeclared { lane } => write!(
                f,
                "java-free slice: lane `{lane}` is not among the carrier's required lanes"
            ),
            Self::CorpusBindingMismatch { document, reason } => write!(
                f,
                "java-free slice: corpus document `{document}` is not bound: {reason}"
            ),
            Self::UnknownDocument { document } => write!(
                f,
                "java-free slice: the run observed `{document}`, which the declaration never pins"
            ),
            Self::MissingDocument { document } => write!(
                f,
                "java-free slice: the declaration pins `{document}` but the run never observed it"
            ),
            Self::PageDenominatorDrift {
                document,
                expected,
                found,
            } => write!(
                f,
                "java-free slice: document `{document}` observed `{found}` pages \
                 but `{expected}` are pinned"
            ),
            Self::LaneSetMismatch { document } => write!(
                f,
                "java-free slice: document `{document}` lane rows are not the declared in-slice set"
            ),
            Self::CorpusPageDrift { expected, found } => write!(
                f,
                "java-free slice: corpus page denominator is `{found}` \
                 but the pinned documents sum to `{expected}`"
            ),
            Self::GuardNotInstalled => write!(
                f,
                "java-free slice: a counted zero requires an installed guard; \
                 an unmeasured seam is `NotObserved`, never zero"
            ),
            Self::GuardIdentityNotIsolated => write!(
                f,
                "java-free slice: the guard rotates the cache identity; \
                 its zero cannot be attributed to a stable transport"
            ),
            Self::ObservedJavaOnSlice { attempts } => write!(
                f,
                "java-free slice: `{attempts}` Java consult(s) were observed on the slice; \
                 the slice must not consult Java"
            ),
            Self::ObservedWithoutGuard { stage } => write!(
                f,
                "java-free slice: stage `{stage}` reports a measurement without guard provenance"
            ),
            Self::OutOfSliceStageCounted { stage } => write!(
                f,
                "java-free slice: out-of-slice stage `{stage}` carries a numeric count; \
                 it must carry a typed reason"
            ),
            Self::StageMatrixMismatch { stage, reason } => write!(
                f,
                "java-free slice: stage `{stage}` drifted from the declared matrix: {reason}"
            ),
            Self::RedactionViolation { marker } => write!(
                f,
                "java-free slice: the artifact leaked `{marker}`; \
                 only counts, digests and closed tokens may leave"
            ),
            Self::ArtifactSerialization { reason } => {
                write!(
                    f,
                    "java-free slice: artifact serialization failed: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for JavaFreeSliceError {}

// ───────────────────────── declaration ─────────────────────────

/// Which part of the cascade a stage belongs to, as the declaration classifies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageScope {
    /// The identity/profile admission boundary (`P0`).
    IdentityAdmission,
    /// Inside the counted Java-free slice (`P1`, `P2`).
    InSlice,
    /// Outside the slice, carrying a typed reason (`P3`..`P9`).
    OutOfSlice,
}

impl StageScope {
    /// Stable snake_case name (diagnostics only, never identity).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IdentityAdmission => "identity_admission",
            Self::InSlice => "in_slice",
            Self::OutOfSlice => "out_of_slice",
        }
    }
}

/// A corpus document exactly as the declaration pins it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredDocument {
    /// Stable document id (`corpus.documents[].id`).
    pub id: String,
    /// Repository-relative source PDF path.
    pub source_pdf: String,
    /// Pinned sha256 (bare hex, no `sha256:` prefix).
    pub sha256: String,
    /// Pinned page count.
    pub pages: u64,
}

/// One cascade stage, with the name resolved from the cascade DATA by pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredStage {
    /// Stage id (`P0`..`P9`).
    pub stage_id: String,
    /// Stage name resolved by pointer from the cascade DATA.
    pub stage_name: String,
    /// Where the declaration places the stage.
    pub scope: StageScope,
    /// Whether the stage needs Java (a fact of the declaration, not a choice).
    pub java_required: bool,
    /// The typed reason of an out-of-slice stage, read from the closed vocabulary.
    pub out_of_slice_reason: Option<String>,
}

/// The validated slice declaration: boundary, corpus pin, lanes and stage matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceDeclaration {
    /// Version of the declaration block.
    pub schema_version: String,
    /// Slice identity.
    pub slice_id: String,
    /// ADR that binds the boundary.
    pub adr_ref: String,
    /// Requirements this slice serves.
    pub requirement_ids: Vec<String>,
    /// Claim scope; `diagnostic_only`.
    pub claim_scope: String,
    /// Product-coverage effect; `none`.
    pub product_coverage_effect: String,
    /// Repository-relative declaration carrier.
    pub declaration_manifest: String,
    /// Pointer to the declaration block.
    pub declaration_pointer: String,
    /// Repository-relative cascade stage carrier.
    pub stage_id_source_manifest: String,
    /// Pointer to the cascade stage mapping.
    pub stage_id_source_pointer: String,
    /// Repository-relative corpus carrier.
    pub corpus_manifest: String,
    /// Pointer to the pinned corpus documents.
    pub corpus_pointer: String,
    /// Corpus id.
    pub corpus_id: String,
    /// Declared document count.
    pub corpus_n: usize,
    /// Declared page denominator (sum of document pages).
    pub page_denominator: u64,
    /// Pinned documents.
    pub documents: Vec<DeclaredDocument>,
    /// Lanes the slice declares as in-slice (must be a subset of the required lanes).
    pub lane_ids: Vec<String>,
    /// Required lanes the carrier declares.
    pub required_lanes: Vec<String>,
    /// Cascade stages in cascade order, every one classified.
    pub stages: Vec<DeclaredStage>,
    /// Out-of-slice reason vocabulary read from the declaration.
    pub out_of_slice_reasons: Vec<String>,
    /// Locks as declared (stringified), after the required-lock check.
    pub locks: BTreeMap<String, String>,
}

impl SliceDeclaration {
    /// The in-slice stage ids, in cascade order.
    pub fn in_slice_stage_ids(&self) -> Vec<&str> {
        self.stages
            .iter()
            .filter(|stage| stage.scope == StageScope::InSlice)
            .map(|stage| stage.stage_id.as_str())
            .collect()
    }

    /// The out-of-slice stage ids, in cascade order.
    pub fn out_of_slice_stage_ids(&self) -> Vec<&str> {
        self.stages
            .iter()
            .filter(|stage| stage.scope == StageScope::OutOfSlice)
            .map(|stage| stage.stage_id.as_str())
            .collect()
    }

    /// Look up a classified stage by id.
    pub fn stage(&self, stage_id: &str) -> Option<&DeclaredStage> {
        self.stages.iter().find(|stage| stage.stage_id == stage_id)
    }

    /// Look up a pinned document by id.
    pub fn document(&self, document_id: &str) -> Option<&DeclaredDocument> {
        self.documents
            .iter()
            .find(|document| document.id == document_id)
    }

    /// Lane-page denominator: pages × in-slice lanes.
    pub fn lane_page_denominator(&self) -> u64 {
        self.page_denominator
            .saturating_mul(self.lane_ids.len() as u64)
    }
}

// ───────────────────────── load + validate declaration ─────────────────────────

/// Read the declaration and the cascade stage ids from two YAML carriers.
///
/// The two carriers are passed as text so the caller owns file access and tests
/// can bind them without touching the filesystem.
pub fn load_declaration(
    layout_bakeoff_yaml: &str,
    compatibility_yaml: &str,
) -> Result<SliceDeclaration, JavaFreeSliceError> {
    let layout = parse_yaml(layout_bakeoff_yaml, SLICE_DECLARATION_DATA_REL)?;
    let compatibility = parse_yaml(compatibility_yaml, COMPATIBILITY_DATA_REL)?;

    let block = pointer(
        &layout,
        SLICE_DECLARATION_POINTER,
        SLICE_DECLARATION_DATA_REL,
    )
    .map_err(|error| match error {
        JavaFreeSliceError::DataFieldMissing { .. } => JavaFreeSliceError::BlockMissing {
            path: format!("{SLICE_DECLARATION_DATA_REL}#{SLICE_DECLARATION_POINTER}"),
        },
        other => other,
    })?;

    let schema_version = required_str(block, "schema_version", SLICE_DECLARATION_POINTER)?;
    if schema_version != SLICE_SCHEMA_VERSION {
        return Err(JavaFreeSliceError::SchemaDrift {
            field: format!("{SLICE_DECLARATION_POINTER}.schema_version"),
            expected: SLICE_SCHEMA_VERSION.to_owned(),
            found: schema_version,
        });
    }

    let claim_scope = required_str(block, "claim_scope", SLICE_DECLARATION_POINTER)?;
    if claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(JavaFreeSliceError::ClaimScopeDrift { found: claim_scope });
    }
    let product_coverage_effect =
        required_str(block, "product_coverage_effect", SLICE_DECLARATION_POINTER)?;
    if product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(JavaFreeSliceError::ProductCoverageClaimed {
            found: product_coverage_effect,
        });
    }

    let locks_value = member(block, "locks", SLICE_DECLARATION_POINTER)?;
    let mut locks = BTreeMap::new();
    for (key, expected) in REQUIRED_LOCKS {
        let found = lock_token(member(locks_value, key, "java_free_vertical_slice.locks")?);
        if found != expected {
            return Err(JavaFreeSliceError::LockDrift {
                lock: key.to_owned(),
                expected: expected.to_owned(),
                found,
            });
        }
        locks.insert(key.to_owned(), found);
    }

    // Authority: the block must be the sole boundary source and must name this
    // very carrier, so a second contract file cannot quietly take over.
    let authority = member(block, "authority", SLICE_DECLARATION_POINTER)?;
    let authority_manifest = required_str(authority, "declaration_manifest", "authority")?;
    if authority_manifest != SLICE_DECLARATION_DATA_REL {
        return Err(JavaFreeSliceError::AuthorityDrift {
            field: "declaration_manifest".to_owned(),
            expected: SLICE_DECLARATION_DATA_REL.to_owned(),
            found: authority_manifest,
        });
    }
    let authority_pointer = required_str(authority, "declaration_pointer", "authority")?;
    if authority_pointer != SLICE_DECLARATION_POINTER {
        return Err(JavaFreeSliceError::AuthorityDrift {
            field: "declaration_pointer".to_owned(),
            expected: SLICE_DECLARATION_POINTER.to_owned(),
            found: authority_pointer,
        });
    }
    let sole_source = required_bool(authority, "sole_source", "authority")?;
    if !sole_source {
        return Err(JavaFreeSliceError::AuthorityDrift {
            field: "sole_source".to_owned(),
            expected: "true".to_owned(),
            found: "false".to_owned(),
        });
    }
    let second_contract = required_str(authority, "second_contract_file", "authority")?;
    if second_contract != "forbidden" {
        return Err(JavaFreeSliceError::AuthorityDrift {
            field: "second_contract_file".to_owned(),
            expected: "forbidden".to_owned(),
            found: second_contract,
        });
    }

    // Stage ids are read by pointer from the cascade DATA; this block never
    // restates them.
    let stage_id_source = member(block, "stage_id_source", SLICE_DECLARATION_POINTER)?;
    let stage_id_source_manifest = required_str(stage_id_source, "manifest", "stage_id_source")?;
    if stage_id_source_manifest != COMPATIBILITY_DATA_REL {
        return Err(JavaFreeSliceError::SchemaDrift {
            field: "stage_id_source.manifest".to_owned(),
            expected: COMPATIBILITY_DATA_REL.to_owned(),
            found: stage_id_source_manifest,
        });
    }
    let stage_id_source_pointer = required_str(stage_id_source, "pointer", "stage_id_source")?;
    let cascade = pointer(
        &compatibility,
        &stage_id_source_pointer,
        COMPATIBILITY_DATA_REL,
    )?;
    let cascade_map = as_mapping(cascade, &stage_id_source_pointer)?;
    let cascade_ids = mapping_keys(cascade_map);
    if cascade_ids.is_empty() {
        return Err(JavaFreeSliceError::SchemaDrift {
            field: stage_id_source_pointer.clone(),
            expected: "a non-empty stage mapping".to_owned(),
            found: "empty".to_owned(),
        });
    }

    // Corpus pin: read by pointer so the block never restates sha256 identities.
    let corpus_binding = member(block, "corpus_binding", SLICE_DECLARATION_POINTER)?;
    let corpus_manifest = required_str(corpus_binding, "manifest", "corpus_binding")?;
    if corpus_manifest != SLICE_DECLARATION_DATA_REL {
        return Err(JavaFreeSliceError::SchemaDrift {
            field: "corpus_binding.manifest".to_owned(),
            expected: SLICE_DECLARATION_DATA_REL.to_owned(),
            found: corpus_manifest,
        });
    }
    let corpus_pointer = required_str(corpus_binding, "pointer", "corpus_binding")?;
    let document_count = required_u64(corpus_binding, "document_count", "corpus_binding")? as usize;

    // The pointer names the document list of a corpus block inside the single
    // declaration manifest. Neither the block name nor the list segment is
    // hardcoded: the containing block is derived by stripping the trailing
    // `.documents` segment, and that block must carry the corpus shape
    // (`id` / `corpus_n` / `page_denominator` / `documents`). A pointer that is
    // absent, empty, does not end in `.documents`, escapes the manifest (an
    // empty, `/`, `\` or `#` segment), or resolves to a non-mapping is refused
    // by name rather than silently ignored.
    let corpus_block_pointer =
        corpus_block_pointer(&corpus_pointer).ok_or_else(|| JavaFreeSliceError::SchemaDrift {
            field: "corpus_binding.pointer".to_owned(),
            expected: "a dotted pointer to a corpus block's `.documents` list".to_owned(),
            found: corpus_pointer.clone(),
        })?;
    let corpus_value = pointer(&layout, &corpus_block_pointer, SLICE_DECLARATION_DATA_REL)
        .map_err(|_| JavaFreeSliceError::SchemaDrift {
            field: "corpus_binding.pointer".to_owned(),
            expected: format!("a corpus block at `{corpus_block_pointer}`"),
            found: corpus_pointer.clone(),
        })?;
    as_mapping(corpus_value, &corpus_block_pointer).map_err(|_| {
        JavaFreeSliceError::SchemaDrift {
            field: "corpus_binding.pointer".to_owned(),
            expected: format!("`{corpus_block_pointer}` to be a corpus mapping"),
            found: corpus_pointer.clone(),
        }
    })?;
    let corpus_id = required_str(corpus_value, "id", &corpus_block_pointer)?;
    let corpus_n = required_u64(corpus_value, "corpus_n", &corpus_block_pointer)? as usize;
    let page_denominator = required_u64(corpus_value, "page_denominator", &corpus_block_pointer)?;
    // The block must actually carry the list the pointer claims; an absent list
    // is refused here, not discovered later as a silent zero.
    member(corpus_value, "documents", &corpus_block_pointer)?;
    if corpus_n != document_count {
        return Err(JavaFreeSliceError::CorpusBindingMismatch {
            document: corpus_block_pointer.clone(),
            reason: format!("corpus_n is {corpus_n} but document_count is {document_count}"),
        });
    }
    let documents = read_declared_documents(&layout, &corpus_pointer)?;
    if documents.len() != document_count {
        return Err(JavaFreeSliceError::CorpusBindingMismatch {
            document: corpus_block_pointer.clone(),
            reason: format!(
                "document_count is {document_count} but the pointer yields {} documents",
                documents.len()
            ),
        });
    }
    let page_sum = documents.iter().map(|document| document.pages).sum::<u64>();
    if page_sum != page_denominator {
        return Err(JavaFreeSliceError::CorpusPageDrift {
            expected: page_sum,
            found: page_denominator,
        });
    }

    // Required lanes live in the carrier; the slice may only execute a subset.
    let required_lane_value = pointer(&layout, "lanes.required", SLICE_DECLARATION_DATA_REL)?;
    let required_lanes = as_str_vec(required_lane_value, "lanes.required")?;

    let lanes_value = member(block, "lanes_in_slice", SLICE_DECLARATION_POINTER)?;
    let mut lane_ids = Vec::new();
    for (index, lane) in as_sequence(lanes_value, "lanes_in_slice")?
        .iter()
        .enumerate()
    {
        let path = format!("lanes_in_slice[{index}]");
        let lane_id = required_str(lane, "lane_id", &path)?;
        let java_required = required_bool(lane, "java_required", &path)?;
        if java_required {
            return Err(JavaFreeSliceError::InSliceJavaRequired {
                stage: lane_id.clone(),
            });
        }
        if !required_lanes.contains(&lane_id) {
            return Err(JavaFreeSliceError::LaneNotDeclared { lane: lane_id });
        }
        lane_ids.push(lane_id);
    }
    if lane_ids.is_empty() {
        return Err(JavaFreeSliceError::SchemaDrift {
            field: "lanes_in_slice".to_owned(),
            expected: "at least one in-slice lane".to_owned(),
            found: "empty".to_owned(),
        });
    }

    let out_of_slice_reasons = as_str_vec(
        member(
            block,
            "out_of_slice_reason_vocabulary",
            SLICE_DECLARATION_POINTER,
        )?,
        "out_of_slice_reason_vocabulary",
    )?;

    // Classify every declared stage; a stage named by the declaration but absent
    // from the cascade is an unknown stage, and a cascade stage the declaration
    // never classifies is an unclassified stage. Neither is silence.
    let mut classified: BTreeMap<String, (StageScope, bool, String)> = BTreeMap::new();
    let mut classify = |stage_id: String,
                        scope: StageScope,
                        java_required: bool,
                        disposition: String|
     -> Result<(), JavaFreeSliceError> {
        if !cascade_ids.contains(&stage_id) {
            return Err(JavaFreeSliceError::UnknownStage { stage: stage_id });
        }
        if classified.contains_key(&stage_id) {
            return Err(JavaFreeSliceError::DuplicateStage { stage: stage_id });
        }
        if scope == StageScope::InSlice && java_required {
            return Err(JavaFreeSliceError::InSliceJavaRequired { stage: stage_id });
        }
        classified.insert(stage_id, (scope, java_required, disposition));
        Ok(())
    };

    let admission = member(block, "identity_admission_stage", SLICE_DECLARATION_POINTER)?;
    let admission_id = required_str(admission, "stage_id", "identity_admission_stage")?;
    if !cascade_ids.contains(&admission_id) {
        return Err(JavaFreeSliceError::UnknownStage {
            stage: admission_id,
        });
    }
    check_stage_name_pointer(
        admission,
        &admission_id,
        &stage_id_source_pointer,
        &compatibility,
    )?;
    let admission_disposition = required_str(admission, "disposition", "identity_admission_stage")?;
    if admission_disposition != NOT_OBSERVED_BOUNDARY_ADMISSION_NOT_COUNTED {
        return Err(JavaFreeSliceError::DispositionDrift {
            stage: admission_id,
            disposition: admission_disposition,
        });
    }
    classify(
        admission_id.clone(),
        StageScope::IdentityAdmission,
        false,
        admission_disposition,
    )?;

    for (index, stage) in as_sequence(
        member(block, "stages_in_slice", SLICE_DECLARATION_POINTER)?,
        "stages_in_slice",
    )?
    .iter()
    .enumerate()
    {
        let path = format!("stages_in_slice[{index}]");
        let stage_id = required_str(stage, "stage_id", &path)?;
        if !cascade_ids.contains(&stage_id) {
            return Err(JavaFreeSliceError::UnknownStage { stage: stage_id });
        }
        check_stage_name_pointer(stage, &stage_id, &stage_id_source_pointer, &compatibility)?;
        let java_required = required_bool(stage, "java_required", &path)?;
        let disposition = required_str(stage, "disposition", &path)?;
        if disposition != DISPOSITION_OBSERVED_ZERO_UNDER_GUARD {
            return Err(JavaFreeSliceError::DispositionDrift {
                stage: stage_id,
                disposition,
            });
        }
        if java_required {
            return Err(JavaFreeSliceError::InSliceJavaRequired { stage: stage_id });
        }
        classify(stage_id, StageScope::InSlice, false, disposition)?;
    }

    for (index, stage) in as_sequence(
        member(block, "stages_out_of_slice", SLICE_DECLARATION_POINTER)?,
        "stages_out_of_slice",
    )?
    .iter()
    .enumerate()
    {
        let path = format!("stages_out_of_slice[{index}]");
        let stage_id = required_str(stage, "stage_id", &path)?;
        if !cascade_ids.contains(&stage_id) {
            return Err(JavaFreeSliceError::UnknownStage { stage: stage_id });
        }
        check_stage_name_pointer(stage, &stage_id, &stage_id_source_pointer, &compatibility)?;
        let java_required = required_bool(stage, "java_required", &path)?;
        let reason = required_str(stage, "reason", &path)?;
        if !out_of_slice_reasons.contains(&reason) {
            return Err(JavaFreeSliceError::VocabularyDrift { reason });
        }
        classify(stage_id, StageScope::OutOfSlice, java_required, reason)?;
    }

    for stage_id in &cascade_ids {
        if !classified.contains_key(stage_id) {
            return Err(JavaFreeSliceError::UnclassifiedStage {
                stage: stage_id.clone(),
            });
        }
    }

    let mut stages = Vec::with_capacity(cascade_ids.len());
    for stage_id in &cascade_ids {
        let (scope, java_required, disposition) =
            classified.get(stage_id).cloned().ok_or_else(|| {
                JavaFreeSliceError::UnclassifiedStage {
                    stage: stage_id.clone(),
                }
            })?;
        let row = cascade_map
            .iter()
            .find_map(|(key, value)| match key {
                Value::String(name) if name == stage_id => Some(value),
                _ => None,
            })
            .ok_or_else(|| JavaFreeSliceError::UnknownStage {
                stage: stage_id.clone(),
            })?;
        let stage_name = required_str(row, "grobid", &format!("{stage_id}.grobid"))?;
        stages.push(DeclaredStage {
            stage_id: stage_id.clone(),
            stage_name,
            scope,
            java_required,
            out_of_slice_reason: (scope == StageScope::OutOfSlice).then_some(disposition),
        });
    }

    Ok(SliceDeclaration {
        schema_version,
        slice_id: required_str(block, "slice_id", SLICE_DECLARATION_POINTER)?,
        adr_ref: required_str(block, "adr_ref", SLICE_DECLARATION_POINTER)?,
        requirement_ids: as_str_vec(
            member(block, "requirement_ids", SLICE_DECLARATION_POINTER)?,
            "requirement_ids",
        )?,
        claim_scope,
        product_coverage_effect,
        declaration_manifest: authority_manifest,
        declaration_pointer: authority_pointer,
        stage_id_source_manifest,
        stage_id_source_pointer,
        corpus_manifest,
        corpus_pointer,
        corpus_id,
        corpus_n,
        page_denominator,
        documents,
        lane_ids,
        required_lanes,
        stages,
        out_of_slice_reasons,
        locks,
    })
}

/// Read the declaration from the default repository-relative carriers.
pub fn load_from_root(root: &Path) -> Result<SliceDeclaration, JavaFreeSliceError> {
    let layout = read_carrier(root, SLICE_DECLARATION_DATA_REL)?;
    let compatibility = read_carrier(root, COMPATIBILITY_DATA_REL)?;
    load_declaration(&layout, &compatibility)
}

fn read_carrier(root: &Path, rel: &str) -> Result<String, JavaFreeSliceError> {
    let path = root.join(rel);
    std::fs::read_to_string(&path).map_err(|error| JavaFreeSliceError::DataUnreadable {
        path: path.display().to_string(),
        reason: error.to_string(),
    })
}

fn read_declared_documents(
    layout: &Value,
    pointer_path: &str,
) -> Result<Vec<DeclaredDocument>, JavaFreeSliceError> {
    let sequence = pointer(layout, pointer_path, SLICE_DECLARATION_DATA_REL)?;
    let mut documents = Vec::new();
    for (index, row) in as_sequence(sequence, pointer_path)?.iter().enumerate() {
        let path = format!("{pointer_path}[{index}]");
        let id = required_str(row, "id", &path)?;
        let source_pdf = required_str(row, "source_pdf", &path)?;
        let sha256 = required_str(row, "sha256", &path)?;
        let pages = required_u64(row, "pages", &path)?;
        if source_pdf.starts_with('/') || Path::new(&source_pdf).is_absolute() {
            return Err(JavaFreeSliceError::CorpusBindingMismatch {
                document: id,
                reason: "source_pdf must be repository-relative".to_owned(),
            });
        }
        if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(JavaFreeSliceError::CorpusBindingMismatch {
                document: id,
                reason: "sha256 must be 64 bare hex characters".to_owned(),
            });
        }
        documents.push(DeclaredDocument {
            id,
            source_pdf,
            sha256,
            pages,
        });
    }
    Ok(documents)
}

/// Verify a stage's `name_pointer` really reuses the cascade DATA pointer.
fn check_stage_name_pointer(
    row: &Value,
    stage_id: &str,
    stage_id_source_pointer: &str,
    compatibility: &Value,
) -> Result<(), JavaFreeSliceError> {
    let name_pointer = required_str(row, "name_pointer", stage_id)?;
    let expected = format!("{stage_id_source_pointer}.{stage_id}.grobid");
    if name_pointer != expected {
        return Err(JavaFreeSliceError::StageIdSourceDrift {
            stage: stage_id.to_owned(),
            expected,
            found: name_pointer,
        });
    }
    pointer(compatibility, &name_pointer, COMPATIBILITY_DATA_REL).map_err(|_| {
        JavaFreeSliceError::StageNameMissing {
            stage: stage_id.to_owned(),
            pointer: name_pointer,
        }
    })?;
    Ok(())
}

// ───────────────────────── observed input ─────────────────────────

/// Guard provenance: whether a Java-consult observer was installed on the real
/// transport seam, whether it is identity-isolated, and what it measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuardProvenance {
    /// An observer was installed on `ParseBackend::fetch_tei`.
    pub installed: bool,
    /// Installing the observer did not rotate the cache identity.
    pub identity_isolated: bool,
    /// Consult attempts the installed observer measured. Zero is a measurement.
    pub observed_java_calls: u64,
}

/// One lane's observation for one document, before it becomes a receipt row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedLane {
    /// Lane id, which must be one of the declared in-slice lanes.
    pub lane_id: String,
    /// Pages the lane's layout donor observed.
    pub observed_pages: u64,
    /// Admitted lexical tokens (the P1 denominator).
    pub admitted_lexical_tokens: u64,
    /// Digest of the donor's layout lattice.
    pub lattice_fingerprint: String,
    /// P2 result: the segmenter accepted or rejected this lane's lattice.
    pub segmentation: SegmentationReceipt,
}

/// One document's observation, before it becomes a receipt row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedDocument {
    /// Pinned document id.
    pub id: String,
    /// Bare sha256 of the bytes that were read.
    pub sha256: String,
    /// Pages the layout donor observed.
    pub observed_pages: u64,
    /// Lane rows for this document.
    pub lanes: Vec<ObservedLane>,
}

// ───────────────────────── receipt ─────────────────────────

/// The typed per-stage Java count. A zero is only reachable as
/// `Observed { attempts: 0 }` under an installed guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JavaStageCount {
    /// The stage was measured under an installed guard.
    Observed { attempts: u64 },
    /// The stage was not measured. A typed absence, never a zero.
    NotObserved { reason: String },
    /// The stage is outside the slice and carries a typed reason.
    OutOfSlice { reason: String },
}

impl JavaStageCount {
    /// Stable snake_case name of the state (diagnostics only, never identity).
    pub const fn state(&self) -> &'static str {
        match self {
            Self::Observed { .. } => "observed",
            Self::NotObserved { .. } => "not_observed",
            Self::OutOfSlice { .. } => "out_of_slice",
        }
    }
}

/// One cascade stage in the emitted matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageReceipt {
    /// Stage id (`P0`..`P9`).
    pub stage_id: String,
    /// Stage name, as resolved from the cascade DATA.
    pub stage_name: String,
    /// Where the declaration places the stage.
    pub scope: StageScope,
    /// Whether the declaration says the stage needs Java.
    pub java_required: bool,
    /// The typed disposition.
    pub count: JavaStageCount,
}

/// Binding of the receipt to the carrier it was read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarrierBinding {
    /// Repository-relative carrier path.
    pub manifest: String,
    /// Pointer inside the carrier.
    pub pointer: String,
    /// sha256 of the exact carrier bytes this receipt was derived from.
    pub revision_sha256: String,
}

/// The guard that makes a zero a measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardProvenanceReceipt {
    /// The seam the guard observes.
    pub observer: String,
    /// An observer was installed.
    pub installed: bool,
    /// Installing it did not rotate the cache identity.
    pub identity_isolated: bool,
    /// Measured Java consult attempts.
    pub observed_java_calls: u64,
}

/// A pinned document identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentIdentity {
    /// Document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Bare sha256.
    pub sha256: String,
    /// Pinned pages.
    pub pages: u64,
}

/// P2 result for one lane: a measurement or a typed absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SegmentationReceipt {
    /// `segment_document_full` accepted the lane's lattice.
    Observed {
        assigned_tokens: u64,
        order_digest: String,
    },
    /// The segmenter rejected the lane's lattice. Typed absence, never a zero.
    NotObserved { reason: String },
}

/// One lane's P1 + P2 rows for one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneReceipt {
    /// Lane id.
    pub lane_id: String,
    /// Pages the donor observed.
    pub observed_pages: u64,
    /// Admitted lexical tokens at P1.
    pub admitted_lexical_tokens: u64,
    /// Donor lattice digest.
    pub lattice_fingerprint: String,
    /// P2 disposition.
    pub segmentation: SegmentationReceipt,
}

/// One document's lane rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentReceipt {
    /// Document id.
    pub id: String,
    /// Pages the donor observed.
    pub observed_pages: u64,
    /// Lane rows, one per declared in-slice lane.
    pub lanes: Vec<LaneReceipt>,
}

/// Corpus page/token denominators and per-lane rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusReceipt {
    /// Repository-relative corpus carrier.
    pub manifest: String,
    /// Pointer to the pinned documents.
    pub pointer: String,
    /// Corpus id.
    pub corpus_id: String,
    /// Declared document count.
    pub document_count: usize,
    /// Pinned document identities.
    pub identities: Vec<DocumentIdentity>,
    /// Declared in-slice lanes.
    pub lane_ids: Vec<String>,
    /// Pages per lane (the pinned page denominator).
    pub pages_per_lane: u64,
    /// pages × lanes.
    pub lane_page_denominator: u64,
    /// Lane pages the run actually observed.
    pub observed_lane_pages: u64,
    /// P1 admitted lexical tokens, per lane.
    pub layout_tokens_by_lane: BTreeMap<String, u64>,
    /// P2 assigned tokens, per lane (observed lanes only).
    pub assigned_tokens_by_lane: BTreeMap<String, u64>,
    /// Per-document rows.
    pub documents: Vec<DocumentReceipt>,
}

/// Redaction contract; every flag is the invariant the artifact must satisfy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Redaction {
    /// No raw text present.
    pub raw_text: bool,
    /// No PDF bytes present.
    pub raw_pdf: bool,
    /// No TEI present.
    pub tei_xml: bool,
    /// No images present.
    pub images: bool,
    /// No absolute/physical paths present.
    pub physical_paths: bool,
    /// No hostname present.
    pub hostname: bool,
    /// No executable present.
    pub executable: bool,
}

impl Redaction {
    /// The only redaction state a receipt may carry: nothing leaked.
    pub const fn clean() -> Self {
        Self {
            raw_text: false,
            raw_pdf: false,
            tei_xml: false,
            images: false,
            physical_paths: false,
            hostname: false,
            executable: false,
        }
    }
}

/// The emitted receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaFreeSliceReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// Slice identity.
    pub slice_id: String,
    /// Binding ADR.
    pub adr_ref: String,
    /// Requirements served.
    pub requirement_ids: Vec<String>,
    /// Claim scope; only `diagnostic_only` is emitted.
    pub claim_scope: String,
    /// Product-coverage effect; only `none` is emitted.
    pub product_coverage_effect: String,
    /// Binding to the declaration carrier.
    pub declaration: CarrierBinding,
    /// Binding to the cascade stage-id carrier.
    pub stage_id_source: CarrierBinding,
    /// Guard provenance.
    pub guard: GuardProvenanceReceipt,
    /// Corpus denominators.
    pub corpus: CorpusReceipt,
    /// Stage matrix, one row per cascade stage.
    pub stages: Vec<StageReceipt>,
    /// Redaction invariant.
    pub redaction: Redaction,
}

impl JavaFreeSliceReceipt {
    /// Serialize the receipt and refuse if any payload marker leaked.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, JavaFreeSliceError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            JavaFreeSliceError::ArtifactSerialization {
                reason: error.to_string(),
            }
        })?;
        assert_redacted(std::str::from_utf8(&bytes).unwrap_or_default())?;
        Ok(bytes)
    }
}

/// Refuse an artifact that leaked a payload, endpoint or host path marker.
pub fn assert_redacted(json: &str) -> Result<(), JavaFreeSliceError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(JavaFreeSliceError::RedactionViolation {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

/// Build the receipt from a validated declaration, guard provenance and the
/// observed documents. Refuses anything the declaration does not justify.
pub fn build_receipt(
    declaration: &SliceDeclaration,
    guard: GuardProvenance,
    declaration_revision_sha256: &str,
    stage_id_source_revision_sha256: &str,
    observed: &[ObservedDocument],
) -> Result<JavaFreeSliceReceipt, JavaFreeSliceError> {
    if !guard.installed {
        return Err(JavaFreeSliceError::GuardNotInstalled);
    }
    if !guard.identity_isolated {
        return Err(JavaFreeSliceError::GuardIdentityNotIsolated);
    }
    if guard.observed_java_calls > 0 {
        return Err(JavaFreeSliceError::ObservedJavaOnSlice {
            attempts: guard.observed_java_calls,
        });
    }

    // Every declared document must be observed exactly once, with the pinned
    // sha256 and page count. A document outside the pin is a refusal.
    let mut by_id: BTreeMap<&str, &ObservedDocument> = BTreeMap::new();
    for document in observed {
        let declared = declaration.document(&document.id).ok_or_else(|| {
            JavaFreeSliceError::UnknownDocument {
                document: document.id.clone(),
            }
        })?;
        if document.sha256 != declared.sha256 {
            return Err(JavaFreeSliceError::CorpusBindingMismatch {
                document: document.id.clone(),
                reason: format!(
                    "observed sha256 is `{}` but `{}` is pinned",
                    document.sha256, declared.sha256
                ),
            });
        }
        if document.observed_pages != declared.pages {
            return Err(JavaFreeSliceError::PageDenominatorDrift {
                document: document.id.clone(),
                expected: declared.pages,
                found: document.observed_pages,
            });
        }
        if by_id.insert(document.id.as_str(), document).is_some() {
            return Err(JavaFreeSliceError::CorpusBindingMismatch {
                document: document.id.clone(),
                reason: "observed twice".to_owned(),
            });
        }
    }
    for declared in &declaration.documents {
        if !by_id.contains_key(declared.id.as_str()) {
            return Err(JavaFreeSliceError::MissingDocument {
                document: declared.id.clone(),
            });
        }
    }

    let mut lane_ids = declaration.lane_ids.clone();
    lane_ids.sort();
    let mut documents = Vec::with_capacity(declaration.documents.len());
    let mut layout_tokens_by_lane: BTreeMap<String, u64> = declaration
        .lane_ids
        .iter()
        .map(|lane| (lane.clone(), 0))
        .collect();
    let mut assigned_tokens_by_lane: BTreeMap<String, u64> = BTreeMap::new();
    let mut observed_lane_pages: u64 = 0;

    for declared in &declaration.documents {
        let observation = by_id[declared.id.as_str()];
        let mut observed_lanes = observation.lanes.clone();
        observed_lanes.sort_by(|left, right| left.lane_id.cmp(&right.lane_id));
        let observed_ids = observed_lanes
            .iter()
            .map(|lane| lane.lane_id.clone())
            .collect::<Vec<_>>();
        if observed_ids != lane_ids {
            return Err(JavaFreeSliceError::LaneSetMismatch {
                document: declared.id.clone(),
            });
        }
        let mut lanes = Vec::with_capacity(observed_lanes.len());
        for lane in observed_lanes {
            if lane.lattice_fingerprint.trim().is_empty() {
                return Err(JavaFreeSliceError::CorpusBindingMismatch {
                    document: declared.id.clone(),
                    reason: format!("lane `{}` has no lattice digest", lane.lane_id),
                });
            }
            *layout_tokens_by_lane
                .entry(lane.lane_id.clone())
                .or_insert(0) += lane.admitted_lexical_tokens;
            if let SegmentationReceipt::Observed {
                assigned_tokens, ..
            } = lane.segmentation
            {
                *assigned_tokens_by_lane
                    .entry(lane.lane_id.clone())
                    .or_insert(0) += assigned_tokens;
            }
            observed_lane_pages += lane.observed_pages;
            lanes.push(LaneReceipt {
                lane_id: lane.lane_id,
                observed_pages: lane.observed_pages,
                admitted_lexical_tokens: lane.admitted_lexical_tokens,
                lattice_fingerprint: lane.lattice_fingerprint,
                segmentation: lane.segmentation,
            });
        }
        documents.push(DocumentReceipt {
            id: declared.id.clone(),
            observed_pages: observation.observed_pages,
            lanes,
        });
    }

    let stages = declaration
        .stages
        .iter()
        .map(|stage| StageReceipt {
            stage_id: stage.stage_id.clone(),
            stage_name: stage.stage_name.clone(),
            scope: stage.scope,
            java_required: stage.java_required,
            count: match stage.scope {
                StageScope::InSlice => JavaStageCount::Observed {
                    attempts: guard.observed_java_calls,
                },
                StageScope::IdentityAdmission => JavaStageCount::NotObserved {
                    reason: NOT_OBSERVED_BOUNDARY_ADMISSION_NOT_COUNTED.to_owned(),
                },
                StageScope::OutOfSlice => JavaStageCount::OutOfSlice {
                    reason: stage
                        .out_of_slice_reason
                        .clone()
                        .unwrap_or_else(|| NOT_OBSERVED_GUARD_NOT_INSTALLED.to_owned()),
                },
            },
        })
        .collect::<Vec<_>>();

    let identities = declaration
        .documents
        .iter()
        .map(|document| DocumentIdentity {
            id: document.id.clone(),
            source_pdf: document.source_pdf.clone(),
            sha256: document.sha256.clone(),
            pages: document.pages,
        })
        .collect::<Vec<_>>();

    let receipt = JavaFreeSliceReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION.to_owned(),
        slice_id: declaration.slice_id.clone(),
        adr_ref: declaration.adr_ref.clone(),
        requirement_ids: declaration.requirement_ids.clone(),
        claim_scope: CLAIM_SCOPE_DIAGNOSTIC_ONLY.to_owned(),
        product_coverage_effect: PRODUCT_COVERAGE_EFFECT_NONE.to_owned(),
        declaration: CarrierBinding {
            manifest: declaration.declaration_manifest.clone(),
            pointer: declaration.declaration_pointer.clone(),
            revision_sha256: declaration_revision_sha256.to_owned(),
        },
        stage_id_source: CarrierBinding {
            manifest: declaration.stage_id_source_manifest.clone(),
            pointer: declaration.stage_id_source_pointer.clone(),
            revision_sha256: stage_id_source_revision_sha256.to_owned(),
        },
        guard: GuardProvenanceReceipt {
            observer: OBSERVER_KIND.to_owned(),
            installed: guard.installed,
            identity_isolated: guard.identity_isolated,
            observed_java_calls: guard.observed_java_calls,
        },
        corpus: CorpusReceipt {
            manifest: declaration.corpus_manifest.clone(),
            pointer: declaration.corpus_pointer.clone(),
            corpus_id: declaration.corpus_id.clone(),
            document_count: declaration.documents.len(),
            identities,
            lane_ids: declaration.lane_ids.clone(),
            pages_per_lane: declaration.page_denominator,
            lane_page_denominator: declaration.lane_page_denominator(),
            observed_lane_pages,
            layout_tokens_by_lane,
            assigned_tokens_by_lane,
            documents,
        },
        stages,
        redaction: Redaction::clean(),
    };
    validate_receipt(&receipt, declaration)?;
    receipt.artifact_json_bytes()?;
    Ok(receipt)
}

/// Re-validate a receipt against the declaration it claims. Refuses any drift:
/// an unknown or missing stage, a changed scope, an out-of-slice stage with a
/// numeric count, a measurement without guard provenance, or a document outside
/// the pin. T05 drives this as its mutation surface.
pub fn validate_receipt(
    receipt: &JavaFreeSliceReceipt,
    declaration: &SliceDeclaration,
) -> Result<(), JavaFreeSliceError> {
    if receipt.schema_version != RECEIPT_SCHEMA_VERSION {
        return Err(JavaFreeSliceError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: RECEIPT_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.slice_id != declaration.slice_id {
        return Err(JavaFreeSliceError::SchemaDrift {
            field: "slice_id".to_owned(),
            expected: declaration.slice_id.clone(),
            found: receipt.slice_id.clone(),
        });
    }
    if receipt.claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(JavaFreeSliceError::ClaimScopeDrift {
            found: receipt.claim_scope.clone(),
        });
    }
    if receipt.product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(JavaFreeSliceError::ProductCoverageClaimed {
            found: receipt.product_coverage_effect.clone(),
        });
    }

    // The stage matrix must be the declared matrix: same set, same scope, same
    // java_required, and a typed disposition per stage.
    let declared_ids = declaration
        .stages
        .iter()
        .map(|stage| stage.stage_id.clone())
        .collect::<Vec<_>>();
    let receipt_ids = receipt
        .stages
        .iter()
        .map(|stage| stage.stage_id.clone())
        .collect::<Vec<_>>();
    if receipt_ids != declared_ids {
        let unknown = receipt_ids
            .iter()
            .find(|id| !declared_ids.contains(id))
            .cloned();
        return Err(match unknown {
            Some(stage) => JavaFreeSliceError::UnknownStage { stage },
            None => JavaFreeSliceError::UnclassifiedStage {
                stage: declared_ids
                    .iter()
                    .find(|id| !receipt_ids.contains(id))
                    .cloned()
                    .unwrap_or_else(|| "<none>".to_owned()),
            },
        });
    }

    let observed_present = receipt
        .stages
        .iter()
        .any(|stage| matches!(stage.count, JavaStageCount::Observed { .. }));
    if observed_present && (!receipt.guard.installed || !receipt.guard.identity_isolated) {
        return Err(JavaFreeSliceError::ObservedWithoutGuard {
            stage: receipt
                .stages
                .iter()
                .find(|stage| matches!(stage.count, JavaStageCount::Observed { .. }))
                .map(|stage| stage.stage_id.clone())
                .unwrap_or_else(|| "<none>".to_owned()),
        });
    }

    for stage in &receipt.stages {
        let declared =
            declaration
                .stage(&stage.stage_id)
                .ok_or_else(|| JavaFreeSliceError::UnknownStage {
                    stage: stage.stage_id.clone(),
                })?;
        if stage.scope != declared.scope {
            return Err(JavaFreeSliceError::StageMatrixMismatch {
                stage: stage.stage_id.clone(),
                reason: format!(
                    "scope is `{}` but `{}` is declared",
                    stage.scope.as_str(),
                    declared.scope.as_str()
                ),
            });
        }
        if stage.java_required != declared.java_required {
            return Err(JavaFreeSliceError::StageMatrixMismatch {
                stage: stage.stage_id.clone(),
                reason: "java_required drifted".to_owned(),
            });
        }
        if stage.stage_name != declared.stage_name {
            return Err(JavaFreeSliceError::StageMatrixMismatch {
                stage: stage.stage_id.clone(),
                reason: "stage_name drifted from the cascade DATA pointer".to_owned(),
            });
        }
        match (declared.scope, &stage.count) {
            (StageScope::InSlice, JavaStageCount::Observed { attempts }) => {
                if *attempts > 0 {
                    return Err(JavaFreeSliceError::ObservedJavaOnSlice {
                        attempts: *attempts,
                    });
                }
                if !receipt.guard.installed || !receipt.guard.identity_isolated {
                    return Err(JavaFreeSliceError::ObservedWithoutGuard {
                        stage: stage.stage_id.clone(),
                    });
                }
            }
            (StageScope::InSlice, JavaStageCount::NotObserved { reason })
                if reason.as_str() == NOT_OBSERVED_GUARD_NOT_INSTALLED => {}
            (StageScope::InSlice, _) => {
                return Err(JavaFreeSliceError::StageMatrixMismatch {
                    stage: stage.stage_id.clone(),
                    reason: "an in-slice stage must be Observed under a guard or NotObserved \
                             with a typed reason"
                        .to_owned(),
                });
            }
            (StageScope::OutOfSlice, JavaStageCount::OutOfSlice { reason }) => {
                if !declaration.out_of_slice_reasons.contains(reason) {
                    return Err(JavaFreeSliceError::VocabularyDrift {
                        reason: reason.clone(),
                    });
                }
            }
            (StageScope::OutOfSlice, JavaStageCount::Observed { .. })
            | (StageScope::OutOfSlice, JavaStageCount::NotObserved { .. }) => {
                return Err(JavaFreeSliceError::OutOfSliceStageCounted {
                    stage: stage.stage_id.clone(),
                });
            }
            (StageScope::IdentityAdmission, JavaStageCount::NotObserved { reason })
                if reason.as_str() == NOT_OBSERVED_BOUNDARY_ADMISSION_NOT_COUNTED => {}
            (StageScope::IdentityAdmission, _) => {
                return Err(JavaFreeSliceError::StageMatrixMismatch {
                    stage: stage.stage_id.clone(),
                    reason: format!(
                        "the admission boundary must be NotObserved({NOT_OBSERVED_BOUNDARY_ADMISSION_NOT_COUNTED})"
                    ),
                });
            }
        }
    }

    // Corpus binding: same identities, same digests, repository-relative paths.
    if receipt.corpus.document_count != declaration.documents.len() {
        return Err(JavaFreeSliceError::CorpusBindingMismatch {
            document: "corpus".to_owned(),
            reason: "document_count drifted".to_owned(),
        });
    }
    let identity_ids = receipt
        .corpus
        .identities
        .iter()
        .map(|identity| identity.id.clone())
        .collect::<Vec<_>>();
    let declared_identity_ids = declaration
        .documents
        .iter()
        .map(|document| document.id.clone())
        .collect::<Vec<_>>();
    if identity_ids != declared_identity_ids {
        let unknown = identity_ids
            .iter()
            .find(|id| !declared_identity_ids.contains(id))
            .cloned();
        return Err(match unknown {
            Some(document) => JavaFreeSliceError::UnknownDocument { document },
            None => JavaFreeSliceError::MissingDocument {
                document: declared_identity_ids
                    .iter()
                    .find(|id| !identity_ids.contains(id))
                    .cloned()
                    .unwrap_or_else(|| "<none>".to_owned()),
            },
        });
    }
    for identity in &receipt.corpus.identities {
        let declared = declaration.document(&identity.id).ok_or_else(|| {
            JavaFreeSliceError::UnknownDocument {
                document: identity.id.clone(),
            }
        })?;
        if identity.sha256 != declared.sha256 {
            return Err(JavaFreeSliceError::CorpusBindingMismatch {
                document: identity.id.clone(),
                reason: format!(
                    "receipt sha256 is `{}` but `{}` is pinned",
                    identity.sha256, declared.sha256
                ),
            });
        }
        if identity.source_pdf.starts_with('/') {
            return Err(JavaFreeSliceError::CorpusBindingMismatch {
                document: identity.id.clone(),
                reason: "source_pdf must be repository-relative".to_owned(),
            });
        }
    }

    if receipt.guard.observed_java_calls > 0 {
        return Err(JavaFreeSliceError::ObservedJavaOnSlice {
            attempts: receipt.guard.observed_java_calls,
        });
    }

    let redaction = &receipt.redaction;
    if redaction.raw_text
        || redaction.raw_pdf
        || redaction.tei_xml
        || redaction.images
        || redaction.physical_paths
        || redaction.hostname
        || redaction.executable
    {
        return Err(JavaFreeSliceError::RedactionViolation {
            marker: "redaction flag set".to_owned(),
        });
    }

    Ok(())
}

// ───────────────────────── helpers ─────────────────────────

fn parse_yaml(text: &str, path: &str) -> Result<Value, JavaFreeSliceError> {
    serde_yaml::from_str::<Value>(text).map_err(|error| JavaFreeSliceError::DataUnreadable {
        path: path.to_owned(),
        reason: error.to_string(),
    })
}

/// Derive the corpus-block pointer from a declared document-list pointer.
///
/// A slice declaration names a `...<corpus>.documents` pointer; the block that
/// contains the list is resolved by stripping the trailing `.documents` segment.
/// Both the block name and the list segment are DATA, never constants. `None`
/// refuses an absent, empty, non-`.documents`, or manifest-escaping pointer:
/// every segment must be a plain key token (no `/`, `\`, `#` or empty `..`), so
/// the walk stays inside the single declaration manifest.
fn corpus_block_pointer(pointer: &str) -> Option<String> {
    let trimmed = pointer.trim();
    if trimmed.is_empty() {
        return None;
    }
    let segments: Vec<&str> = trimmed.split('.').collect();
    if segments.len() < 2 || segments.iter().any(|segment| segment.is_empty()) {
        return None;
    }
    if segments
        .iter()
        .any(|segment| segment.contains('/') || segment.contains('\\') || segment.contains('#'))
    {
        return None;
    }
    if segments.last().copied() != Some("documents") {
        return None;
    }
    Some(segments[..segments.len() - 1].join("."))
}

/// Walk a dot-separated pointer. Numeric segments index sequences.
fn pointer<'a>(
    root: &'a Value,
    path: &str,
    carrier: &str,
) -> Result<&'a Value, JavaFreeSliceError> {
    let mut current = root;
    let mut walked = String::new();
    for segment in path.split('.') {
        if !walked.is_empty() {
            walked.push('.');
        }
        walked.push_str(segment);
        current = match current {
            Value::Mapping(map) => map
                .iter()
                .find_map(|(key, value)| match key {
                    Value::String(name) if name == segment => Some(value),
                    _ => None,
                })
                .ok_or_else(|| JavaFreeSliceError::DataFieldMissing {
                    path: format!("{carrier}#{walked}"),
                })?,
            Value::Sequence(sequence) => {
                let index =
                    segment
                        .parse::<usize>()
                        .map_err(|_| JavaFreeSliceError::DataFieldType {
                            path: format!("{carrier}#{walked}"),
                            expected: "a sequence index".to_owned(),
                        })?;
                sequence
                    .get(index)
                    .ok_or_else(|| JavaFreeSliceError::DataFieldMissing {
                        path: format!("{carrier}#{walked}"),
                    })?
            }
            _ => {
                return Err(JavaFreeSliceError::DataFieldType {
                    path: format!("{carrier}#{walked}"),
                    expected: "a mapping or sequence".to_owned(),
                });
            }
        };
    }
    Ok(current)
}

fn member<'a>(value: &'a Value, key: &str, path: &str) -> Result<&'a Value, JavaFreeSliceError> {
    let map = as_mapping(value, path)?;
    map.iter()
        .find_map(|(candidate, value)| match candidate {
            Value::String(name) if name == key => Some(value),
            _ => None,
        })
        .ok_or_else(|| JavaFreeSliceError::DataFieldMissing {
            path: format!("{path}.{key}"),
        })
}

fn as_mapping<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a serde_yaml::Mapping, JavaFreeSliceError> {
    match value {
        Value::Mapping(map) => Ok(map),
        _ => Err(JavaFreeSliceError::DataFieldType {
            path: path.to_owned(),
            expected: "a mapping".to_owned(),
        }),
    }
}

fn as_sequence<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>, JavaFreeSliceError> {
    match value {
        Value::Sequence(sequence) => Ok(sequence),
        _ => Err(JavaFreeSliceError::DataFieldType {
            path: path.to_owned(),
            expected: "a sequence".to_owned(),
        }),
    }
}

fn required_str(value: &Value, key: &str, path: &str) -> Result<String, JavaFreeSliceError> {
    let raw = member(value, key, path)?;
    match raw {
        Value::String(text) => Ok(text.clone()),
        Value::Number(number) => Ok(number.to_string()),
        _ => Err(JavaFreeSliceError::DataFieldType {
            path: format!("{path}.{key}"),
            expected: "a string".to_owned(),
        }),
    }
}

fn required_bool(value: &Value, key: &str, path: &str) -> Result<bool, JavaFreeSliceError> {
    match member(value, key, path)? {
        Value::Bool(flag) => Ok(*flag),
        _ => Err(JavaFreeSliceError::DataFieldType {
            path: format!("{path}.{key}"),
            expected: "a boolean".to_owned(),
        }),
    }
}

fn required_u64(value: &Value, key: &str, path: &str) -> Result<u64, JavaFreeSliceError> {
    match member(value, key, path)? {
        Value::Number(number) => number
            .as_u64()
            .ok_or_else(|| JavaFreeSliceError::DataFieldType {
                path: format!("{path}.{key}"),
                expected: "a non-negative integer".to_owned(),
            }),
        _ => Err(JavaFreeSliceError::DataFieldType {
            path: format!("{path}.{key}"),
            expected: "an integer".to_owned(),
        }),
    }
}

fn as_str_vec(value: &Value, path: &str) -> Result<Vec<String>, JavaFreeSliceError> {
    as_sequence(value, path)?
        .iter()
        .map(|item| match item {
            Value::String(text) => Ok(text.clone()),
            _ => Err(JavaFreeSliceError::DataFieldType {
                path: path.to_owned(),
                expected: "a sequence of strings".to_owned(),
            }),
        })
        .collect()
}

fn mapping_keys(map: &serde_yaml::Mapping) -> Vec<String> {
    map.iter()
        .filter_map(|(key, _)| match key {
            Value::String(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn lock_token(value: &Value) -> String {
    match value {
        Value::Bool(flag) => flag.to_string(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        other => format!("{other:?}"),
    }
}

/// Bare sha256 hex of the given bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn declaration() -> SliceDeclaration {
        load_from_root(&repo_root()).expect("the declared slice loads")
    }

    fn layout_yaml() -> String {
        std::fs::read_to_string(repo_root().join(SLICE_DECLARATION_DATA_REL))
            .expect("layout carrier readable")
    }

    /// Mutate one occurrence inside the `java_free_vertical_slice` block only,
    /// so a same-named key elsewhere in the carrier cannot absorb the edit.
    fn mutate_slice_block(from: &str, to: &str) -> String {
        let yaml = layout_yaml();
        let marker = format!("{SLICE_DECLARATION_POINTER}:");
        let index = yaml.find(&marker).expect("the declaration block exists");
        let (head, tail) = yaml.split_at(index);
        format!("{head}{}", tail.replacen(from, to, 1))
    }

    fn compatibility_yaml() -> String {
        std::fs::read_to_string(repo_root().join(COMPATIBILITY_DATA_REL))
            .expect("compatibility carrier readable")
    }

    fn synthetic_observed(declaration: &SliceDeclaration) -> Vec<ObservedDocument> {
        declaration
            .documents
            .iter()
            .map(|document| ObservedDocument {
                id: document.id.clone(),
                sha256: document.sha256.clone(),
                observed_pages: document.pages,
                lanes: declaration
                    .lane_ids
                    .iter()
                    .map(|lane| ObservedLane {
                        lane_id: lane.clone(),
                        observed_pages: document.pages,
                        admitted_lexical_tokens: 10,
                        lattice_fingerprint: "sha256:lattice".to_owned(),
                        segmentation: SegmentationReceipt::Observed {
                            assigned_tokens: 10,
                            order_digest: "sha256:order".to_owned(),
                        },
                    })
                    .collect(),
            })
            .collect()
    }

    fn good_guard() -> GuardProvenance {
        GuardProvenance {
            installed: true,
            identity_isolated: true,
            observed_java_calls: 0,
        }
    }

    #[test]
    fn loads_the_declared_slice_boundary() {
        let declaration = declaration();
        assert_eq!(declaration.slice_id, "pdf-layout-p1-p2");
        assert_eq!(declaration.adr_ref, "ADR-077");
        assert_eq!(declaration.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
        assert_eq!(declaration.in_slice_stage_ids(), vec!["P1", "P2"]);
        assert_eq!(
            declaration.out_of_slice_stage_ids(),
            vec!["P3", "P4", "P5", "P6", "P7", "P8", "P9"]
        );
        assert_eq!(declaration.documents.len(), 12);
        assert_eq!(declaration.page_denominator, 381);
        assert_eq!(declaration.lane_ids.len(), 2);
        assert_eq!(declaration.lane_page_denominator(), 762);
        assert_eq!(declaration.stages.len(), 10);
        assert!(declaration.stage("P0").is_some());
        assert_eq!(
            declaration
                .stage("P1")
                .map(|stage| stage.stage_name.as_str()),
            Some("pdfalto_layout_lattice")
        );
    }

    #[test]
    fn refuses_a_widened_claim_scope() {
        let yaml = mutate_slice_block("claim_scope: diagnostic_only", "claim_scope: product_ready");
        let error = load_declaration(&yaml, &compatibility_yaml()).unwrap_err();
        assert_eq!(
            error,
            JavaFreeSliceError::ClaimScopeDrift {
                found: "product_ready".to_owned()
            }
        );
    }

    #[test]
    fn refuses_a_product_coverage_claim() {
        let yaml = mutate_slice_block(
            "product_coverage_effect: none",
            "product_coverage_effect: full",
        );
        let error = load_declaration(&yaml, &compatibility_yaml()).unwrap_err();
        assert_eq!(
            error,
            JavaFreeSliceError::ProductCoverageClaimed {
                found: "full".to_owned()
            }
        );
    }

    #[test]
    fn refuses_a_relaxed_lock() {
        let yaml = mutate_slice_block("import_eligible: false", "import_eligible: true");
        let error = load_declaration(&yaml, &compatibility_yaml()).unwrap_err();
        assert_eq!(
            error,
            JavaFreeSliceError::LockDrift {
                lock: "import_eligible".to_owned(),
                expected: "false".to_owned(),
                found: "true".to_owned()
            }
        );
    }

    #[test]
    fn refuses_an_unknown_stage_named_by_the_declaration() {
        let yaml = mutate_slice_block("stage_id: P3", "stage_id: P30");
        let error = load_declaration(&yaml, &compatibility_yaml()).unwrap_err();
        assert_eq!(
            error,
            JavaFreeSliceError::UnknownStage {
                stage: "P30".to_owned()
            }
        );
    }

    #[test]
    fn refuses_an_out_of_slice_reason_outside_the_vocabulary() {
        let yaml = mutate_slice_block("reason: java_authority", "reason: made_up");
        let error = load_declaration(&yaml, &compatibility_yaml()).unwrap_err();
        assert_eq!(
            error,
            JavaFreeSliceError::VocabularyDrift {
                reason: "made_up".to_owned()
            }
        );
    }

    #[test]
    fn refuses_java_required_inside_the_slice() {
        let yaml = mutate_slice_block("java_required: false", "java_required: true");
        let error = load_declaration(&yaml, &compatibility_yaml()).unwrap_err();
        assert!(matches!(
            error,
            JavaFreeSliceError::InSliceJavaRequired { .. }
        ));
    }

    #[test]
    fn builds_and_validates_a_receipt_from_the_declared_corpus() {
        let declaration = declaration();
        let receipt = build_receipt(
            &declaration,
            good_guard(),
            "sha256:declaration",
            "sha256:compatibility",
            &synthetic_observed(&declaration),
        )
        .expect("a justified receipt builds");
        validate_receipt(&receipt, &declaration).expect("and re-validates");
        assert_eq!(receipt.stages.len(), 10);
        assert_eq!(receipt.corpus.identities.len(), 12);
        assert_eq!(receipt.corpus.lane_page_denominator, 762);
        assert_eq!(receipt.guard.observed_java_calls, 0);
        // A zero is reachable only as Observed under an installed guard.
        let p1 = receipt
            .stages
            .iter()
            .find(|stage| stage.stage_id == "P1")
            .expect("P1 present");
        assert_eq!(p1.count, JavaStageCount::Observed { attempts: 0 });
    }

    #[test]
    fn refuses_an_observed_zero_without_guard_provenance() {
        let declaration = declaration();
        let mut receipt = build_receipt(
            &declaration,
            good_guard(),
            "sha256:declaration",
            "sha256:compatibility",
            &synthetic_observed(&declaration),
        )
        .expect("builds");
        receipt.guard.installed = false;
        let error = validate_receipt(&receipt, &declaration).unwrap_err();
        assert!(matches!(
            error,
            JavaFreeSliceError::ObservedWithoutGuard { .. }
        ));
    }

    #[test]
    fn refuses_a_guard_that_rotates_the_cache_identity() {
        let declaration = declaration();
        let error = build_receipt(
            &declaration,
            GuardProvenance {
                installed: true,
                identity_isolated: false,
                observed_java_calls: 0,
            },
            "sha256:declaration",
            "sha256:compatibility",
            &synthetic_observed(&declaration),
        )
        .unwrap_err();
        assert_eq!(error, JavaFreeSliceError::GuardIdentityNotIsolated);
    }

    #[test]
    fn refuses_an_out_of_slice_stage_with_a_numeric_count() {
        let declaration = declaration();
        let mut receipt = build_receipt(
            &declaration,
            good_guard(),
            "sha256:declaration",
            "sha256:compatibility",
            &synthetic_observed(&declaration),
        )
        .expect("builds");
        let p3 = receipt
            .stages
            .iter_mut()
            .find(|stage| stage.stage_id == "P3")
            .expect("P3 present");
        p3.count = JavaStageCount::Observed { attempts: 0 };
        let error = validate_receipt(&receipt, &declaration).unwrap_err();
        assert_eq!(
            error,
            JavaFreeSliceError::OutOfSliceStageCounted {
                stage: "P3".to_owned()
            }
        );
    }

    #[test]
    fn refuses_a_receipt_stage_outside_the_declared_matrix() {
        let declaration = declaration();
        let mut receipt = build_receipt(
            &declaration,
            good_guard(),
            "sha256:declaration",
            "sha256:compatibility",
            &synthetic_observed(&declaration),
        )
        .expect("builds");
        receipt.stages.push(StageReceipt {
            stage_id: "P99".to_owned(),
            stage_name: "ghost".to_owned(),
            scope: StageScope::OutOfSlice,
            java_required: true,
            count: JavaStageCount::OutOfSlice {
                reason: "java_authority".to_owned(),
            },
        });
        let error = validate_receipt(&receipt, &declaration).unwrap_err();
        assert_eq!(
            error,
            JavaFreeSliceError::UnknownStage {
                stage: "P99".to_owned()
            }
        );
    }

    #[test]
    fn refuses_an_observed_java_call_on_the_slice() {
        let declaration = declaration();
        let error = build_receipt(
            &declaration,
            GuardProvenance {
                installed: true,
                identity_isolated: true,
                observed_java_calls: 1,
            },
            "sha256:declaration",
            "sha256:compatibility",
            &synthetic_observed(&declaration),
        )
        .unwrap_err();
        assert_eq!(
            error,
            JavaFreeSliceError::ObservedJavaOnSlice { attempts: 1 }
        );
    }

    #[test]
    fn refuses_a_document_outside_the_pinned_identities() {
        let declaration = declaration();
        let mut observed = synthetic_observed(&declaration);
        observed[0].sha256 = "0".repeat(64);
        let error = build_receipt(
            &declaration,
            good_guard(),
            "sha256:declaration",
            "sha256:compatibility",
            &observed,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            JavaFreeSliceError::CorpusBindingMismatch { .. }
        ));
    }

    #[test]
    fn refuses_an_unpinned_observed_document() {
        let declaration = declaration();
        let mut observed = synthetic_observed(&declaration);
        observed.push(ObservedDocument {
            id: "not-pinned".to_owned(),
            sha256: "0".repeat(64),
            observed_pages: 1,
            lanes: Vec::new(),
        });
        let error = build_receipt(
            &declaration,
            good_guard(),
            "sha256:declaration",
            "sha256:compatibility",
            &observed,
        )
        .unwrap_err();
        assert_eq!(
            error,
            JavaFreeSliceError::UnknownDocument {
                document: "not-pinned".to_owned()
            }
        );
    }

    #[test]
    fn artifact_serialization_refuses_a_leak_marker() {
        let declaration = declaration();
        let receipt = build_receipt(
            &declaration,
            good_guard(),
            "sha256:declaration",
            "sha256:compatibility",
            &synthetic_observed(&declaration),
        )
        .expect("builds");
        let clean = receipt.artifact_json_bytes().expect("clean artifact");
        assert!(!clean.is_empty());
        assert!(assert_redacted("a http://leak is refused").is_err());
        assert!(assert_redacted("a /root/path is refused").is_err());
        assert!(assert_redacted("a clean token is accepted").is_ok());
    }

    #[test]
    fn receipt_roundtrips_through_json() {
        let declaration = declaration();
        let receipt = build_receipt(
            &declaration,
            good_guard(),
            "sha256:declaration",
            "sha256:compatibility",
            &synthetic_observed(&declaration),
        )
        .expect("builds");
        let bytes = receipt.artifact_json_bytes().expect("serializes");
        let back: JavaFreeSliceReceipt = serde_json::from_slice(&bytes).expect("deserializes");
        assert_eq!(back, receipt);
        validate_receipt(&back, &declaration).expect("still validates");
    }
}
