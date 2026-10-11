//! T2/T3 (Э2): GGUF inference critic. Loads a registered model through the
//! vendored CandleBackend on CPU, renders the redacted structural subject
//! into the model's chat scaffold, and maps the generated verdict back to
//! the same `parse-learning-ablation.v1` observation contract as the
//! deterministic critic (`observe`).
//!
//! Sampling is greedy (temperature 0.0 -> candle `Sampling::ArgMax`), so
//! identical (model, template, subject) triples yield identical tokens:
//! that determinism is asserted by the model-gated tests below.
//!
//! Provenance differs from the deterministic producer by construction
//! (T3): `code_digest` carries the sha256 of the exact GGUF file AND the
//! sha256 of the prompt template, because the behaviour of this producer
//! is defined by those two blobs, not only by code.

use crate::models::{self, ModelSpec};
use crate::subject::StructuralProfile;
use ruvllm::{CandleBackend, DeviceType, GenerateParams, LlmBackend, ModelConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::Instant;

use crate::critic::{CriticCase, CriticError, CriticObservation, CriticStatus};

pub const GGUF_CRITIC_AUTHORITY: &str = "ruvllm-gguf-candle-cpu";

/// System half of the verdict prompt (rubric; the model still infers the
/// verdict from the subject line itself).
pub const VERDICT_SYSTEM: &str = "You are a parse-quality critic for a scholarly-document \
parsing pipeline. You see only a redacted structural profile of one parsed \
document: tag counts, no text. The citation bibliography is the product. A \
profile with zero or absent sections/references/abstract is a failed parse. A \
profile with many sections, many references, an abstract and header metadata \
is a successful fulltext parse.";

/// User half: the redacted subject plus the output contract.
///
/// Fragility note (measured): adding an explicit decision rule here
/// ("Successful parse: ... Failed parse: any of them zero, absent or
/// false") flipped Llama-3.2-1B to `bad` verdicts on well-formed profiles:
/// the literal `false` values of optional profile fields
/// (`funding_present`, `keywords`) got swept into the rule. The rubric
/// stays in [`VERDICT_SYSTEM`]; every suffix change must be re-probed on
/// BOTH registered models before it ships (see the Э2 report).
pub const VERDICT_USER_PREFIX: &str = "Rate the parse quality of this profile:\n";
pub const VERDICT_USER_SUFFIX: &str = "\n\nAnswer with exactly one JSON object on one line: \
{\"quality\": <number 0.0-1.0>, \"verdict\": \"good\"|\"partial\"|\"bad\"}";

/// Fixed greedy sampling seed (honest under ArgMax; recorded for the
/// non-greedy fallback paths of the sampler).
pub const CRITIC_SEED: u64 = 4711;
/// Token budget for one verdict; the JSON contract and the first prose
/// verdict word both land well inside this. Kept tight because CPU
/// generation is ~0.2 tok/s (measured, TinyLlama Q4_K_M release).
pub const MAX_TOKENS: usize = 28;

/// Fixed score mapping for class-only verdicts (model gave no number).
pub const SCORE_GOOD: f64 = 0.95;
pub const SCORE_PARTIAL: f64 = 0.55;
pub const SCORE_BAD: f64 = 0.05;
/// metric_two threshold, same convention as the deterministic critic.
pub const PASS_THRESHOLD: f64 = 0.8;

/// Parsed verdict extracted from model output.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Numeric quality in [0, 1] straight from the model.
    Quality(f64),
    /// Class verdict; mapped through the fixed score table.
    Class(VerdictClass),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictClass {
    Good,
    Partial,
    Bad,
}

impl VerdictClass {
    fn score(self) -> f64 {
        match self {
            VerdictClass::Good => SCORE_GOOD,
            VerdictClass::Partial => SCORE_PARTIAL,
            VerdictClass::Bad => SCORE_BAD,
        }
    }

    fn from_word(word: &str) -> Option<Self> {
        match word.trim().to_lowercase().as_str() {
            "good" | "well-formed" | "well formed" | "excellent" | "high" => {
                Some(VerdictClass::Good)
            }
            "partial" | "medium" | "moderate" => Some(VerdictClass::Partial),
            "bad" | "degraded" | "poor" | "failed" | "empty" => Some(VerdictClass::Bad),
            _ => None,
        }
    }
}

/// Extract a verdict from raw generated text.
///
/// Strategy: (1) first JSON object with a numeric `quality` or a known
/// `verdict` string — the closing brace may be missing because generation
/// can stop on the `}` stop sequence; (2) first class keyword in the prose
/// otherwise. Returns `None` when the model produced nothing usable.
pub fn extract_verdict(text: &str) -> Option<Verdict> {
    if let Some(candidate) = json_candidate(text) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&candidate) {
            if let Some(q) = v.get("quality").and_then(|q| q.as_f64()) {
                if q.is_finite() {
                    return Some(Verdict::Quality(q.clamp(0.0, 1.0)));
                }
            }
            if let Some(word) = v.get("verdict").and_then(|w| w.as_str()) {
                if let Some(class) = VerdictClass::from_word(word) {
                    return Some(Verdict::Class(class));
                }
            }
        }
    }
    // prose fallback: earliest class keyword wins
    let lower = text.to_lowercase();
    let mut best: Option<(usize, VerdictClass)> = None;
    for word in [
        "well-formed",
        "well formed",
        "excellent",
        "good",
        "partial",
        "moderate",
        "medium",
        "degraded",
        "failed",
        "poor",
        "bad",
        "empty",
    ] {
        if let Some(pos) = lower.find(word) {
            if best.is_none_or(|(p, _)| pos < p) {
                if let Some(class) = VerdictClass::from_word(word) {
                    best = Some((pos, class));
                }
            }
        }
    }
    best.map(|(_, class)| Verdict::Class(class))
}

/// JSON text to parse: the first balanced `{...}` object, or — when
/// generation stopped on the `}` stop sequence — everything from the first
/// `{` with the closing brace re-appended.
fn json_candidate(text: &str) -> Option<String> {
    if let Some((start, end)) = first_json_object(text) {
        return Some(text[start..=end].to_string());
    }
    let start = text.find('{')?;
    if text[start..].contains('}') {
        return None; // unbalanced for another reason; do not guess
    }
    Some(text[start..].to_string() + "}")
}

/// Byte offsets of the first balanced `{...}` object, inclusive of both
/// braces. `None` when there is no complete object.
fn first_json_object(text: &str) -> Option<(usize, usize)> {
    let start = text.find('{')?;
    let mut depth = 0usize;
    for (offset, ch) in text[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, start + offset));
                }
            }
            _ => {}
        }
    }
    None
}

fn sha256_file(path: &Path) -> Result<String, CriticError> {
    let bytes = std::fs::read(path).map_err(|e| CriticError::Io(e.to_string()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

/// A loaded GGUF critic: model + tokenizer resident, verdicts per case.
pub struct GgufCritic {
    backend: CandleBackend,
    spec: &'static ModelSpec,
    model_sha: String,
    template_sha: String,
}

/// Throughput report for one observe run (audit evidence, Э2 report).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationStats {
    pub prompt_tokens_approx: usize,
    pub generated_tokens_approx: usize,
    pub elapsed_ms: u128,
    pub tokens_per_second: f64,
}

/// Full result of one inference observation (audit trail).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserveOutput {
    pub observation: CriticObservation,
    pub stats: GenerationStats,
    /// Raw generated text (after special-token-skipping decode; may be
    /// truncated by the `}` stop sequence). Evidence for the Э2 report.
    pub raw: String,
}

impl GgufCritic {
    /// Resolve `model` (registry id or absolute dir path) and load it on CPU.
    pub fn load(model: &str) -> Result<Self, CriticError> {
        let spec =
            models::resolve(model).ok_or_else(|| CriticError::UnknownModel(model.to_string()))?;
        let dir = Path::new(spec.dir);
        let gguf = dir.join(spec.gguf);
        let tokenizer = dir.join("tokenizer.json");
        if !gguf.is_file() {
            return Err(CriticError::ModelMissing(gguf.display().to_string()));
        }
        if !tokenizer.is_file() {
            return Err(CriticError::ModelMissing(tokenizer.display().to_string()));
        }
        let mut backend = CandleBackend::with_device(DeviceType::Cpu)
            .map_err(|e| CriticError::Backend(e.to_string()))?;
        backend
            .load_tokenizer(&tokenizer)
            .map_err(|e| CriticError::Backend(e.to_string()))?;
        let config = ModelConfig {
            max_sequence_length: spec.ctx as usize,
            ..ModelConfig::default()
        };
        backend
            .load_gguf(&gguf, &config)
            .map_err(|e| CriticError::Backend(e.to_string()))?;
        Ok(Self {
            backend,
            spec,
            model_sha: sha256_file(&gguf)?,
            template_sha: identity_hex(&rendered_template_identity(spec)),
        })
    }

    pub fn spec(&self) -> &'static ModelSpec {
        self.spec
    }

    pub fn model_sha256(&self) -> &str {
        &self.model_sha
    }

    /// T3: producer identity binds code + exact model blob + prompt template.
    pub fn provenance(&self, corpus: &str) -> crate::critic::ProducerProvenance {
        crate::critic::ProducerProvenance {
            // both digests, verbatim: this producer's behaviour is defined
            // by the weights and the prompt, not by this crate's code alone
            code_digest: format!("{}+{}", self.model_sha, self.template_sha),
            input_digest: corpus.to_string(),
            method: format!("ruvllm_gguf_verdict_{}", self.spec.id),
            authority: GGUF_CRITIC_AUTHORITY.into(),
        }
    }

    pub fn producer_label(&self) -> String {
        format!("ruvllm-gguf-{}", self.spec.id)
    }

    /// Observe one case through real model inference (greedy).
    pub fn observe(&self, case: &CriticCase) -> Result<ObserveOutput, CriticError> {
        if case.case_id.is_empty() {
            return Err(CriticError::EmptyCaseId);
        }
        // subject must be a well-formed redacted profile (same boundary as
        // the deterministic critic); garbage in -> Malformed, no inference
        let Some(profile) = StructuralProfile::parse(&case.subject) else {
            return Ok(ObserveOutput {
                observation: CriticObservation {
                    schema_version: crate::critic::CRITIC_SCHEMA_VERSION,
                    case_id: case.case_id.clone(),
                    corpus: case.corpus.clone(),
                    order: case.order,
                    producer: self.producer_label(),
                    metric_one: 0.0,
                    metric_two: 0.0,
                    status: CriticStatus::Malformed,
                },
                stats: GenerationStats {
                    prompt_tokens_approx: 0,
                    generated_tokens_approx: 0,
                    elapsed_ms: 0,
                    tokens_per_second: 0.0,
                },
                raw: String::new(),
            });
        };
        let _ = profile; // validated only; the model sees the subject line
        let prompt = render_prompt(self.spec, &case.subject);
        let t0 = Instant::now();
        let text = self
            .backend
            .generate(
                &prompt,
                GenerateParams {
                    max_tokens: MAX_TOKENS,
                    temperature: 0.0, // greedy ArgMax: deterministic
                    top_p: 1.0,
                    top_k: 0,
                    repetition_penalty: 1.0,
                    stop_sequences: self
                        .spec
                        .stop_sequences
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    seed: Some(CRITIC_SEED),
                    ..Default::default()
                },
            )
            .map_err(|e| CriticError::Backend(e.to_string()))?;
        let elapsed = t0.elapsed();
        let stats = GenerationStats {
            prompt_tokens_approx: approx_tokens(&prompt),
            generated_tokens_approx: approx_tokens(&text),
            elapsed_ms: elapsed.as_millis(),
            tokens_per_second: approx_tokens(&text) as f64 / elapsed.as_secs_f64().max(1e-9),
        };
        let verdict = extract_verdict(&text);
        let observation = match verdict {
            Some(Verdict::Quality(q)) => CriticObservation {
                schema_version: crate::critic::CRITIC_SCHEMA_VERSION,
                case_id: case.case_id.clone(),
                corpus: case.corpus.clone(),
                order: case.order,
                producer: self.producer_label(),
                metric_one: q,
                metric_two: f64::from(q >= PASS_THRESHOLD),
                status: CriticStatus::Ok,
            },
            Some(Verdict::Class(class)) => {
                let score = class.score();
                CriticObservation {
                    schema_version: crate::critic::CRITIC_SCHEMA_VERSION,
                    case_id: case.case_id.clone(),
                    corpus: case.corpus.clone(),
                    order: case.order,
                    producer: self.producer_label(),
                    metric_one: score,
                    metric_two: f64::from(score >= PASS_THRESHOLD),
                    status: CriticStatus::Ok,
                }
            }
            None => CriticObservation {
                schema_version: crate::critic::CRITIC_SCHEMA_VERSION,
                case_id: case.case_id.clone(),
                corpus: case.corpus.clone(),
                order: case.order,
                producer: self.producer_label(),
                metric_one: 0.0,
                metric_two: 0.0,
                status: CriticStatus::Missing,
            },
        };
        Ok(ObserveOutput {
            observation,
            stats,
            raw: text,
        })
    }
}

fn identity_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

/// The template identity hashed for provenance: scaffold + rubric halves.
fn rendered_template_identity(spec: &ModelSpec) -> String {
    format!(
        "template={}|system={}|user_prefix={}|user_suffix={}",
        spec.chat_template, VERDICT_SYSTEM, VERDICT_USER_PREFIX, VERDICT_USER_SUFFIX
    )
}

pub fn render_prompt(spec: &ModelSpec, subject: &str) -> String {
    spec.chat_template
        .replace("{system}", VERDICT_SYSTEM)
        .replace(
            "{user}",
            &format!("{VERDICT_USER_PREFIX}{subject}{VERDICT_USER_SUFFIX}"),
        )
}

/// Cheap token estimate for throughput reporting (not exact; noted in reports).
fn approx_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WELL: &str = "sections: 74; references: 81; figures: 25; tables: 3; formulas: 24; \
        abstract_present: true; funding_present: false; notes: 116; keywords: false; \
        language_set: true";
    const DEGRADED: &str = "sections: 0; references: 0; figures: 0; tables: 0; formulas: 0; \
        abstract_present: false; funding_present: false; notes: 0; keywords: false; \
        language_set: false";

    #[test]
    fn extract_verdict_json_quality() {
        assert_eq!(
            extract_verdict(r#"Here it is: {"quality": 0.9, "verdict": "good"}"#),
            Some(Verdict::Quality(0.9))
        );
    }

    #[test]
    fn extract_verdict_json_clamps() {
        assert_eq!(
            extract_verdict("{\"quality\": 8}"),
            Some(Verdict::Quality(1.0))
        );
        assert_eq!(
            extract_verdict("{\"quality\": -3}"),
            Some(Verdict::Quality(0.0))
        );
    }

    #[test]
    fn extract_verdict_json_class_without_number() {
        assert_eq!(
            extract_verdict("{\"verdict\": \"bad\"}"),
            Some(Verdict::Class(VerdictClass::Bad))
        );
    }

    #[test]
    fn extract_verdict_unclosed_object_from_stop_sequence() {
        // generation stopped on '}' -> text lacks the closing brace
        assert_eq!(
            extract_verdict("{\"quality\": 0.25, \"verdict\": \"partial\""),
            Some(Verdict::Quality(0.25))
        );
        // the exact live shape from the llama-3.2 run
        assert_eq!(
            extract_verdict("{\"quality\": 0.8, \"verdict\": \"good\""),
            Some(Verdict::Quality(0.8))
        );
    }

    #[test]
    fn json_candidate_rejects_unbalanced_garbage() {
        assert_eq!(
            json_candidate("a {\"q\": 1"),
            Some("{\"q\": 1}".to_string())
        );
        assert_eq!(
            json_candidate("a {\"q\": 1} b"),
            Some("{\"q\": 1}".to_string())
        );
        assert_eq!(json_candidate("no braces"), None);
        // lone '{' yields an empty-object candidate; harmless: it parses to
        // {} with no verdict keys and falls through to the prose fallback
        assert_eq!(json_candidate("} {"), Some("{}".to_string()));
    }

    #[test]
    fn extract_verdict_prose_fallback() {
        assert_eq!(
            extract_verdict("This parse looks degraded to me."),
            Some(Verdict::Class(VerdictClass::Bad))
        );
        assert_eq!(
            extract_verdict("The parse is good overall."),
            Some(Verdict::Class(VerdictClass::Good))
        );
    }

    #[test]
    fn extract_verdict_rejects_noise() {
        assert_eq!(extract_verdict("I like turtles."), None);
        assert_eq!(extract_verdict(""), None);
    }

    #[test]
    fn first_json_object_finds_balanced_only() {
        assert_eq!(
            first_json_object("xx {\"a\": {\"b\": 1}} yy"),
            Some((3, 17))
        );
        assert_eq!(first_json_object("no braces"), None);
        assert_eq!(first_json_object("{\"a\": 1"), None);
    }

    #[test]
    fn rendered_prompt_carries_subject_and_bos() {
        let spec = &models::MODELS[0];
        let prompt = render_prompt(spec, WELL);
        assert!(prompt.contains("<|assistant|>"));
        assert!(prompt.contains(WELL));
        assert!(prompt.contains("{\"quality\""));
    }

    // ---- model-gated tests: skip automatically when models are absent ----

    fn model_available(id: &str) -> bool {
        models::resolve(id)
            .map(|m| Path::new(m.dir).join(m.gguf).is_file())
            .unwrap_or(false)
    }

    #[test]
    fn gguf_determinism_two_runs_identical() {
        if !model_available("llama-3.2-1b-instruct") {
            eprintln!("SKIP: llama-3.2-1b-instruct not present");
            return;
        }
        let critic = GgufCritic::load("llama-3.2-1b-instruct").expect("load");
        let case = CriticCase {
            case_id: "determinism".into(),
            corpus: "test-corpus".into(),
            order: 0,
            subject: WELL.into(),
        };
        let a = critic.observe(&case).expect("observe");
        let b = critic.observe(&case).expect("observe");
        let (a, b) = (a.observation, b.observation);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap(),
            "greedy decode must be reproducible for identical inputs"
        );
    }

    #[test]
    fn gguf_verdict_path_and_discrimination() {
        for spec in models::MODELS {
            if !model_available(spec.id) {
                eprintln!("SKIP: {} not present", spec.id);
                continue;
            }
            let critic = GgufCritic::load(spec.id).expect("load");
            let well = critic
                .observe(&CriticCase {
                    case_id: "well".into(),
                    corpus: "c".into(),
                    order: 0,
                    subject: WELL.into(),
                })
                .expect("observe")
                .observation;
            let degraded = critic
                .observe(&CriticCase {
                    case_id: "degraded".into(),
                    corpus: "c".into(),
                    order: 1,
                    subject: DEGRADED.into(),
                })
                .expect("observe")
                .observation;
            // every registered model must produce parseable verdicts
            assert_eq!(
                well.status,
                CriticStatus::Ok,
                "{}: well-formed run must produce a verdict",
                spec.id
            );
            assert_eq!(
                degraded.status,
                CriticStatus::Ok,
                "{}: degraded run must produce a verdict",
                spec.id
            );
            // discrimination is asserted only where the registry expects it
            if spec.discriminates {
                assert!(
                    well.metric_one - degraded.metric_one >= 0.4,
                    "{}: no discrimination (well {}, degraded {})",
                    spec.id,
                    well.metric_one,
                    degraded.metric_one
                );
            }
        }
    }
}
