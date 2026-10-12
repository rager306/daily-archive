# ADR-079: Queen Faculties for the Parse Policy Plane — Health Report, Confidence Stamps, Hierarchical Bucket Fallback, Pre-Parse Analysis

- Status: Accepted
- Date: 2026-10-12
- Deciders: repository owner + agent (owner selected the faculties to port)
- Decisions: D503 (port four Queen faculties in deterministic form; reject five), D504 (health report is derived data from the observation journal), D505 (consult detail carries bucket, support, fallback level — counts, never probabilities), D506 (fallback is a fixed suffix-dropping chain that never generalizes across text-layer quality), D507 (pre-parse analysis feeds only declared decision points, from journal history, never from a model)
- Requirements: R146–R150 (new); inherits R142–R145 (ADR-078) and R118–R124 (ADR-076)
- Milestone: parallel branch `feat/learning-critic-e1` (GSD M516 unaffected)
- Amends: ADR-078 (the learned table gains per-cell support and a fallback chain; consult detail is a superset of the stamp), ADR-074 (the declared-but-dead decision points get a data path)
- Binding level: binding for the form of the four ported faculties and the not-ported list; does not amend ADR-076 waves, promotion rules, or the three-automata separation
- Revisable: after faculty A runs over a real journal and faculty D is exercised with F4 two-arm data; or if the fallback chain proves to blur route boundaries
- Tags: policy, fsm, queen-pattern, health, confidence, fallback, observation-journal

## 0. One-line decision

> We port four faculties of the ruFlo "Queen" coordinator into the parse policy
> plane — health reporting, confidence-aware consult detail, hierarchical bucket
> fallback, and pre-parse analysis — each rendered deterministic and
> receipt-driven, and we record the five Queen mechanisms we deliberately do
> not port.

## LLM Reading Notes

- The "Queen pattern" here means: one strategic decision-maker above
  specialist workers. Our Policy FSM already IS that queen (ADR-076's
  three-automata separation is the queen/workers/admission split). This ADR
> adds the faculties the queen currently lacks, not a new layer.
- Every ported faculty derives from data we already collect
  (`observations.jsonl` + `pdf_policy_features`). No new collection path, no
  model, no sidecar probing.
- ADR-078's locks are unchanged: candidate-only, fail-open, digest-sealed,
  no promotion. The fallback chain (D506) widens coverage but never invents a
  code the exact-key table did not earn.
- The consult detail (D505) is a superset for callers that want the WHY; the
  existing `PolicyDecision` shape is untouched, so governed records stay
  compatible.

## 1. Context

A study of ruFlo's `queen-coordinator.ts` (2035 lines: analyzeTask,
delegateToAgents, monitorSwarmHealth, coordinateConsensus,
recordOutcome/learnFromOutcome, findMatchingPatterns) mapped each faculty onto
our parse plane. Already built here: routing (the policy table, ADR-078),
learning from outcomes (the observation journal + `da policy learn`), decision
stamps. Missing: health summarization, decision confidence exposure,
nearest-case degradation, and any data path for the declared-but-dead decision
points (`concurrency_tune`, `retry_policy`, `summary_depth` are empty-native;
`batch_admission` has rules but is not consulted; `head_fallback`,
`visual_bundle`, `critic_sample` are `consult_status.later`).

## 2. Decision

### We will

1. **Faculty A — health report (D504, R146).** A pure aggregation over the
   observation journal producing a `ParseHealthReport`: per decision_point
   totals and ok-rate; per backend (from outcome evidence) ok-rate and latency
   mean/p50/p95, with a drift alarm when more than one backend version appears
   or a non-modal backend's ok-rate crosses a DATA threshold; per bucket
   ok-rate and latency with the slowest bucket named. CLI: `da policy health
   [--journal ...] [--json out]`. No new data is collected; the report is
   derived data with a canonical-JSON digest.
2. **Faculty B — confidence-aware consult detail (D505, R148).** The learned
   table carries a per-cell support count (training evidence, sealed into the
   digest). A new `consult_detail` surface returns
   `{decision, bucket_key, cell_support: Option<u32>, fallback_level: u8}`
   alongside the existing decision. Support is an observation count, never a
   probability; downstream gating thresholds live in DATA, not code.
3. **Faculty C — hierarchical bucket fallback (D506, R147).** The bucket key is
   structured (`point;p=..;b=..;q=..;d=..`). Consult tries a fixed, total
   chain: L0 exact → L1 domain dropped (`d=*`) → L2 domain and bookmarks
   dropped (`d=*,b=*`) → native fail-open. **The chain never generalizes
   across text-layer quality (`q`)**: a scan-vs-text distinction must not be
   blurred. Each level is stamped in the consult detail; replay under the same
   table and features reproduces the same chain.
4. **Faculty D — pre-parse analysis (D507, R149/R150).** A small
   `ParseTaskAnalysis { features, estimated_duration_ms, sample_count,
   risk_flags }` derived from journal history for the bucket (mean latency at
   the deepest covered fallback level) plus classification risk flags (OCR-page
   share, encoding issues). It feeds only declared decision points, starting
   with `batch_admission` (whose native rules already exist); concurrency and
   retry stay DATA-declared natives until a consult site exists. Estimates are
   historical means, never model outputs.
5. **Latency budgets (R149).** Asserted in tests: consult ≤ 10 µs, health
   report over 10k observations ≤ 1 s, learn ≤ 5 s @ 10k. These are budgets,
   not measurements; breaking one fails the test.

### We will not

1. Consensus voting (majority/weighted/unanimous) — correctness here is
   proven by receipts, not votes.
2. Embedding/kNN pattern retrieval — nondeterminism and a 128-cell space make
   the structured fallback (Faculty C) strictly cheaper.
3. Capability scoring of parse lanes — lanes are fixed code paths, not agents.
4. Auto-promotion of learned behavior — candidate-only locks stand.
5. The swarm itself — ADR-074 already recorded "we have GSD".

## 3. Implementation design

### Faculty A — `parse_policy_health.rs` (da-application)

```rust
pub struct ParseHealthReport {
    pub schema_version: String,          // parse-health-report.v1
    pub journal_digest: String,          // digest of the observations read
    pub points: BTreeMap<String, PointHealth>,      // per decision_point
    pub backends: BTreeMap<String, BackendHealth>,  // per backend version
    pub buckets: BTreeMap<String, BucketHealth>,    // per exact bucket key
    pub drift_alarms: Vec<DriftAlarm>,
    pub report_digest: String,
}
pub struct BackendHealth { total: u64, ok: u64, latency_mean_ms: f64, latency_p50_ms: u64, latency_p95_ms: u64 }
```

- Pure function `parse_health(observations: &[PolicyObservation], thresholds: &HealthThresholds) -> ParseHealthReport`.
- Percentiles: nearest-rank on sorted latencies (deterministic, no interpolation).
- Drift alarm rules (DATA thresholds in `data/policies/registry.yaml` under a
  `health:` block): `distinct_backends > 1`, or any non-modal backend ok-rate
  below `min_ok_rate`. Alarm names the offending backend and its owner class
  (`governor-audit` style), it does not suppress anything.
- CLI: `da policy health` reuses the journal loader from `policy learn`.

### Faculty B+C — table support + fallback chain (da-domain, ADR-078 extension)

```rust
pub struct LearnedDecisionTable {
    ...,
    #[serde(default)]
    pub cell_support: BTreeMap<String, u32>,  // training evidence per bucket
}

impl LearnedDecisionTable {
    /// L0 exact → L1 drop domain → L2 drop domain+bookmarks → None.
    /// Never drops the quality band.
    pub fn lookup_with_fallback(&self, point: DecisionPoint, features: &ArticleFeatures)
        -> Option<TableCell>; // { code, bucket_key_used, support, fallback_level }
}

pub struct PolicyConsultDetail {
    pub decision: PolicyDecision,     // unchanged closed shape
    pub bucket_key: String,           // L0 key of the query
    pub cell: Option<TableCell>,      // None ⇒ native fail-open answered
}
```

- Fallback keys are derived by string surgery on the structured key
  (`d=<domain>` → `d=*`, then `b=<n>` → `b=*`), deterministic by construction.
- `NativeFallbackPolicy::consult_detail(point, features)` is the new surface;
  `consult` delegates to it and discards the detail.
- The trainer emits `cell_support` alongside cells; the digest covers
  cells+support (re-observing the same codes with more data legitimately
  produces a new candidate identity).

### Faculty D — `parse_task_analysis.rs` (da-application)

```rust
pub struct ParseTaskAnalysis {
    pub features: ArticleFeatures,
    pub estimated_duration_ms: Option<u64>,
    pub sample_count: u32,             // history depth behind the estimate
    pub risk_flags: Vec<RiskFlag>,     // ScanShare, EncodingIssues, NoHistory
}
```

- Estimate: mean `outcome.latency_ms` over observations matching the deepest
  covered fallback level; `None` + `NoHistory` when no coverage.
- Risk flags derive from the classification-derived features (OCR share ≥
  threshold ⇒ ScanShare; classifier `has_encoding_issues` ⇒ EncodingIssues).
- First consumer: `batch_admission` consult in the parse path (rules already
  exist: skip 0-page, defer low quality). Concurrency/retry wiring is recorded
  as follow-up until consult sites exist.

### Sequencing

A → C+B → D(alongside F4). A runs on data that already accumulates; C+B widen
coverage and auditability from the same table; D needs history that F4 will
also consume.

## 4. Consequences

- The queen gains health, confidence, graceful degradation, and pre-flight
  estimation without adding a runtime dependency, a model, or a new store.
- `PolicyDecision`'s closed shape is untouched; governed-record consumers are
  unaffected until they opt into the detail.
- Table artifacts gain a field; older artifacts load with `cell_support`
  defaulting empty (support unknown ⇒ consult detail reports `None`, never a
  fabricated count).

## 5. Relations

- ADR-078: extended (support, fallback, consult detail). Its locks are
  unchanged.
- ADR-074: the dead decision points get their intended data path.
- ADR-076: unchanged — the queen still emits closed enums and stamps
  `policy_hash`; no TEI, no graph writes.
