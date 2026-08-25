//! SONA bridge: CVF TrajectoryRecord → SONA ReasoningBank pattern extraction.
//!
//! Connects our CVF agent memory (da-ports::agent_memory) to the SONA
//! self-optimizing engine (ruvector-sona crate). Extracts LearnedPattern
//! clusters from accumulated trajectories — "which genres/domains give
//! grounding below threshold" — without any LLM call.
//!
//! ADR-074 Phase W2 (record-only → pattern extraction). No LoRA training
//! yet — that's D136 gated.

use ruvector_sona::reasoning_bank::{PatternConfig, ReasoningBank};
use ruvector_sona::types::{LearnedPattern, PatternType, QueryTrajectory};

/// Convert a CVF trajectory JSON record into a SONA embedding vector.
///
/// Features are deterministic from the article metadata:
/// [grounding_hint, latency_norm, scrubbed_flag, section_count, char_count_log]
/// Padded/truncated to `dim` dimensions.
pub fn features_to_embedding(rec: &serde_json::Value, dim: usize) -> Vec<f32> {
    let mut v = vec![0.0; dim];
    let grounding = rec
        .get("grounding_hint")
        .and_then(|g| g.as_f64())
        .unwrap_or(0.5) as f32;
    let latency = rec.get("latency_ms").and_then(|l| l.as_u64()).unwrap_or(0);
    let latency_norm = (latency as f32 / 30000.0).min(1.0);
    let scrubbed = rec
        .get("injection_scrubbed")
        .and_then(|s| s.as_u64())
        .unwrap_or(0) as f32;
    let tokens = rec
        .get("completion_tokens")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);
    let tokens_norm = (tokens as f32 / 2000.0).min(1.0);

    let base = [
        grounding,
        latency_norm,
        scrubbed,
        tokens_norm,
        1.0 - grounding, // inverse quality for clustering separation
    ];
    for (i, &val) in base.iter().enumerate() {
        if i < dim {
            v[i] = val;
        }
    }
    v
}

/// Build a SONA ReasoningBank from a list of CVF trajectory JSON records.
/// Returns extracted patterns sorted by avg_quality descending.
pub fn build_reasoning_bank(
    trajectories: &[serde_json::Value],
    dim: usize,
    k_clusters: usize,
) -> Vec<LearnedPattern> {
    let config = PatternConfig {
        k_clusters,
        embedding_dim: dim,
        max_iterations: 50,
        convergence_threshold: 0.001,
        min_cluster_size: 2,
        max_trajectories: 10000,
        quality_threshold: 0.3,
    };
    let mut bank = ReasoningBank::new(config);

    for rec in trajectories {
        let embedding = features_to_embedding(rec, dim);
        let quality = rec
            .get("grounding_hint")
            .and_then(|g| g.as_f64())
            .unwrap_or(0.5) as f32;
        let latency_us = rec.get("latency_ms").and_then(|l| l.as_u64()).unwrap_or(0) * 1000;

        let mut traj = QueryTrajectory::new(bank.trajectory_count() as u64 + 1, embedding.clone());
        traj.final_quality = quality.clamp(0.0, 1.0);
        traj.latency_us = latency_us;
        bank.add_trajectory(&traj);
    }

    let mut patterns = bank.extract_patterns();
    patterns.sort_by(|a, b| b.avg_quality.partial_cmp(&a.avg_quality).unwrap());
    patterns
}

/// Human-readable summary of extracted patterns.
pub fn summarize_patterns(patterns: &[LearnedPattern]) -> String {
    let mut out = format!(
        "{} patterns extracted:\n",
        patterns.len()
    );
    for p in patterns {
        out.push_str(&format!(
            "  cluster size={} avg_quality={:.2} accesses={}\n",
            p.cluster_size, p.avg_quality, p.access_count
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_to_embedding_produces_valid_vector() {
        let rec = serde_json::json!({
            "grounding_hint": 0.86,
            "latency_ms": 5000,
            "injection_scrubbed": 1,
            "completion_tokens": 500,
        });
        let v = features_to_embedding(&rec, 8);
        assert_eq!(v.len(), 8);
        assert!((v[0] - 0.86).abs() < 0.01, "grounding at position 0");
        assert!(v[2] > 0.0, "scrubbed flag nonzero");
    }

    #[test]
    fn reasoning_bank_extracts_patterns_from_trajectories() {
        let trajs: Vec<serde_json::Value> = (0..10)
            .map(|i| {
                serde_json::json!({
                    "grounding_hint": if i < 5 { 0.9 } else { 0.3 },
                    "latency_ms": 1000 + i * 100,
                    "injection_scrubbed": 0,
                    "completion_tokens": 500,
                })
            })
            .collect();
        let patterns = build_reasoning_bank(&trajs, 8, 2);
        assert!(!patterns.is_empty(), "patterns must be extracted from clustered trajectories");
    }
}
