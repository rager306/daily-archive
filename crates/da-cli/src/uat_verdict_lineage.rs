//! Bounded, machine-checked verdict lineage for report-producing CLI tests.
//!
//! GAR-016 (`doc/review/grobid-absorption-review-2026-09-10.jsonl`, programme
//! M502-2w7y8k) records that the CLI shadow tests assert runner exit success and
//! inspect generated reports directly, so a later reader can assert PASS from
//! prose even when the command, inputs or artifacts changed. This module makes
//! the PASS itself a bounded, serializable object tied to exactly what produced
//! it.
//!
//! A verdict binds five things and nothing else:
//!
//! 1. the command identity (a stable identifier plus a digest over the argv),
//! 2. every input digest,
//! 3. the source revision,
//! 4. every produced artifact digest, and
//! 5. the parsed assertions that held.
//!
//! Two properties keep the PASS honest:
//!
//! * **Validity is recomputed, never stored.** [`VerdictLineage`] has no
//!   `status`/`pass` field. [`VerdictLineage::verify_binding`] recomputes the
//!   binding digest from the bound payload, and [`VerdictLineage::verify`]
//!   compares the bound payload against an observed one. A JSON document that
//!   smuggles a `"status": "PASS"` key is rejected outright
//!   (`deny_unknown_fields`), never read as a verdict.
//! * **The claim is explicit and scoped.** [`VerdictClaim`] separates an
//!   execution observation from a quality or parity claim. Only
//!   [`VerdictClaim::Execution`] can produce
//!   [`VerdictStatus::ExecutionPass`]; a `quality` or `parity` claim is refused
//!   ([`VerdictFailure::UnsupportedClaim`]) instead of being read as PASS.
//!
//! Host-path redaction is part of the contract: input/artifact names and the
//! source revision are validated to be host-free, and the argv is hashed rather
//! than stored, so a durable verdict cannot re-introduce the physical paths that
//! the shadow reports already redact.
//!
//! This is test/library provenance only. `import_eligible` stays false, no graph
//! is written, and no route or promotion is implied.

use da_domain::ContentDigest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Schema identity of the durable verdict document.
pub const UAT_VERDICT_LINEAGE_SCHEMA_VERSION: &str = "uat-verdict-lineage.v1";

/// Substrings that mark a value as host-specific. A bounded verdict must not
/// carry them: the shadow reports already enforce the same redaction.
const HOST_PATH_MARKERS: [&str; 6] = ["/root/", "/home/", "/tmp/", "file://", "C:\\", "\\\\"];

/// The claim a verdict speaks to. `Execution` states only that the bound
/// command ran against the bound inputs and its parsed assertions held.
/// `Quality` and `Parity` are declarable but never substantiated by an
/// execution verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictClaim {
    /// The command executed and the bound assertions held.
    Execution,
    /// A quality claim, which an execution verdict cannot establish.
    Quality,
    /// A parity claim, which an execution verdict cannot establish.
    Parity,
}

impl VerdictClaim {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Execution => "execution",
            Self::Quality => "quality",
            Self::Parity => "parity",
        }
    }
}

impl fmt::Display for VerdictClaim {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Validation failures for a bounded verdict. Kept typed so a caller can tell a
/// host-path refusal from an empty binding from a digest mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineageError {
    EmptyField {
        field: &'static str,
    },
    AbsolutePath {
        field: &'static str,
    },
    HostPathMarker {
        field: &'static str,
        marker: &'static str,
    },
    TooLong {
        field: &'static str,
        limit: usize,
    },
    EmptyBinding {
        field: &'static str,
    },
    SchemaVersionDrift {
        expected: String,
        observed: String,
    },
    BindingDigestMismatch {
        stored: String,
        recomputed: String,
    },
    SerializationFailed {
        detail: String,
    },
}

impl fmt::Display for LineageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "verdict field `{field}` is empty"),
            Self::AbsolutePath { field } => {
                write!(
                    formatter,
                    "verdict field `{field}` must not be an absolute host path"
                )
            }
            Self::HostPathMarker { field, marker } => write!(
                formatter,
                "verdict field `{field}` carries host marker `{marker}`"
            ),
            Self::TooLong { field, limit } => {
                write!(
                    formatter,
                    "verdict field `{field}` exceeds {limit} characters"
                )
            }
            Self::EmptyBinding { field } => write!(
                formatter,
                "verdict binding `{field}` must not be empty; an empty binding cannot be a PASS"
            ),
            Self::SchemaVersionDrift { expected, observed } => write!(
                formatter,
                "verdict schema drift: expected `{expected}`, observed `{observed}`"
            ),
            Self::BindingDigestMismatch { stored, recomputed } => write!(
                formatter,
                "verdict binding digest mismatch: stored {stored}, recomputed {recomputed}"
            ),
            Self::SerializationFailed { detail } => {
                write!(formatter, "verdict serialization failed: {detail}")
            }
        }
    }
}

impl std::error::Error for LineageError {}

fn validate_host_free(field: &'static str, value: &str) -> Result<(), LineageError> {
    if value.is_empty() {
        return Err(LineageError::EmptyField { field });
    }
    if value.starts_with('/') {
        return Err(LineageError::AbsolutePath { field });
    }
    for marker in HOST_PATH_MARKERS {
        if value.contains(marker) {
            return Err(LineageError::HostPathMarker { field, marker });
        }
    }
    Ok(())
}

fn basename(program: &str) -> &str {
    program
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(program)
}

/// How a verdict was invoked. The argv is hashed, never stored, so absolute
/// host paths used by a test cannot leak into durable evidence; the digest still
/// changes when the command or any argument changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandIdentity {
    /// Stable, host-independent identifier, e.g. `da document_segmentation_shadow`.
    pub identifier: String,
    /// Executable basename (never a full path).
    pub program: String,
    /// `sha256` over the executable basename and every argv element, newline
    /// separated.
    pub argv_digest: ContentDigest,
}

impl CommandIdentity {
    pub fn new(identifier: &str, program: &str, argv: &[String]) -> Result<Self, LineageError> {
        validate_host_free("command.identifier", identifier)?;
        let program = basename(program);
        validate_host_free("command.program", program)?;
        let mut hashed = String::from(program);
        for argument in argv {
            hashed.push('\n');
            hashed.push_str(argument);
        }
        Ok(Self {
            identifier: identifier.to_owned(),
            program: program.to_owned(),
            argv_digest: ContentDigest::of_text(&hashed),
        })
    }
}

/// A content-addressed input or produced artifact. `name` is a repo-relative
/// path or a logical name, never a host path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundArtifact {
    pub name: String,
    pub sha256: ContentDigest,
}

impl BoundArtifact {
    pub fn new(name: &str, sha256: ContentDigest) -> Result<Self, LineageError> {
        validate_host_free("artifact.name", name)?;
        Ok(Self {
            name: name.to_owned(),
            sha256,
        })
    }

    pub fn of_bytes(name: &str, bytes: &[u8]) -> Result<Self, LineageError> {
        Self::new(name, ContentDigest::of_bytes(bytes))
    }
}

/// The source revision a verdict was executed against, e.g. `git:<sha>` or the
/// engine aggregate `sha256:<hex>`. Opaque and validated host-free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceRevision(String);

impl SourceRevision {
    pub const MAX_LEN: usize = 200;

    pub fn new(value: &str) -> Result<Self, LineageError> {
        validate_host_free("source_revision", value)?;
        if value.len() > Self::MAX_LEN {
            return Err(LineageError::TooLong {
                field: "source_revision",
                limit: Self::MAX_LEN,
            });
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SourceRevision {
    type Error = LineageError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<SourceRevision> for String {
    fn from(value: SourceRevision) -> Self {
        value.0
    }
}

/// One assertion the test parsed out of a produced artifact, kept as the
/// artifact path plus the expected value so a reader can re-evaluate it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParsedAssertion {
    pub path: String,
    pub expected: serde_json::Value,
}

impl ParsedAssertion {
    pub fn new(path: &str, expected: serde_json::Value) -> Result<Self, LineageError> {
        validate_host_free("assertion.path", path)?;
        Ok(Self {
            path: path.to_owned(),
            expected,
        })
    }
}

/// Everything a verdict is bound to. Serializable, non-empty and host-free.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerdictBinding {
    pub claim: VerdictClaim,
    pub command: CommandIdentity,
    pub inputs: Vec<BoundArtifact>,
    pub source_revision: SourceRevision,
    pub artifacts: Vec<BoundArtifact>,
    pub assertions: Vec<ParsedAssertion>,
}

impl VerdictBinding {
    pub fn new(
        claim: VerdictClaim,
        command: CommandIdentity,
        inputs: Vec<BoundArtifact>,
        source_revision: SourceRevision,
        artifacts: Vec<BoundArtifact>,
        assertions: Vec<ParsedAssertion>,
    ) -> Result<Self, LineageError> {
        for (field, empty) in [
            ("inputs", inputs.is_empty()),
            ("artifacts", artifacts.is_empty()),
            ("assertions", assertions.is_empty()),
        ] {
            if empty {
                return Err(LineageError::EmptyBinding { field });
            }
        }
        Ok(Self {
            claim,
            command,
            inputs,
            source_revision,
            artifacts,
            assertions,
        })
    }

    /// Recomputed digest over the canonical serialization of this binding.
    /// Struct field order and `serde_json::Map` key ordering are deterministic,
    /// so the same binding always produces the same digest.
    pub fn digest(&self) -> Result<ContentDigest, LineageError> {
        let bytes =
            serde_json::to_vec(self).map_err(|error| LineageError::SerializationFailed {
                detail: error.to_string(),
            })?;
        Ok(ContentDigest::of_bytes(&bytes))
    }

    /// Every way the bound payload differs from an observed one. The claim gate
    /// is applied first, so a non-execution claim is always a failure.
    pub fn drift_against(&self, observed: &Self) -> Vec<VerdictFailure> {
        let mut reasons = Vec::new();
        if self.claim != VerdictClaim::Execution {
            reasons.push(VerdictFailure::UnsupportedClaim { claim: self.claim });
        }
        if observed.claim != VerdictClaim::Execution {
            reasons.push(VerdictFailure::UnsupportedClaim {
                claim: observed.claim,
            });
        }
        if self.claim != observed.claim {
            reasons.push(VerdictFailure::ClaimMismatch {
                bound: self.claim,
                observed: observed.claim,
            });
        }
        if self.command != observed.command {
            reasons.push(VerdictFailure::CommandMismatch {
                bound: self.command.clone(),
                observed: observed.command.clone(),
            });
        }
        if self.source_revision != observed.source_revision {
            reasons.push(VerdictFailure::SourceRevisionMismatch {
                bound: self.source_revision.as_str().to_owned(),
                observed: observed.source_revision.as_str().to_owned(),
            });
        }
        reasons.extend(artifact_drift("input", &self.inputs, &observed.inputs));
        reasons.extend(artifact_drift(
            "artifact",
            &self.artifacts,
            &observed.artifacts,
        ));
        reasons.extend(assertion_drift(&self.assertions, &observed.assertions));
        reasons
    }
}

fn artifact_drift(
    role: &'static str,
    bound: &[BoundArtifact],
    observed: &[BoundArtifact],
) -> Vec<VerdictFailure> {
    let bound_by_name: BTreeMap<&str, &str> = bound
        .iter()
        .map(|item| (item.name.as_str(), item.sha256.as_str()))
        .collect();
    let observed_by_name: BTreeMap<&str, &str> = observed
        .iter()
        .map(|item| (item.name.as_str(), item.sha256.as_str()))
        .collect();
    let mut reasons = Vec::new();
    for (name, digest) in &bound_by_name {
        match observed_by_name.get(name) {
            None => reasons.push(VerdictFailure::ArtifactMissing {
                role: role.to_owned(),
                name: (*name).to_owned(),
            }),
            Some(observed_digest) if observed_digest != digest => {
                reasons.push(VerdictFailure::ArtifactDigestMismatch {
                    role: role.to_owned(),
                    name: (*name).to_owned(),
                    bound: (*digest).to_owned(),
                    observed: (*observed_digest).to_owned(),
                });
            }
            Some(_) => {}
        }
    }
    for name in observed_by_name.keys() {
        if !bound_by_name.contains_key(name) {
            reasons.push(VerdictFailure::ArtifactUnexpected {
                role: role.to_owned(),
                name: (*name).to_owned(),
            });
        }
    }
    reasons
}

fn assertion_drift(bound: &[ParsedAssertion], observed: &[ParsedAssertion]) -> Vec<VerdictFailure> {
    let bound_by_path: BTreeMap<&str, &serde_json::Value> = bound
        .iter()
        .map(|item| (item.path.as_str(), &item.expected))
        .collect();
    let observed_by_path: BTreeMap<&str, &serde_json::Value> = observed
        .iter()
        .map(|item| (item.path.as_str(), &item.expected))
        .collect();
    let mut reasons = Vec::new();
    for (path, expected) in &bound_by_path {
        match observed_by_path.get(path) {
            None => reasons.push(VerdictFailure::AssertionMissing {
                path: (*path).to_owned(),
            }),
            Some(observed_expected) if observed_expected != expected => {
                reasons.push(VerdictFailure::AssertionMismatch {
                    path: (*path).to_owned(),
                    bound: (*expected).clone(),
                    observed: (*observed_expected).clone(),
                });
            }
            Some(_) => {}
        }
    }
    for path in observed_by_path.keys() {
        if !bound_by_path.contains_key(path) {
            reasons.push(VerdictFailure::AssertionUnexpected {
                path: (*path).to_owned(),
            });
        }
    }
    reasons
}

/// A single reason a verdict is not an execution PASS.
// `Eq` is intentionally absent: `AssertionMismatch` carries a
// `serde_json::Value`, which is only `PartialEq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerdictFailure {
    /// The stored binding digest no longer matches the bound payload.
    BindingInvalid {
        detail: String,
    },
    /// The bound or observed claim is not an execution claim.
    UnsupportedClaim {
        claim: VerdictClaim,
    },
    ClaimMismatch {
        bound: VerdictClaim,
        observed: VerdictClaim,
    },
    CommandMismatch {
        bound: CommandIdentity,
        observed: CommandIdentity,
    },
    SourceRevisionMismatch {
        bound: String,
        observed: String,
    },
    ArtifactMissing {
        role: String,
        name: String,
    },
    ArtifactDigestMismatch {
        role: String,
        name: String,
        bound: String,
        observed: String,
    },
    ArtifactUnexpected {
        role: String,
        name: String,
    },
    AssertionMissing {
        path: String,
    },
    AssertionMismatch {
        path: String,
        bound: serde_json::Value,
        observed: serde_json::Value,
    },
    AssertionUnexpected {
        path: String,
    },
}

/// The outcome of [`VerdictLineage::verify`]. There is deliberately no plain
/// `Pass` variant: the only success is scoped to execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum VerdictStatus {
    /// The bound command executed and matched the observed binding. This is an
    /// execution observation, not a quality, parity or promotion claim.
    ExecutionPass,
    /// Fail-closed: at least one binding drifted or is invalid.
    Failed { reasons: Vec<VerdictFailure> },
}

impl VerdictStatus {
    pub fn is_execution_pass(&self) -> bool {
        matches!(self, Self::ExecutionPass)
    }

    /// The claim this status speaks to. An execution PASS can only ever report
    /// `Some(Execution)`; a serialized status cannot widen it to quality or
    /// parity.
    pub fn claim(&self) -> Option<VerdictClaim> {
        match self {
            Self::ExecutionPass => Some(VerdictClaim::Execution),
            Self::Failed { .. } => None,
        }
    }
}

/// Durable verdict evidence: a binding plus the digest that detects edits to it.
/// There is no stored status field; call [`VerdictLineage::verify`] to obtain one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerdictLineage {
    pub schema_version: String,
    pub binding: VerdictBinding,
    pub binding_sha256: ContentDigest,
}

impl VerdictLineage {
    pub fn new(binding: VerdictBinding) -> Result<Self, LineageError> {
        let binding_sha256 = binding.digest()?;
        Ok(Self {
            schema_version: UAT_VERDICT_LINEAGE_SCHEMA_VERSION.to_owned(),
            binding,
            binding_sha256,
        })
    }

    /// Recompute the binding digest from the payload and compare it to the
    /// stored one. Nothing about validity is read from a field.
    pub fn verify_binding(&self) -> Result<(), LineageError> {
        if self.schema_version != UAT_VERDICT_LINEAGE_SCHEMA_VERSION {
            return Err(LineageError::SchemaVersionDrift {
                expected: UAT_VERDICT_LINEAGE_SCHEMA_VERSION.to_owned(),
                observed: self.schema_version.clone(),
            });
        }
        let recomputed = self.binding.digest()?;
        if recomputed != self.binding_sha256 {
            return Err(LineageError::BindingDigestMismatch {
                stored: self.binding_sha256.as_str().to_owned(),
                recomputed: recomputed.as_str().to_owned(),
            });
        }
        Ok(())
    }

    /// The bounded verdict: recompute the binding digest, then compare the bound
    /// payload against `observed`. Any difference is a failure.
    pub fn verify(&self, observed: &VerdictBinding) -> VerdictStatus {
        let mut reasons = Vec::new();
        if let Err(error) = self.verify_binding() {
            reasons.push(VerdictFailure::BindingInvalid {
                detail: error.to_string(),
            });
        }
        reasons.extend(self.binding.drift_against(observed));
        if reasons.is_empty() {
            VerdictStatus::ExecutionPass
        } else {
            VerdictStatus::Failed { reasons }
        }
    }

    pub fn to_json_pretty(&self) -> Result<String, LineageError> {
        serde_json::to_string_pretty(self).map_err(|error| LineageError::SerializationFailed {
            detail: error.to_string(),
        })
    }

    /// Deserialize durable evidence and immediately recompute its binding digest,
    /// so tampered JSON fails to load instead of becoming a trusted verdict.
    pub fn from_json(text: &str) -> Result<Self, LineageError> {
        let lineage: Self =
            serde_json::from_str(text).map_err(|error| LineageError::SerializationFailed {
                detail: error.to_string(),
            })?;
        lineage.verify_binding()?;
        Ok(lineage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentDigest {
        ContentDigest::of_text(label)
    }

    fn command() -> CommandIdentity {
        CommandIdentity::new(
            "da document_segmentation_shadow",
            "/root/daily-archive/target/debug/examples/document_segmentation_shadow",
            &["--report".to_owned(), "out.json".to_owned()],
        )
        .expect("command identity")
    }

    fn binding() -> VerdictBinding {
        VerdictBinding::new(
            VerdictClaim::Execution,
            command(),
            vec![BoundArtifact::new("data/parse/layout_bakeoff.yaml", digest("layout")).unwrap()],
            SourceRevision::new("git:8bf77cc540d3b11b1f3efa5331daf6187dbebe3e").unwrap(),
            vec![
                BoundArtifact::new("document-segmentation-shadow.json", digest("report")).unwrap(),
            ],
            vec![ParsedAssertion::new("policy.import_eligible", serde_json::json!(false)).unwrap()],
        )
        .expect("sample binding")
    }

    #[test]
    fn roundtrip_lineage_and_status() {
        let lineage = VerdictLineage::new(binding()).expect("lineage");
        crate::roundtrip_json(&lineage);
        let status = lineage.verify(&binding());
        assert!(status.is_execution_pass(), "{status:?}");
        crate::roundtrip_json(&status);
        assert_eq!(status.claim(), Some(VerdictClaim::Execution));
    }

    #[test]
    fn unchanged_binding_is_an_execution_pass() {
        let lineage = VerdictLineage::new(binding()).expect("lineage");
        assert_eq!(lineage.verify(&binding()), VerdictStatus::ExecutionPass);
        assert!(lineage.verify_binding().is_ok());
    }

    #[test]
    fn changed_input_digest_fails_verdict() {
        let lineage = VerdictLineage::new(binding()).expect("lineage");
        let mut observed = binding();
        observed.inputs[0].sha256 = digest("layout-mutated");
        let status = lineage.verify(&observed);
        assert!(!status.is_execution_pass(), "mutated input must not pass");
        let reasons = match status {
            VerdictStatus::Failed { reasons } => reasons,
            VerdictStatus::ExecutionPass => unreachable!(),
        };
        assert!(
            reasons.iter().any(|reason| matches!(
                reason,
                VerdictFailure::ArtifactDigestMismatch { role, .. } if role == "input"
            )),
            "{reasons:?}"
        );
        // The stored evidence itself is still internally consistent; only the
        // observed world drifted, which is exactly what verify() must catch.
        assert!(lineage.verify_binding().is_ok());
    }

    #[test]
    fn changed_command_fails_verdict() {
        let lineage = VerdictLineage::new(binding()).expect("lineage");
        let mut observed = binding();
        observed.command = CommandIdentity::new(
            "da document_segmentation_shadow",
            "document_segmentation_shadow",
            &["--report".to_owned(), "other.json".to_owned()],
        )
        .expect("other command");
        let status = lineage.verify(&observed);
        assert!(!status.is_execution_pass());
        assert!(
            matches!(&status, VerdictStatus::Failed { reasons }
                if reasons.iter().any(|reason| matches!(reason, VerdictFailure::CommandMismatch { .. }))),
            "{status:?}"
        );
    }

    #[test]
    fn changed_artifact_digest_fails_verdict() {
        let lineage = VerdictLineage::new(binding()).expect("lineage");
        let mut observed = binding();
        observed.artifacts[0].sha256 = digest("report-mutated");
        let status = lineage.verify(&observed);
        assert!(
            matches!(&status, VerdictStatus::Failed { reasons }
                if reasons.iter().any(|reason| matches!(reason, VerdictFailure::ArtifactDigestMismatch { role, .. } if role == "artifact"))),
            "{status:?}"
        );
    }

    #[test]
    fn changed_source_revision_fails_verdict() {
        let lineage = VerdictLineage::new(binding()).expect("lineage");
        let mut observed = binding();
        observed.source_revision =
            SourceRevision::new("git:0000000000000000000000000000000000000000")
                .expect("other revision");
        assert!(
            matches!(&lineage.verify(&observed), VerdictStatus::Failed { reasons }
                if reasons.iter().any(|reason| matches!(reason, VerdictFailure::SourceRevisionMismatch { .. }))),
        );
    }

    #[test]
    fn quality_claim_is_refused_and_never_becomes_pass() {
        let mut bound = binding();
        bound.claim = VerdictClaim::Quality;
        let lineage = VerdictLineage::new(bound.clone()).expect("lineage");
        let status = lineage.verify(&bound);
        assert_eq!(status.claim(), None);
        assert!(
            matches!(&status, VerdictStatus::Failed { reasons }
                if reasons.contains(&VerdictFailure::UnsupportedClaim { claim: VerdictClaim::Quality })),
            "{status:?}"
        );

        let mut parity = binding();
        parity.claim = VerdictClaim::Parity;
        let parity_lineage = VerdictLineage::new(parity.clone()).expect("lineage");
        assert!(
            matches!(&parity_lineage.verify(&parity), VerdictStatus::Failed { reasons }
                if reasons.contains(&VerdictFailure::UnsupportedClaim { claim: VerdictClaim::Parity })),
        );
    }

    #[test]
    fn stored_status_field_is_rejected_instead_of_read_as_pass() {
        let lineage = VerdictLineage::new(binding()).expect("lineage");
        let mut value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&lineage).unwrap()).unwrap();
        value["status"] = serde_json::json!("PASS");
        value["verdict"] = serde_json::json!("pass");
        let error = VerdictLineage::from_json(&value.to_string()).unwrap_err();
        assert!(
            matches!(error, LineageError::SerializationFailed { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn from_json_rejects_payload_tampering() {
        let lineage = VerdictLineage::new(binding()).expect("lineage");
        let mut value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&lineage).unwrap()).unwrap();
        let tampered = digest("tampered");
        value["binding"]["inputs"][0]["sha256"] = serde_json::json!(tampered.as_str());
        let error = VerdictLineage::from_json(&value.to_string()).unwrap_err();
        assert!(
            matches!(error, LineageError::BindingDigestMismatch { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn binding_digest_is_stable_and_sensitive() {
        let first = VerdictLineage::new(binding()).expect("lineage");
        let second = VerdictLineage::new(binding()).expect("lineage");
        assert_eq!(first.binding_sha256, second.binding_sha256);
        let mut mutated = binding();
        mutated.artifacts[0].sha256 = digest("different");
        let third = VerdictLineage::new(mutated).expect("lineage");
        assert_ne!(first.binding_sha256, third.binding_sha256);
    }

    #[test]
    fn host_paths_are_rejected() {
        assert!(matches!(
            BoundArtifact::new("/tmp/report.json", digest("x")),
            Err(LineageError::AbsolutePath { .. })
        ));
        assert!(matches!(
            BoundArtifact::new("data/root/leak.json", digest("x")),
            Err(LineageError::HostPathMarker { .. })
        ));
        assert!(CommandIdentity::new("cmd", "cmd", &[]).is_ok());
        assert!(matches!(
            SourceRevision::new("file:///root/repo"),
            Err(LineageError::HostPathMarker { .. })
        ));
    }

    #[test]
    fn command_identity_hashes_argv_and_keeps_only_the_basename() {
        let identity = command();
        assert_eq!(identity.program, "document_segmentation_shadow");
        let serialized = serde_json::to_string(&identity).unwrap();
        assert!(!serialized.contains("/root/"), "{serialized}");
        assert!(!serialized.contains("target/debug"), "{serialized}");
        let mut other_argv = vec!["--report".to_owned(), "other.json".to_owned()];
        let changed = CommandIdentity::new(
            "da document_segmentation_shadow",
            "document_segmentation_shadow",
            &other_argv,
        )
        .unwrap();
        assert_ne!(identity.argv_digest, changed.argv_digest);
        other_argv.push("extra".to_owned());
        let mutated = CommandIdentity::new(
            "da document_segmentation_shadow",
            "document_segmentation_shadow",
            &other_argv,
        )
        .unwrap();
        assert_ne!(changed.argv_digest, mutated.argv_digest);
    }

    #[test]
    fn empty_binding_cannot_be_a_pass() {
        assert!(matches!(
            VerdictBinding::new(
                VerdictClaim::Execution,
                command(),
                vec![],
                SourceRevision::new("git:abc").unwrap(),
                vec![BoundArtifact::new("report.json", digest("r")).unwrap()],
                vec![ParsedAssertion::new("a", serde_json::json!(1)).unwrap()],
            ),
            Err(LineageError::EmptyBinding { field: "inputs" })
        ));
        assert!(matches!(
            VerdictBinding::new(
                VerdictClaim::Execution,
                command(),
                vec![BoundArtifact::new("input.json", digest("i")).unwrap()],
                SourceRevision::new("git:abc").unwrap(),
                vec![],
                vec![ParsedAssertion::new("a", serde_json::json!(1)).unwrap()],
            ),
            Err(LineageError::EmptyBinding { field: "artifacts" })
        ));
        assert!(matches!(
            VerdictBinding::new(
                VerdictClaim::Execution,
                command(),
                vec![BoundArtifact::new("input.json", digest("i")).unwrap()],
                SourceRevision::new("git:abc").unwrap(),
                vec![BoundArtifact::new("report.json", digest("r")).unwrap()],
                vec![],
            ),
            Err(LineageError::EmptyBinding {
                field: "assertions"
            })
        ));
    }

    #[test]
    fn schema_drift_is_rejected() {
        let mut lineage = VerdictLineage::new(binding()).expect("lineage");
        lineage.schema_version = "uat-verdict-lineage.v0".to_owned();
        assert!(matches!(
            lineage.verify_binding(),
            Err(LineageError::SchemaVersionDrift { .. })
        ));
    }

    #[test]
    fn durable_json_round_trips_through_disk_shape() {
        let lineage = VerdictLineage::new(binding()).expect("lineage");
        let json = lineage.to_json_pretty().expect("serialize");
        let restored = VerdictLineage::from_json(&json).expect("deserialize");
        assert_eq!(restored, lineage);
        assert_eq!(restored.verify(&binding()), VerdictStatus::ExecutionPass);
    }
}
