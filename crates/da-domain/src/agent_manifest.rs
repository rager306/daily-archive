//! Agent manifest types (M451 W1): agents are DATA objects per the
//! agent.schema norm. Wire types only; invariants ride the generic
//! schema_invariants engine over the manifest as a Value.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

/// Default location of the prompt normative register (M510/S02/T02). Agent
/// manifests declare it under `prompt.normative_register`; this constant is
/// the fallback for manifests written before the register existed.
pub const DEFAULT_NORMATIVE_REGISTER: &str = "data/prompts/normatives.yaml";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentManifest {
    pub schema_version: String,
    pub agent_id: String,
    pub agent_version: String,
    #[serde(default)]
    pub schema_ref: String,
    pub identity: Identity,
    pub model: ModelBinding,
    pub prompt: PromptSpec,
    #[serde(default)]
    pub tools: Tools,
    pub memory: Memory,
    pub optimization: Optimization,
    pub guardrails: Guardrails,
    /// Optional critic-style repair contract (DATA). Summarizer omits it.
    #[serde(default)]
    pub repair: Option<RepairSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    pub role: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelBinding {
    pub generate_binding: String,
    #[serde(default)]
    pub fallback_binding: Option<String>,
    #[serde(default)]
    pub judge_binding: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptSpec {
    pub system_file: String,
    #[serde(default)]
    pub inputs: serde_yaml::Mapping,
    /// Path to the prompt normative register this agent's prompt is bound to
    /// (M510/S02/T02). Optional for backward compatibility; absent manifests
    /// resolve to [`DEFAULT_NORMATIVE_REGISTER`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normative_register: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tools {
    #[serde(default)]
    pub read_tools: Vec<String>,
    #[serde(default)]
    pub write_tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub backend: String, // cvf | none
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub trajectory: bool,
    #[serde(default)]
    pub feedback_sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Optimization {
    pub optimizer_enabled: bool, // lock: false
    #[serde(default)]
    pub frozen_eval: Option<String>,
    #[serde(default)]
    pub candidate_slots: Vec<serde_yaml::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guardrails {
    pub injection_policy: String,
    pub max_output_tokens: usize,
    pub rate_limit_per_min: u32,
}

/// Critic repair contract (DATA). Nested as `repair:` on the manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RepairSpec {
    #[serde(default)]
    pub obligation: String,
    #[serde(default)]
    pub mechanism: RepairMechanism,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RepairMechanism {
    #[serde(default)]
    pub reject_triggers: Vec<crate::critic_repair::RepairAction>,
    #[serde(default)]
    pub max_repair_attempts: u32,
    #[serde(default)]
    pub on_exhausted: String,
}

impl AgentManifest {
    pub fn from_yaml_str(yaml: &str) -> Result<Self, String> {
        serde_yaml::from_str(yaml).map_err(|e| e.to_string())
    }

    /// Prompt normative register this agent is bound to (declared path or
    /// [`DEFAULT_NORMATIVE_REGISTER`]).
    pub fn normative_register_path(&self) -> &str {
        self.prompt
            .normative_register
            .as_deref()
            .unwrap_or(DEFAULT_NORMATIVE_REGISTER)
    }

    /// Hard lock check (ADR-059/D136): optimizer must stay off in prod
    /// manifests; enabled requires frozen_eval present.
    pub fn check_locks(&self) -> Result<(), String> {
        if self.optimization.optimizer_enabled && self.optimization.frozen_eval.is_none() {
            return Err(format!(
                "agent {}: optimizer_enabled=true without frozen_eval (ADR-059 lock)",
                self.agent_id
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Prompt normative register (M510-jsg1pj / S02 / T02)
//
// Machine-readable obligations for prompt and meta-prompt surfaces. Authority
// is recomputed from this register, never read from prompt prose (D418). An
// obligation that is dropped, renamed or weakened does not degrade into a
// recommendation -- it loses its authority entirely: `resolve_authority` fails
// closed, and `validate` reports a typed violation.
// ---------------------------------------------------------------------------

/// Strength of a registered normative. Only `must` confers authority;
/// `should` / `may` are typed explicitly so a weakened obligation is rejected
/// rather than silently downgraded into a recommendation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NormativeStrength {
    Must,
    Should,
    May,
}

impl NormativeStrength {
    pub fn as_str(self) -> &'static str {
        match self {
            NormativeStrength::Must => "must",
            NormativeStrength::Should => "should",
            NormativeStrength::May => "may",
        }
    }
}

/// Enforced obligation vs explicitly marked descriptive material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormativeKind {
    Obligation,
    Descriptive,
}

/// An authority source an obligation may derive from (DATA).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoritySource {
    pub id: String,
    pub kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

/// Typed scalar for a verification parameter. Untagged so YAML booleans and
/// integers stay typed; the operation's declared param schema decides what is
/// allowed. A parameter can never carry an executable shell string in an
/// untyped position -- unknown ops and mismatched params fail validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Bool(bool),
    Int(i64),
    Text(String),
}

impl ParamValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            ParamValue::Bool(_) => "bool",
            ParamValue::Int(_) => "int",
            ParamValue::Text(_) => "string",
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            ParamValue::Text(value) => Some(value.as_str()),
            _ => None,
        }
    }

    fn canonical(&self) -> String {
        match self {
            ParamValue::Bool(value) => (if *value { "true" } else { "false" }).to_string(),
            ParamValue::Int(value) => value.to_string(),
            ParamValue::Text(value) => value.clone(),
        }
    }
}

/// Declared type of one verification parameter (DATA in `verification_ops`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationParamSpec {
    #[serde(rename = "type")]
    pub param_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<String>,
}

/// An allowlisted verification operation: a typed identifier plus a typed
/// parameter schema. The register never stores a shell command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationOpSpec {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub params: BTreeMap<String, VerificationParamSpec>,
}

/// A concrete verification: allowlisted `op` plus typed `params`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verification {
    pub op: String,
    #[serde(default)]
    pub params: BTreeMap<String, ParamValue>,
}

/// One register entry. A single struct (kind-tagged) keeps the DATA flat for
/// the governor and makes each obligation easier to validate against the
/// boundaries; `deny_unknown_fields` means an injected `shell:` field is a
/// parse error, not a silently ignored value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormativeEntry {
    pub kind: NormativeKind,
    pub id: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<NormativeStrength>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authority: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preconditions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_obligation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<Vec<Verification>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optimizer_immutable: Vec<String>,
    /// Descriptive-material marker. `Some(false)` is required for descriptive
    /// entries; an obligation must not carry it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforced: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// The proof / authority boundaries an optimizer must not rewrite (DATA).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormativeBoundaries {
    pub required_optimizer_immutable: Vec<String>,
    #[serde(default)]
    pub forbidden_rewrites: Vec<String>,
}

/// The whole machine-readable prompt normative register.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormativeRegister {
    pub schema_version: String,
    pub register_id: String,
    #[serde(default)]
    pub adr_ref: Vec<String>,
    /// D127 lock: the register never grants import eligibility. Missing is a
    /// violation, so the lock cannot be deleted silently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d127_import_eligible: Option<bool>,
    pub authority_sources: Vec<AuthoritySource>,
    pub verification_ops: Vec<VerificationOpSpec>,
    pub boundaries: NormativeBoundaries,
    pub normatives: Vec<NormativeEntry>,
}

/// A typed validation failure. `code` is a stable token; the committed tests
/// and the governor rule assert on it, so a violation is observable rather
/// than a silent degradation into a recommendation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormativeViolation {
    pub code: String,
    pub normative_id: String,
    pub detail: String,
}

fn violation(
    code: &str,
    normative_id: impl Into<String>,
    detail: impl Into<String>,
) -> NormativeViolation {
    NormativeViolation {
        code: code.to_string(),
        normative_id: normative_id.into(),
        detail: detail.into(),
    }
}

/// The authority an obligation actually holds. Returned only when the whole
/// register validates and the entry is an enforced `must` obligation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormativeAuthority {
    pub normative_id: String,
    pub scope: String,
    pub sources: Vec<String>,
    pub evidence_obligation: String,
    pub optimizer_immutable: Vec<String>,
}

/// Authority source kinds the register recognises.
const AUTHORITY_SOURCE_KINDS: &[&str] = &[
    "requirement",
    "constraint",
    "operability",
    "adr",
    "decision",
    "prompt_contract",
];

/// Verification parameter types the register recognises.
const PARAM_TYPES: &[&str] = &["string", "int", "bool", "path", "digest", "enum"];

/// Rewrite targets that must stay forbidden for an optimizer.
const REQUIRED_FORBIDDEN_REWRITES: &[&str] = &["obligation_id", "authority", "evidence_obligation"];

fn is_repo_relative_path(value: &str) -> bool {
    if value.is_empty() || value.contains('\0') || value.contains("://") {
        return false;
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return false;
    }
    !path.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// How a parameter value conforms to its declared spec. Returns the violation
/// code for the first mismatch, or `Ok(())` when the value is acceptable.
fn check_param(spec: &VerificationParamSpec, value: &ParamValue) -> Result<(), &'static str> {
    match spec.param_type.as_str() {
        "string" => match value {
            ParamValue::Text(_) => Ok(()),
            _ => Err("type_mismatch"),
        },
        "int" => match value {
            ParamValue::Int(_) => Ok(()),
            _ => Err("type_mismatch"),
        },
        "bool" => match value {
            ParamValue::Bool(_) => Ok(()),
            _ => Err("type_mismatch"),
        },
        "path" => match value {
            ParamValue::Text(text) if is_repo_relative_path(text) => Ok(()),
            ParamValue::Text(_) => Err("invalid_path_parameter"),
            _ => Err("type_mismatch"),
        },
        "digest" => match value {
            ParamValue::Text(text) if is_sha256_hex(text) => Ok(()),
            ParamValue::Text(_) => Err("invalid_digest_parameter"),
            _ => Err("type_mismatch"),
        },
        "enum" => match value {
            ParamValue::Text(text) if spec.values.iter().any(|v| v == text) => Ok(()),
            ParamValue::Text(_) => Err("enum_value_not_allowed"),
            _ => Err("type_mismatch"),
        },
        _ => Err("unknown_parameter_type"),
    }
}

impl NormativeEntry {
    pub fn is_obligation(&self) -> bool {
        self.kind == NormativeKind::Obligation
    }

    /// Canonical projection of the immutable boundary fields. Used as the
    /// content-binding digest input: weakening, renaming or dropping content
    /// changes the digest, so authority is bound to content rather than to a
    /// label.
    fn boundary_canonical_string(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.id);
        out.push('\n');
        out.push_str(&self.scope);
        out.push('\n');
        out.push_str(self.strength.map(|s| s.as_str()).unwrap_or("none"));
        out.push('\n');
        for source in &self.authority {
            out.push_str(source);
            out.push(',');
        }
        out.push('\n');
        if let Some(preconditions) = &self.preconditions {
            for precondition in preconditions {
                out.push_str(precondition);
                out.push(',');
            }
        }
        out.push('\n');
        out.push_str(self.evidence_obligation.as_deref().unwrap_or(""));
        out.push('\n');
        if let Some(verifications) = &self.verification {
            for verification in verifications {
                out.push_str(&verification.op);
                out.push(':');
                for (name, value) in &verification.params {
                    out.push_str(name);
                    out.push('=');
                    out.push_str(&value.canonical());
                    out.push(',');
                }
                out.push(';');
            }
        }
        out.push('\n');
        for boundary in &self.optimizer_immutable {
            out.push_str(boundary);
            out.push(',');
        }
        out
    }
}

impl NormativeRegister {
    /// Parse only. Use [`Self::load`] when authority matters.
    pub fn from_yaml_str(yaml: &str) -> Result<Self, String> {
        serde_yaml::from_str(yaml).map_err(|e| e.to_string())
    }

    /// Parse and fully validate, fail-closed. This is the only safe entry
    /// point: an invalid register has no authority at all.
    pub fn load(yaml: &str) -> Result<Self, String> {
        let register = Self::from_yaml_str(yaml)?;
        match register.validate() {
            Ok(()) => Ok(register),
            Err(violations) => Err(violations
                .iter()
                .map(|v| format!("{}:{}: {}", v.code, v.normative_id, v.detail))
                .collect::<Vec<_>>()
                .join("; ")),
        }
    }

    /// Structural + semantic validation. Returns every violation found so a
    /// caller can see exactly why an obligation lost its authority.
    pub fn validate(&self) -> Result<(), Vec<NormativeViolation>> {
        let mut out = Vec::new();

        if self.schema_version != "prompt-normatives.v1" {
            out.push(violation(
                "unsupported_schema_version",
                &self.register_id,
                &self.schema_version,
            ));
        }
        match self.d127_import_eligible {
            Some(false) => {}
            Some(true) => out.push(violation(
                "import_eligibility_leak",
                &self.register_id,
                "d127_import_eligible must stay false (D127)",
            )),
            None => out.push(violation(
                "missing_import_lock",
                &self.register_id,
                "d127_import_eligible must be declared false",
            )),
        }

        let required_immutable: BTreeSet<&str> = self
            .boundaries
            .required_optimizer_immutable
            .iter()
            .map(String::as_str)
            .collect();
        for required in ["proof", "authority", "evidence"] {
            if !required_immutable.contains(required) {
                out.push(violation(
                    "incomplete_immutable_boundary_set",
                    &self.register_id,
                    required,
                ));
            }
        }
        for required in REQUIRED_FORBIDDEN_REWRITES {
            if !self
                .boundaries
                .forbidden_rewrites
                .iter()
                .any(|f| f == required)
            {
                out.push(violation(
                    "incomplete_forbidden_rewrites",
                    &self.register_id,
                    *required,
                ));
            }
        }

        let mut source_ids: BTreeSet<&str> = BTreeSet::new();
        for source in &self.authority_sources {
            if !source_ids.insert(source.id.as_str()) {
                out.push(violation(
                    "duplicate_authority_source",
                    &source.id,
                    "authority source id must be unique",
                ));
            }
            if !AUTHORITY_SOURCE_KINDS.contains(&source.kind.as_str()) {
                out.push(violation(
                    "unknown_authority_source_kind",
                    &source.id,
                    &source.kind,
                ));
            }
        }

        let mut op_specs: BTreeMap<&str, &VerificationOpSpec> = BTreeMap::new();
        for op in &self.verification_ops {
            if op_specs.insert(op.id.as_str(), op).is_some() {
                out.push(violation(
                    "duplicate_verification_op",
                    &op.id,
                    "verification op id must be unique",
                ));
            }
            for (name, spec) in &op.params {
                if !PARAM_TYPES.contains(&spec.param_type.as_str()) {
                    out.push(violation(
                        "unknown_parameter_type",
                        &op.id,
                        format!("{name}: {}", spec.param_type),
                    ));
                }
                if spec.param_type == "enum" && spec.values.is_empty() {
                    out.push(violation("enum_without_values", &op.id, name));
                }
            }
        }

        let mut seen_ids: BTreeSet<&str> = BTreeSet::new();
        for entry in &self.normatives {
            if !seen_ids.insert(entry.id.as_str()) {
                out.push(violation(
                    "duplicate_normative_id",
                    &entry.id,
                    "normative id must be unique",
                ));
            }
            if entry.is_obligation() {
                self.validate_obligation(entry, &source_ids, &op_specs, &mut out);
            } else {
                Self::validate_descriptive(entry, &mut out);
            }
        }

        if out.is_empty() { Ok(()) } else { Err(out) }
    }

    fn validate_obligation(
        &self,
        entry: &NormativeEntry,
        source_ids: &BTreeSet<&str>,
        op_specs: &BTreeMap<&str, &VerificationOpSpec>,
        out: &mut Vec<NormativeViolation>,
    ) {
        let id = entry.id.as_str();

        if entry.enforced.is_some() {
            out.push(violation(
                "obligation_marked_descriptive",
                id,
                "an enforced obligation must not carry the descriptive `enforced` marker",
            ));
        }
        match entry.strength {
            Some(NormativeStrength::Must) => {}
            Some(other) => out.push(violation(
                "weak_obligation",
                id,
                format!("strength={}; only `must` confers authority", other.as_str()),
            )),
            None => out.push(violation("missing_strength", id, "strength is required")),
        }

        if entry.authority.is_empty() {
            out.push(violation(
                "missing_authority",
                id,
                "an obligation must derive authority from at least one source",
            ));
        }
        for source in &entry.authority {
            if !source_ids.contains(source.as_str()) {
                out.push(violation("unknown_authority", id, source));
            }
        }

        if entry.preconditions.is_none() {
            out.push(violation(
                "missing_preconditions",
                id,
                "preconditions must be declared (an empty list is allowed)",
            ));
        }

        match entry.evidence_obligation.as_deref() {
            Some(obligation) if !obligation.trim().is_empty() => {}
            _ => out.push(violation(
                "missing_evidence_obligation",
                id,
                "an obligation must name its durable evidence",
            )),
        }

        match entry.verification.as_ref() {
            Some(verifications) if !verifications.is_empty() => {
                for verification in verifications {
                    match op_specs.get(verification.op.as_str()) {
                        None => out.push(violation(
                            "unknown_verification_op",
                            id,
                            format!("op `{}` is not allowlisted", verification.op),
                        )),
                        Some(spec) => {
                            for (name, param_spec) in &spec.params {
                                match verification.params.get(name) {
                                    None => out.push(violation(
                                        "missing_parameter",
                                        id,
                                        format!("{}:{} is required", verification.op, name),
                                    )),
                                    Some(value) => {
                                        if let Err(code) = check_param(param_spec, value) {
                                            out.push(violation(
                                                code,
                                                id,
                                                format!(
                                                    "{}:{} expected {} got {}",
                                                    verification.op,
                                                    name,
                                                    param_spec.param_type,
                                                    value.type_name()
                                                ),
                                            ));
                                        }
                                    }
                                }
                            }
                            for name in verification.params.keys() {
                                if !spec.params.contains_key(name) {
                                    out.push(violation(
                                        "untyped_parameter",
                                        id,
                                        format!(
                                            "{}:{} is not declared by the op schema",
                                            verification.op, name
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            _ => out.push(violation(
                "missing_verification",
                id,
                "an obligation must declare at least one typed verification operation",
            )),
        }

        if entry.optimizer_immutable.is_empty() {
            out.push(violation(
                "missing_immutable_boundary",
                id,
                "optimizer_immutable must declare the preserved boundaries",
            ));
        }
        for required in &self.boundaries.required_optimizer_immutable {
            if !entry.optimizer_immutable.contains(required) {
                out.push(violation(
                    "missing_immutable_boundary",
                    id,
                    format!("boundary `{required}` is not preserved"),
                ));
            }
        }
    }

    fn validate_descriptive(entry: &NormativeEntry, out: &mut Vec<NormativeViolation>) {
        let id = entry.id.as_str();

        if entry.enforced != Some(false) {
            out.push(violation(
                "descriptive_marked_enforced",
                id,
                "descriptive material must be explicitly marked `enforced: false`",
            ));
        }
        if !entry.authority.is_empty() {
            out.push(violation(
                "descriptive_authority_leak",
                id,
                "descriptive material must not derive authority",
            ));
        }
        if entry.verification.is_some() {
            out.push(violation(
                "descriptive_carries_verification",
                id,
                "descriptive material must not carry a verification operation",
            ));
        }
        if entry.strength.is_some() || entry.evidence_obligation.is_some() {
            out.push(violation(
                "descriptive_carries_obligation",
                id,
                "descriptive material must not carry obligation fields",
            ));
        }
        if !entry.optimizer_immutable.is_empty() {
            out.push(violation(
                "descriptive_immutable_boundary",
                id,
                "descriptive material must not claim immutable boundaries",
            ));
        }
        match entry.text.as_deref() {
            Some(text) if !text.trim().is_empty() => {}
            _ => out.push(violation(
                "missing_descriptive_text",
                id,
                "descriptive material must carry its text",
            )),
        }
    }

    /// Resolve the authority an obligation actually holds. Fails closed: the
    /// whole register must validate, the entry must be an enforced `must`
    /// obligation, and it must carry authority, evidence and immutable
    /// boundaries. A dropped, renamed or weakened obligation resolves to
    /// `None` -- never to a recommendation.
    pub fn authority_for(&self, id: &str) -> Option<NormativeAuthority> {
        if self.validate().is_err() {
            return None;
        }
        self.normatives
            .iter()
            .find(|entry| entry.is_obligation() && entry.id == id)
            .map(|entry| NormativeAuthority {
                normative_id: entry.id.clone(),
                scope: entry.scope.clone(),
                sources: entry.authority.clone(),
                evidence_obligation: entry.evidence_obligation.clone().unwrap_or_default(),
                optimizer_immutable: entry.optimizer_immutable.clone(),
            })
    }

    /// Parse + validate + resolve in one call. `None` on any failure.
    pub fn resolve_authority(yaml: &str, id: &str) -> Option<NormativeAuthority> {
        Self::load(yaml).ok()?.authority_for(id)
    }

    /// Content-binding digest over every obligation's immutable boundary
    /// fields. Renaming, weakening or dropping an obligation changes it, so a
    /// change in "authority" is traceable to a change in content.
    pub fn authority_digest(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for entry in self.normatives.iter().filter(|entry| entry.is_obligation()) {
            hasher.update(entry.boundary_canonical_string().as_bytes());
            hasher.update([0u8]);
        }
        format!("sha256:{}", hex::encode(hasher.finalize()))
    }

    /// Convenience: all registered obligation ids.
    pub fn obligation_ids(&self) -> Vec<&str> {
        self.normatives
            .iter()
            .filter(|entry| entry.is_obligation())
            .map(|entry| entry.id.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: &str = "schema_version: agent-manifest.v1\nagent_id: summarizer\nagent_version: 1.0.0\nidentity: {role: summarizer, description: d, enabled: true}\nmodel: {generate_binding: paper-summary-generate-default}\nprompt: {system_file: data/prompts/summarizer/system.v1.md}\nmemory: {backend: cvf, trajectory: true}\noptimization: {optimizer_enabled: false, candidate_slots: []}\nguardrails: {injection_policy: p, max_output_tokens: 4000, rate_limit_per_min: 12}\n";

    #[test]
    fn parses_manifest_and_locks_hold() {
        let m = AgentManifest::from_yaml_str(M).unwrap();
        assert_eq!(m.agent_id, "summarizer");
        assert_eq!(m.model.generate_binding, "paper-summary-generate-default");
        assert!(m.check_locks().is_ok());
        assert!(m.memory.trajectory);
    }

    #[test]
    fn summarizer_yaml_still_parses_without_repair_section() {
        let m = AgentManifest::from_yaml_str(M).unwrap();
        assert!(m.repair.is_none());
    }

    #[test]
    fn critic_manifest_parses_figure_analysis_trigger() {
        let yaml = include_str!("../../../data/agents/critic/manifest.yaml");
        let m = AgentManifest::from_yaml_str(yaml).expect("critic manifest must parse");
        let repair = m.repair.expect("critic repair section must not be dropped");
        assert_eq!(repair.mechanism.max_repair_attempts, 2);
        let stages: Vec<&str> = repair
            .mechanism
            .reject_triggers
            .iter()
            .map(|t| t.stage.as_str())
            .collect();
        assert!(stages.contains(&"figure_analysis"), "{stages:?}");
        assert!(stages.contains(&"enriched"));
        assert!(stages.contains(&"summaries"));
    }

    #[test]
    fn optimizer_without_frozen_eval_is_rejected() {
        let bad = M.replace("optimizer_enabled: false", "optimizer_enabled: true");
        let m = AgentManifest::from_yaml_str(&bad).unwrap();
        assert!(m.check_locks().is_err());
    }
}
