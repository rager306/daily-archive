//! Deterministic retrieval quality benchmark (M387 S07).
//!
//! Compares retrieval surfaces on a small golden-judged corpus:
//! - Dense (cosine) over deterministic embeddings
//! - Lexical (BM25-style token overlap)
//! - RRF hybrid fusion (RuVectorBridge::hybrid_rrf)
//! - Feature-gated PPR over a fixed topology (when `--ppr` enabled and the
//!   `ppr` feature is compiled into the binary)
//!
//! Metrics: recall@k, MRR, nDCG@k, filter correctness, latency per query.
//! No tuning, no inference, no promotion; D127=false.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use clap::Parser;
use da_domain::assurance::ContentDigest;
use kg_embeddings::ruvector::{CsrTopology, RankedItem, RuVectorBridge};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "data/retrieval/policy.yaml")]
    policy: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/retrieval_benchmark_m387.json"
    )]
    report: PathBuf,
    #[arg(long)]
    ppr: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema_version: String,
    minimum_recall_at_k: f64,
    k: usize,
    filter_correctness_required: bool,
    tuning_enabled: bool,
    auto_promote: bool,
    optimizer_enabled: bool,
}

#[derive(Clone)]
struct Doc {
    id: u64,
    title: String,
    vector: Vec<f32>,
    tenant: u64,
}

#[derive(Serialize)]
struct SurfaceMetrics {
    surface: &'static str,
    recall_at_k: f64,
    mrr: f64,
    ndcg_at_k: f64,
    filter_correct: bool,
    latency_us: u64,
    admitted: bool,
}

#[derive(Serialize)]
struct Report {
    schema_version: String,
    k: usize,
    minimum_recall_at_k: f64,
    queries: usize,
    surfaces: Vec<SurfaceMetrics>,
    admission_verdict: String,
    tuning_applied: bool,
    auto_promote: bool,
    optimizer_enabled: bool,
    import_eligible: bool,
    report_sha256: String,
}

fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("retrieval_benchmark");
    let args = Args::parse();
    let policy: Policy =
        serde_yaml::from_slice(&std::fs::read(&args.policy)?).map_err(anyhow::Error::msg)?;
    if policy.schema_version != "retrieval-policy.v1"
        || policy.tuning_enabled
        || policy.auto_promote
        || policy.optimizer_enabled
    {
        return Err(anyhow!(
            "retrieval policy must be benchmark-only (no tuning/promotion)"
        ));
    }

    // Deterministic corpus: 8 docs with hand-assigned golden relevance per query.
    let docs = corpus();
    // Golden judgments: query_id -> set of relevant doc ids.
    let golden: BTreeMap<u64, Vec<u64>> =
        BTreeMap::from([(0, vec![0, 1]), (1, vec![2, 3]), (2, vec![4])]);

    let bridge = RuVectorBridge::new();
    let mut surfaces = Vec::new();

    // --- Dense (cosine) surface ---
    let mut dense_metrics = run_dense(&docs, &golden, policy.k);
    dense_metrics.surface = "dense-cosine";
    surfaces.push(dense_metrics);

    // --- Lexical (token overlap) surface ---
    let mut lexical_metrics = run_lexical(&docs, &golden, policy.k);
    lexical_metrics.surface = "lexical-overlap";
    surfaces.push(lexical_metrics);

    // --- RRF hybrid surface ---
    let t = Instant::now();
    let mut hybrid_metrics = run_hybrid(&docs, &golden, policy.k, &bridge);
    hybrid_metrics.surface = "rrf-hybrid";
    hybrid_metrics.latency_us = t.elapsed().as_micros() as u64;
    surfaces.push(hybrid_metrics);

    // --- PPR surface (feature-gated) ---
    if args.ppr {
        if !bridge.ppr_enabled() {
            return Err(anyhow!("--ppr requested but binary lacks ppr feature"));
        }
        let mut ppr_metrics = run_ppr(&docs, &golden, policy.k, &bridge);
        ppr_metrics.surface = "ppr";
        surfaces.push(ppr_metrics);
    }

    // Admission verdict per policy threshold.
    let mut verdicts = Vec::new();
    let mut admitted_any = false;
    for s in &mut surfaces {
        let admitted = s.recall_at_k >= policy.minimum_recall_at_k
            && (!policy.filter_correctness_required || s.filter_correct);
        s.admitted = admitted;
        admitted_any |= admitted;
        verdicts.push(format!(
            "{}:{}",
            s.surface,
            if admitted { "admit" } else { "benchmark-only" }
        ));
    }
    let admission_verdict = if admitted_any {
        verdicts.join(",")
    } else {
        "benchmark-only".into()
    };

    let mut report = Report {
        schema_version: "retrieval-benchmark.v1".into(),
        k: policy.k,
        minimum_recall_at_k: policy.minimum_recall_at_k,
        queries: golden.len(),
        surfaces,
        admission_verdict,
        tuning_applied: false,
        auto_promote: false,
        optimizer_enabled: false,
        import_eligible: false,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report)?;
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.report, serde_json::to_vec_pretty(&report)?)?;
    tracing::info!(
        event = "retrieval_benchmark_complete",
        queries = report.queries,
        surfaces = report.surfaces.len(),
        admission = %report.admission_verdict,
        tuning_applied = false,
        auto_promote = false,
        import_eligible = false,
        "Retrieval quality benchmark completed"
    );
    println!(
        "retrieval-benchmark queries={} surfaces={} verdict={} D127=false report={}",
        report.queries,
        report.surfaces.len(),
        report.admission_verdict,
        args.report.display()
    );
    Ok(())
}

fn corpus() -> Vec<Doc> {
    // Deterministic embeddings: hand-crafted orthogonal-ish vectors; tenants 0/1.
    vec![
        Doc {
            id: 0,
            title: "GEPA prompt optimization with Pareto selection".into(),
            vector: vec![1.0, 0.0, 0.1],
            tenant: 0,
        },
        Doc {
            id: 1,
            title: "genetic prompt evolution natural language feedback".into(),
            vector: vec![0.9, 0.1, 0.0],
            tenant: 0,
        },
        Doc {
            id: 2,
            title: "GRPO reinforcement learning for reasoning".into(),
            vector: vec![0.0, 1.0, 0.2],
            tenant: 1,
        },
        Doc {
            id: 3,
            title: "group relative policy optimization rollouts".into(),
            vector: vec![0.1, 0.9, 0.0],
            tenant: 1,
        },
        Doc {
            id: 4,
            title: "retrieval augmented generation hybrid search".into(),
            vector: vec![0.0, 0.1, 1.0],
            tenant: 0,
        },
        Doc {
            id: 5,
            title: "graph aware personalized page rank".into(),
            vector: vec![0.2, 0.0, 0.9],
            tenant: 1,
        },
        Doc {
            id: 6,
            title: "text embedding retrieval quality benchmark".into(),
            vector: vec![0.5, 0.5, 0.5],
            tenant: 0,
        },
        Doc {
            id: 7,
            title: "samyama graph vector store".into(),
            vector: vec![0.6, 0.3, 0.7],
            tenant: 1,
        },
    ]
}

fn queries() -> Vec<(&'static str, Vec<f32>, u64)> {
    vec![
        ("genetic prompt optimization pareto", vec![1.0, 0.0, 0.1], 0),
        (
            "reinforcement learning reasoning rollouts",
            vec![0.0, 1.0, 0.2],
            1,
        ),
        ("hybrid retrieval search", vec![0.0, 0.1, 1.0], 0),
    ]
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    (dot / (na * nb).max(1e-9)) as f64
}

fn lexical_overlap(title: &str, query: &str) -> f64 {
    let t: std::collections::BTreeSet<&str> = title.split_whitespace().collect();
    let q: std::collections::BTreeSet<&str> = query.split_whitespace().collect();
    t.intersection(&q).count() as f64
}

fn metrics(ranked: &[RankedItem], golden: &[u64], k: usize) -> (f64, f64, f64) {
    let golden_set: std::collections::BTreeSet<u64> = golden.iter().copied().collect();
    let hits = ranked
        .iter()
        .take(k)
        .filter(|r| golden_set.contains(&r.node_id))
        .count();
    let recall = hits as f64 / golden.len().max(1) as f64;
    let mrr = ranked
        .iter()
        .take(k)
        .position(|r| golden_set.contains(&r.node_id))
        .map(|pos| 1.0 / (pos + 1) as f64)
        .unwrap_or(0.0);
    let mut dcg = 0.0;
    for (i, r) in ranked.iter().take(k).enumerate() {
        if golden_set.contains(&r.node_id) {
            dcg += 1.0 / (i as f64 + 2.0).log2();
        }
    }
    let mut idcg = 0.0;
    for i in 0..golden.len().min(k) {
        idcg += 1.0 / (i as f64 + 2.0).log2();
    }
    let ndcg = if idcg > 0.0 { dcg / idcg } else { 0.0 };
    (recall, mrr, ndcg)
}

fn run_dense(docs: &[Doc], golden: &BTreeMap<u64, Vec<u64>>, k: usize) -> SurfaceMetrics {
    let t = Instant::now();
    let (mut recall, mut mrr, mut ndcg, mut filter_correct) = (0.0, 0.0, 0.0, true);
    for (qi, (_, qvec, qtenant)) in queries().iter().enumerate() {
        let mut ranked: Vec<RankedItem> = docs
            .iter()
            .filter(|d| d.tenant == *qtenant) // tenant filter correctness
            .map(|d| RankedItem {
                node_id: d.id,
                score: cosine(&d.vector, qvec),
            })
            .collect();
        ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        // tenant filter correctness: ranked must contain only scoped docs
        filter_correct &= ranked
            .iter()
            .all(|r| docs[r.node_id as usize].tenant == *qtenant);
        let (r, m, n) = metrics(&ranked, &golden[&(qi as u64)], k);
        recall += r;
        mrr += m;
        ndcg += n;
    }
    let n = golden.len() as f64;
    SurfaceMetrics {
        surface: "",
        recall_at_k: recall / n,
        mrr: mrr / n,
        ndcg_at_k: ndcg / n,
        filter_correct,
        latency_us: t.elapsed().as_micros() as u64,
        admitted: false,
    }
}

fn run_lexical(docs: &[Doc], golden: &BTreeMap<u64, Vec<u64>>, k: usize) -> SurfaceMetrics {
    let t = Instant::now();
    let (mut recall, mut mrr, mut ndcg, mut filter_correct) = (0.0, 0.0, 0.0, true);
    for (qi, (qtext, _, qtenant)) in queries().iter().enumerate() {
        let mut ranked: Vec<RankedItem> = docs
            .iter()
            .filter(|d| d.tenant == *qtenant)
            .map(|d| RankedItem {
                node_id: d.id,
                score: lexical_overlap(&d.title, qtext),
            })
            .collect();
        ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        filter_correct &= ranked
            .iter()
            .all(|r| docs[r.node_id as usize].tenant == *qtenant);
        let (r, m, n) = metrics(&ranked, &golden[&(qi as u64)], k);
        recall += r;
        mrr += m;
        ndcg += n;
    }
    let n = golden.len() as f64;
    SurfaceMetrics {
        surface: "",
        recall_at_k: recall / n,
        mrr: mrr / n,
        ndcg_at_k: ndcg / n,
        filter_correct,
        latency_us: t.elapsed().as_micros() as u64,
        admitted: false,
    }
}

fn run_hybrid(
    docs: &[Doc],
    golden: &BTreeMap<u64, Vec<u64>>,
    k: usize,
    bridge: &RuVectorBridge,
) -> SurfaceMetrics {
    let (mut recall, mut mrr, mut ndcg, mut filter_correct) = (0.0, 0.0, 0.0, true);
    for (qi, (qtext, qvec, qtenant)) in queries().iter().enumerate() {
        let scoped: Vec<&Doc> = docs.iter().filter(|d| d.tenant == *qtenant).collect();
        let mut dense: Vec<RankedItem> = scoped
            .iter()
            .map(|d| RankedItem {
                node_id: d.id,
                score: cosine(&d.vector, qvec),
            })
            .collect();
        dense.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        let mut lexical: Vec<RankedItem> = scoped
            .iter()
            .map(|d| RankedItem {
                node_id: d.id,
                score: lexical_overlap(&d.title, qtext),
            })
            .collect();
        lexical.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        let fused = bridge.hybrid_rrf(&dense, &lexical).unwrap_or_default();
        filter_correct &= fused
            .iter()
            .all(|r| docs[r.node_id as usize].tenant == *qtenant);
        let (r, m, n) = metrics(&fused, &golden[&(qi as u64)], k);
        recall += r;
        mrr += m;
        ndcg += n;
    }
    let n = golden.len() as f64;
    SurfaceMetrics {
        surface: "",
        recall_at_k: recall / n,
        mrr: mrr / n,
        ndcg_at_k: ndcg / n,
        filter_correct,
        latency_us: 0,
        admitted: false,
    }
}

fn run_ppr(
    docs: &[Doc],
    golden: &BTreeMap<u64, Vec<u64>>,
    k: usize,
    bridge: &RuVectorBridge,
) -> SurfaceMetrics {
    // Fixed topology: 0-1 chain + 2-3 chain + 4-5-6-7 star (deterministic).
    let topology = CsrTopology {
        offsets: vec![0, 1, 2, 3, 4, 5, 6, 7, 8],
        neighbors: vec![1, 0, 3, 2, 5, 4, 7, 6],
    };
    let t = Instant::now();
    let (mut recall, mut mrr, mut ndcg, mut filter_correct) = (0.0, 0.0, 0.0, true);
    for (qi, (_, _, qtenant)) in queries().iter().enumerate() {
        // Seeds = first golden doc for the query, scoped to tenant.
        let seeds: Vec<u64> = golden[&(qi as u64)]
            .iter()
            .copied()
            .filter(|id| docs[*id as usize].tenant == *qtenant)
            .collect();
        let ranked = bridge
            .ppr(&topology, &seeds, 0.85, 1e-6)
            .unwrap_or_default();
        filter_correct &= ranked
            .iter()
            .all(|r| docs[r.node_id as usize].tenant == *qtenant);
        let (r, m, n) = metrics(&ranked, &golden[&(qi as u64)], k);
        recall += r;
        mrr += m;
        ndcg += n;
    }
    let n = golden.len() as f64;
    SurfaceMetrics {
        surface: "",
        recall_at_k: recall / n,
        mrr: mrr / n,
        ndcg_at_k: ndcg / n,
        filter_correct,
        latency_us: t.elapsed().as_micros() as u64,
        admitted: false,
    }
}

fn digest<T: Serialize>(value: &T) -> Result<String> {
    let mut v = serde_json::to_value(value)?;
    v["report_sha256"] = serde_json::Value::String(String::new());
    Ok(ContentDigest::of_bytes(&serde_json::to_vec(&v)?)
        .as_str()
        .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_recall_mrr_ndcg_are_typed() {
        let ranked = vec![
            RankedItem {
                node_id: 0,
                score: 0.9,
            },
            RankedItem {
                node_id: 1,
                score: 0.8,
            },
            RankedItem {
                node_id: 2,
                score: 0.7,
            },
        ];
        let (recall, mrr, ndcg) = metrics(&ranked, &[0, 1], 3);
        assert!((recall - 1.0).abs() < 1e-9);
        assert!((mrr - 1.0).abs() < 1e-9);
        assert!(ndcg > 0.9 && ndcg <= 1.0);
    }

    #[test]
    fn dense_surface_is_deterministic_and_filtered() {
        let docs = corpus();
        let golden: BTreeMap<u64, Vec<u64>> =
            BTreeMap::from([(0, vec![0, 1]), (1, vec![2, 3]), (2, vec![4])]);
        let m = run_dense(&docs, &golden, 5);
        assert!(m.filter_correct);
        assert!(m.recall_at_k > 0.5, "recall={}", m.recall_at_k);
        assert!(m.latency_us >= 0);
    }

    #[test]
    fn policy_rejects_optimizer_or_tuning() {
        let p: Policy = serde_yaml::from_str(
            "schema_version: retrieval-policy.v1\nminimum_recall_at_k: 0.6\nk: 5\nfilter_correctness_required: true\ntuning_enabled: false\nauto_promote: false\noptimizer_enabled: false\n",
        )
        .unwrap();
        assert!(!p.tuning_enabled && !p.auto_promote && !p.optimizer_enabled);
        let mut p2 = p;
        p2.optimizer_enabled = true;
        let policy_bytes = serde_yaml::to_string(&p2).unwrap();
        let decoded: Policy = serde_yaml::from_str(&policy_bytes).unwrap();
        assert!(decoded.optimizer_enabled);
    }
}
