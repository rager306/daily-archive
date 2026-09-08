//! Offline, payload-free cross-stage semantic checks over already-produced receipts.
use crate::{
    AccountingBucket, BucketCount, NATIVE_BODY_SCHEMA_VERSION,
    NATIVE_VISUAL_SEQUENCE_SCHEMA_VERSION, NativeBodyAccounting, NativeVisualHeadOutcome,
    NativeVisualMetrics, NativeVisualSequenceReceipt, P5ShadowJoinReport,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CROSS_STAGE_SEMANTIC_SCHEMA_VERSION: &str = "da-parse.cross-stage-semantic-invariants/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaP4Observation {
    pub paper_id: String,
    pub grobid_profile: String,
    pub java_citation_n: usize,
    pub production_authority: String,
    pub observe_mode: String,
    pub empty_count_not_success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaP6Observation {
    pub paper_id: String,
    pub grobid_profile: String,
    pub java_figure_n: usize,
    pub java_table_n: usize,
    pub java_formula_n: usize,
    pub figure_typed_absence: bool,
    pub table_typed_absence: bool,
    pub formula_typed_absence: bool,
    pub production_authority: String,
    pub observe_mode: String,
    pub empty_count_not_success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaP7Observation {
    pub paper_id: String,
    pub grobid_profile: String,
    pub java_callout_n: usize,
    pub unresolved_n: usize,
    pub production_authority: String,
    pub observe_mode: String,
    pub native_surface: String,
    pub empty_count_not_success: bool,
}

#[derive(Debug, Clone)]
pub struct CrossStageSemanticInvariantsInput<'a> {
    pub join: &'a P5ShadowJoinReport,
    pub java_p4: JavaP4Observation,
    pub java_p6: JavaP6Observation,
    pub java_p7: JavaP7Observation,
    pub native_accounting: Option<&'a NativeBodyAccounting>,
    pub native_visual: Option<&'a NativeVisualSequenceReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeBucketReport {
    pub admitted: usize,
    pub conserved: usize,
    pub unsupported: usize,
    pub loss: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeVisualHeadReport {
    pub supplied: bool,
    pub typed_absence: bool,
    pub metrics: Option<NativeVisualMetrics>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossStageSemanticInvariantsReport {
    pub schema_version: String,
    pub identity_match: bool,
    pub identity: crate::P5ShadowJoinIdentity,
    pub java_p5: crate::JavaP5Observation,
    pub java_p2: crate::JavaP2Observation,
    pub java_p4: JavaP4Observation,
    pub java_p6: JavaP6Observation,
    pub java_p7: JavaP7Observation,
    pub native_sequence: crate::NativeBodySequenceCounts,
    pub native_status: String,
    pub native_buckets: BTreeMap<AccountingBucket, NativeBucketReport>,
    pub native_visual: Option<BTreeMap<String, NativeVisualHeadReport>>,
    pub body_markers_admitted: usize,
    pub empty_count_not_success: bool,
    pub redaction: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossStageSemanticInvariantsError {
    JoinNotMatched,
    MissingIdentity {
        field: &'static str,
    },
    MalformedIdentity {
        field: &'static str,
    },
    DocumentMismatch,
    SourceDigestMismatch,
    ProfileMismatch,
    ProviderMismatch,
    SchemaMismatch {
        field: &'static str,
    },
    BoundaryDigestMismatch,
    AuthorityDrift,
    ObserveModeDrift,
    NativeSurfaceLive,
    EmptyCountTreatedAsSuccess,
    MissingRequiredLeg {
        leg: &'static str,
    },
    NativeAccountingMismatch {
        field: &'static str,
    },
    BucketSumMismatch {
        bucket: AccountingBucket,
        field: &'static str,
    },
    CountEqualReorder,
    DuplicateUnit,
    VisualHeadMismatch {
        head: &'static str,
    },
}

fn non_empty(value: &str, field: &'static str) -> Result<(), CrossStageSemanticInvariantsError> {
    if value.trim().is_empty() {
        Err(CrossStageSemanticInvariantsError::MissingIdentity { field })
    } else {
        Ok(())
    }
}
fn digest(value: &str, field: &'static str) -> Result<(), CrossStageSemanticInvariantsError> {
    non_empty(value, field)?;
    let candidate = value.strip_prefix("sha256:").unwrap_or(value);
    if candidate.len() != 64 || !candidate.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(CrossStageSemanticInvariantsError::MalformedIdentity { field });
    }
    Ok(())
}
fn identity_check(
    join: &P5ShadowJoinReport,
    paper_id: &str,
    profile: &str,
) -> Result<(), CrossStageSemanticInvariantsError> {
    non_empty(paper_id, "java.paper_id")?;
    non_empty(profile, "java.grobid_profile")?;
    if paper_id != join.identity.document_id {
        return Err(CrossStageSemanticInvariantsError::DocumentMismatch);
    }
    if profile != join.identity.grobid_profile {
        return Err(CrossStageSemanticInvariantsError::ProfileMismatch);
    }
    Ok(())
}
fn validate_java_common(
    join: &P5ShadowJoinReport,
    paper_id: &str,
    profile: &str,
    authority: &str,
    mode: &str,
    empty: bool,
) -> Result<(), CrossStageSemanticInvariantsError> {
    identity_check(join, paper_id, profile)?;
    if authority != "java_http" {
        return Err(CrossStageSemanticInvariantsError::AuthorityDrift);
    }
    if mode != "shadow_receipt" && mode != "production_stamp" {
        return Err(CrossStageSemanticInvariantsError::ObserveModeDrift);
    }
    if !empty {
        return Err(CrossStageSemanticInvariantsError::EmptyCountTreatedAsSuccess);
    }
    Ok(())
}

fn bucket_report(
    accounting: &NativeBodyAccounting,
) -> BTreeMap<AccountingBucket, NativeBucketReport> {
    AccountingBucket::all()
        .into_iter()
        .map(|bucket| {
            let count = accounting.buckets.get(&bucket).cloned().unwrap_or_default();
            (
                bucket,
                NativeBucketReport {
                    admitted: count.admitted,
                    conserved: count.conserved,
                    unsupported: count.unsupported,
                    loss: count.loss,
                },
            )
        })
        .collect()
}

impl AccountingBucket {
    fn all() -> [Self; 7] {
        [
            Self::BodyBlocks,
            Self::BodyMarkers,
            Self::VisualContent,
            Self::Annex,
            Self::References,
            Self::PageFurniture,
            Self::Unsupported,
        ]
    }
}

fn metrics_for(
    step: &crate::NativeVisualSequenceStep,
    head: &'static str,
) -> Result<NativeVisualHeadReport, CrossStageSemanticInvariantsError> {
    let Some(outcome) = step.outcome.as_ref() else {
        return Err(CrossStageSemanticInvariantsError::VisualHeadMismatch { head });
    };
    let metrics = match outcome {
        NativeVisualHeadOutcome::Figure { metrics, .. }
        | NativeVisualHeadOutcome::Table { metrics, .. }
        | NativeVisualHeadOutcome::Formula { metrics, .. }
        | NativeVisualHeadOutcome::Terminal { metrics, .. } => metrics.clone(),
    };
    Ok(NativeVisualHeadReport {
        supplied: true,
        typed_absence: matches!(outcome, NativeVisualHeadOutcome::Terminal { .. }),
        metrics: Some(metrics),
    })
}

pub fn check_cross_stage_semantic_invariants(
    input: CrossStageSemanticInvariantsInput<'_>,
) -> Result<CrossStageSemanticInvariantsReport, CrossStageSemanticInvariantsError> {
    let join = input.join;
    if !join.identity_match {
        return Err(CrossStageSemanticInvariantsError::JoinNotMatched);
    }
    if join.schema_version != crate::P5_SHADOW_JOIN_SCHEMA_VERSION {
        return Err(CrossStageSemanticInvariantsError::SchemaMismatch {
            field: "join.schema_version",
        });
    }
    non_empty(&join.identity.document_id, "document_id")?;
    digest(&join.identity.source_sha256, "source_sha256")?;
    non_empty(&join.identity.grobid_profile, "grobid_profile")?;
    non_empty(&join.identity.provider_id, "provider_id")?;
    non_empty(
        &join.identity.segmentation_schema_version,
        "segmentation_schema_version",
    )?;
    non_empty(
        &join.identity.accounting_schema_version,
        "accounting_schema_version",
    )?;
    digest(&join.identity.boundary_digest, "boundary_digest")?;
    validate_java_common(
        join,
        &input.java_p4.paper_id,
        &input.java_p4.grobid_profile,
        &input.java_p4.production_authority,
        &input.java_p4.observe_mode,
        input.java_p4.empty_count_not_success,
    )?;
    validate_java_common(
        join,
        &input.java_p6.paper_id,
        &input.java_p6.grobid_profile,
        &input.java_p6.production_authority,
        &input.java_p6.observe_mode,
        input.java_p6.empty_count_not_success,
    )?;
    validate_java_common(
        join,
        &input.java_p7.paper_id,
        &input.java_p7.grobid_profile,
        &input.java_p7.production_authority,
        &input.java_p7.observe_mode,
        input.java_p7.empty_count_not_success,
    )?;
    if input.java_p7.native_surface != "none" {
        return Err(CrossStageSemanticInvariantsError::NativeSurfaceLive);
    }
    if !join.empty_count_not_success {
        return Err(CrossStageSemanticInvariantsError::EmptyCountTreatedAsSuccess);
    }

    let mut buckets = BTreeMap::new();
    if let Some(accounting) = input.native_accounting {
        if accounting.schema_version != NATIVE_BODY_SCHEMA_VERSION {
            return Err(CrossStageSemanticInvariantsError::SchemaMismatch {
                field: "native_accounting.schema_version",
            });
        }
        non_empty(&accounting.document_id, "native_accounting.document_id")?;
        digest(&accounting.source_sha256, "native_accounting.source_sha256")?;
        non_empty(&accounting.provider_id, "native_accounting.provider_id")?;
        digest(
            &accounting.source_boundary_digest,
            "native_accounting.source_boundary_digest",
        )?;
        if accounting.document_id != join.identity.document_id {
            return Err(CrossStageSemanticInvariantsError::DocumentMismatch);
        }
        if accounting.source_sha256 != join.identity.source_sha256 {
            return Err(CrossStageSemanticInvariantsError::SourceDigestMismatch);
        }
        if accounting.provider_id != join.identity.provider_id {
            return Err(CrossStageSemanticInvariantsError::ProviderMismatch);
        }
        if accounting.source_boundary_digest != join.identity.boundary_digest {
            return Err(CrossStageSemanticInvariantsError::BoundaryDigestMismatch);
        }
        for (actual, expected, field) in [
            (accounting.admitted, join.native_counts.admitted, "admitted"),
            (
                accounting.conserved,
                join.native_counts.conserved,
                "conserved",
            ),
            (
                accounting.unsupported,
                join.native_counts.unsupported,
                "unsupported",
            ),
            (accounting.loss, join.native_counts.loss, "loss"),
        ] {
            if actual != expected {
                return Err(CrossStageSemanticInvariantsError::NativeAccountingMismatch { field });
            }
        }
        let mut unit_counts = BTreeMap::<AccountingBucket, BucketCount>::new();
        let mut previous = None;
        let mut seen = BTreeSet::new();
        for unit in &accounting.units {
            let key = (unit.page_ordinal, unit.token_ordinal);
            if !seen.insert(key) {
                return Err(CrossStageSemanticInvariantsError::DuplicateUnit);
            }
            if previous.is_some_and(|old| key < old) {
                return Err(CrossStageSemanticInvariantsError::CountEqualReorder);
            }
            previous = Some(key);
            let count = unit_counts.entry(unit.bucket).or_default();
            count.admitted += 1;
            count.conserved += 1;
            if unit.bucket == AccountingBucket::Unsupported {
                count.unsupported += 1;
            }
        }
        let mut admitted_sum = 0usize;
        let mut conserved_sum = 0usize;
        let mut unsupported_sum = 0usize;
        let mut loss_sum = 0usize;
        for bucket in AccountingBucket::all() {
            let expected = accounting.buckets.get(&bucket).cloned().unwrap_or_default();
            let observed = unit_counts.get(&bucket).cloned().unwrap_or_default();
            if expected.admitted != observed.admitted
                || expected.conserved != observed.conserved
                || expected.unsupported != observed.unsupported
            {
                return Err(CrossStageSemanticInvariantsError::BucketSumMismatch {
                    bucket,
                    field: "units",
                });
            }
            admitted_sum += expected.admitted;
            conserved_sum += expected.conserved;
            unsupported_sum += expected.unsupported;
            loss_sum += expected.loss;
        }
        if admitted_sum != accounting.admitted {
            return Err(CrossStageSemanticInvariantsError::BucketSumMismatch {
                bucket: AccountingBucket::BodyBlocks,
                field: "admitted_total",
            });
        }
        if conserved_sum != accounting.conserved {
            return Err(CrossStageSemanticInvariantsError::BucketSumMismatch {
                bucket: AccountingBucket::BodyBlocks,
                field: "conserved_total",
            });
        }
        if unsupported_sum != accounting.unsupported {
            return Err(CrossStageSemanticInvariantsError::BucketSumMismatch {
                bucket: AccountingBucket::BodyBlocks,
                field: "unsupported_total",
            });
        }
        if loss_sum != accounting.loss {
            return Err(CrossStageSemanticInvariantsError::BucketSumMismatch {
                bucket: AccountingBucket::BodyBlocks,
                field: "loss_total",
            });
        }
        buckets = bucket_report(accounting);
    }

    let native_visual = if let Some(visual) = input.native_visual {
        if visual.schema_version != NATIVE_VISUAL_SEQUENCE_SCHEMA_VERSION {
            return Err(CrossStageSemanticInvariantsError::SchemaMismatch {
                field: "native_visual.schema_version",
            });
        }
        let id = &visual.identity;
        non_empty(&id.document_id, "native_visual.document_id")?;
        digest(&id.source_sha256, "native_visual.source_sha256")?;
        non_empty(&id.provider_id, "native_visual.provider_id")?;
        if id.document_id != join.identity.document_id {
            return Err(CrossStageSemanticInvariantsError::DocumentMismatch);
        }
        if id.source_sha256 != join.identity.source_sha256 {
            return Err(CrossStageSemanticInvariantsError::SourceDigestMismatch);
        }
        if id.provider_id != join.identity.provider_id {
            return Err(CrossStageSemanticInvariantsError::ProviderMismatch);
        }
        let mut result = BTreeMap::new();
        for (name, head) in [
            ("figure", "figure"),
            ("table", "table"),
            ("formula", "formula"),
        ] {
            let step = visual
                .steps
                .iter()
                .find(|step| step.name == name)
                .ok_or(CrossStageSemanticInvariantsError::VisualHeadMismatch { head })?;
            result.insert(name.to_owned(), metrics_for(step, head)?);
        }
        Some(result)
    } else {
        None
    };

    Ok(CrossStageSemanticInvariantsReport {
        schema_version: CROSS_STAGE_SEMANTIC_SCHEMA_VERSION.to_owned(),
        identity_match: true,
        identity: join.identity.clone(),
        java_p5: join.java_p5.clone(),
        java_p2: join.java_p2.clone(),
        java_p4: input.java_p4,
        java_p6: input.java_p6,
        java_p7: input.java_p7,
        native_sequence: join.native_counts.clone(),
        native_status: join.native_status.clone(),
        native_buckets: buckets,
        native_visual,
        body_markers_admitted: input
            .native_accounting
            .and_then(|a| a.buckets.get(&AccountingBucket::BodyMarkers))
            .map_or(0, |c| c.admitted),
        empty_count_not_success: true,
        redaction: [
            ("content", false),
            ("xml", false),
            ("binary", false),
            ("paths", false),
            ("opaque", false),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect(),
    })
}
