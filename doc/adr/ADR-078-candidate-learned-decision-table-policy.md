# ADR-078: Candidate Learned Decision Table as the Parse-FSM Policy Artifact

- Status: Accepted
- Date: 2026-10-12
- Deciders: repository owner + agent (owner directed ADR-first recording before further implementation)
- Decisions: D500 (the learned policy artifact is a candidate decision table, not MicroLoRA weights), D501 (one bucketing function shared by training and consult), D502 (observation record carries condition, decision, reward provenance; coverage constraint recorded honestly)
- Requirements: R142–R145 (new); inherits R118–R124 (ADR-076 locks unchanged)
- Milestone: parallel branch `feat/learning-critic-e1` (not a GSD milestone; GSD M516 unaffected, graph untouched)
- Amends: ADR-074 (the learned artifact form in the native consult path: a decision table; WASM packaging remains the production hot-swap form and is deferred, not replaced)
- Binding level: binding for the form of the learned parse-policy artifact, its consult precedence, and the observation record; does not amend ADR-076 waves, promotion rules, or the three-automata separation
- Revisable: after two-arm (shadow A/B) data exists and a measured gain/no-gain is recorded; or when the policy feature set outgrows coarse bucketing (a different function class then needs a new recorded decision)
- Tags: policy, fsm, learning, decision-table, candidate-only, coverage, sona

## 0. One-line decision

> The learned artifact for the parse-FSM policy is a **candidate decision
> table** — a deterministic bucketing of `ArticleFeatures` mapped to a closed
> decision code — trained from recorded observations, content-digested,
> fail-open, and never production-active without an explicit promotion.
>
> We will **not** ship MicroLoRA weights as the runtime policy artifact, record
> observations without their decision condition, or claim learning gain from
> single-arm observational data.

## LLM Reading Notes

- ADR-076 owns sequencing/honesty and is **unchanged**: policy outputs stay
  closed enums plus `policy_hash`; the table emits exactly those enums
  (`GrammarRoute`/`Admission`/`ProfileRoute`), never TEI fields.
- The function-class argument: the policy sees four coarse features
  (pages, bookmarks, text-layer quality, domain) and answers a closed enum. A
  rank-1 MicroLoRA over that input **is** a threshold table with extra moving
  parts (weight IO, D136 gating, no replayability). The table is the same
  function, exactly replayable and auditable. This is why D500 supersedes the
  LoRA-as-artifact idea **for the runtime path only**; the MicroLoRA trainer
  stays as an ablation arm under D294.
- The consult precedence is: **WASM host > learned table (env-gated) > native
  DATA rules**. Uncovered buckets fail open to native. An invalid table is
  refused at attach, never silently consulted.
- The coverage constraint (§5) is the honest center of this ADR: observational
  data under a deterministic policy can only reproduce that policy. The first
  honest measured result is expected to be `measured_no_gain`. Gain requires
  two-arm same-document data, which is deliberately **out of scope** here.

## 1. Context

An audit of the learning loop found it inert end to end:

| Gap | Evidence |
|---|---|
| Trained artifact not consultable | `candidate_forward`/`reload_persisted_candidate` called only by the eval cycle; `consult_with_fallback` knows only WASM host or native thresholds |
| Type mismatch | trainer learned on `TrajectoryRecord` outcome fields; consult answers on domain `ArticleFeatures`; no bridge existed |
| Features fake at consult time | `corpus_features()` hard-codes `{0,0,0.85,""}`; grammar consult reads env vars or a toc cache written by a legacy Python script |
| Decision condition not recorded | `PolicyDecisionRecord` stores decision/source/hash but never the features the decision was conditioned on |
| No reward defined | honest parse reward per ADR-076 is conservation, not extraction F1; nothing converted any outcome into a per-decision reward |
| No coverage | native policy is deterministic → one code per bucket ever observed → any learned table can only imitate native |

ADR-074 already accepted WASM MicroLoRA policy modules; D294 opened training.
What was missing was an artifact form the consult path can actually consume,
and an honest account of what training can and cannot produce.

## 2. Decision

### We will

1. **Ship the learned artifact as a `LearnedDecisionTable`** (D500):
   `BTreeMap<bucket_key, closed_code>` per `DecisionPoint`, sealed with a
   content digest (`table_sha256`), `deny_unknown_fields`, validated on load.
2. **Use exactly one bucketing function** `feature_bucket(point, features)`
   (D501, R142): training and consult call the same function; a cell written
   under any other bucketing is unreachable by construction.
3. **Stamp provenance** via `PolicySource::Learned { table_sha256 }` (R145):
   trajectory records can replay the exact learned artifact; renderers print
   `learned:<sha256>`.
4. **Record observations as first-class data** (D502, R143): each record
   carries document identity (the split key, never the prompt), the decision
   condition (real `ArticleFeatures` derived from `PdfClassification`), the
   decision code actually answered, the reward, and raw outcome evidence
   (`ok`, section count, body chars, latency, backend) so the reward rule is
   re-computable. The first reward rule is deliberately binary
   (`ok -> 1.0, failure -> 0.0`).
5. **Split document-disjoint** with the frozen-corpus rule
   (`document_identity_sha256_rank_modulo;validation_every=4`); a document's
   observations never straddle the split (R143).
6. **Keep candidate-only locks** (R144): `deployed=false`,
   `import_eligible=false`, no promotion branch, no graph write, fail-open to
   native DATA rules for uncovered buckets, env-gated opt-in
   (`DA_LEARNED_POLICY_TABLE`) so a learned table is never production by
   default.

### We will not

1. Ship MicroLoRA weights as the runtime consult artifact (kept as ablation).
2. Record observations without their condition, or rewards without evidence.
3. Add exploration to production consults to manufacture coverage.
4. Claim gain from single-arm data, or treat table≈native as a failure of the
   loop — it is the honest baseline the loop must be able to state.
5. Emit anything but closed enum codes from the policy (ADR-076 unchanged).

## 3. The artifact and its consult path

```
PdfClassification ──► features_from_classification ──► ArticleFeatures
                                                              │
            feature_bucket(point, features)  ◄───────────────┤  (the ONE bucketing)
                        │                                     │
        training: cells[bucket] = argmax_code mean(reward)    │ consult: cells.get(bucket)
                        │                                     │
                seal(digest) ──► artifact.json ──► load+validate
                                                              │
        WASM host? ──yes──► wasm consult                      │
              │no                                             │
        table covers bucket? ──yes──► Learned{table_sha256} ◄┘
              │no
        native DATA rules (fail-open)
```

Precedence rationale: the WASM host is the explicit versioned production
artifact (ADR-074); the learned table is the candidate under measurement;
native is the floor. The table is attached only through
`NativeFallbackPolicy::with_learned_table`, which validates before attaching.

## 4. The observation record

```rust
PolicyObservation {
    document_identity,          // split key; never the prompt
    decision_point,             // GrammarRoute | BatchAdmission | ProfileRoute
    features: ArticleFeatures,  // the condition the policy actually saw
    decision_code,              // closed enum value actually answered
    reward,                     // binary rule first; re-computable
    outcome: PolicyOutcomeEvidence { ok, section_count, body_chars, latency_ms, backend },
}
```

Feature provenance is part of the record's honesty: page count and text-layer
quality derive from the real pre-GROBID classification; `toc_bookmarks` stays
`0` as **typed absence** (the classifier does not measure it) — an absent
count must not invent a monograph signal.

## 5. The coverage constraint (recorded, not solved)

A deterministic policy observed single-arm yields exactly one code per bucket.
A table fitted to that data converges to the native policy — `measured_no_gain`
**by construction**, independent of code quality. Three consequences:

1. The first honest run of the closed loop is expected to report no-gain. That
   is a valid result: it proves the loop measures something real.
2. Gain requires two-arm same-document data (both routes measured on the same
   document). Both arms exist as commands today (`parse-pdf` and
   `parse-pdf-toc`); wiring them into a shadow A/B harness is a separate
   decision with its own receipt requirements.
3. Exploration inside production consults is forbidden (D502): it would
   manufacture coverage by degrading the very decisions under measurement.

## 6. Phasing

| Phase | Scope | Status at recording |
|---|---|---|
| P1 | Table type + trainer + artifact IO + env-gated consult (commits 573df2bb1, c8d8f92f2, 429849bcc) | shipped on this branch, 1416 tests green |
| P2 | F1 real features into consult; F2 observation writer at parse-pdf; F3 binary reward from outcome evidence; F5 `policy learn` command over the journal | in flight; recorded by this ADR before completion |
| P3 | F4 two-arm shadow A/B data source | deferred; separate decision |

## 7. Consequences

- The parse-policy learning loop becomes runnable on real data with an honest
  expected first outcome (no-gain baseline).
- `PolicySource` gains a third variant; all renderers updated (`learned:<sha>`).
- The MicroLoRA trainer keeps its D294 ablation role; nothing it produced is
  invalidated — it was never consultable, which was the diagnosed defect.
- Future richer features (e.g. layout stats) may outgrow coarse bucketing;
  that is the recorded revisability trigger.

## 8. Alternatives considered

- **MicroLoRA as the runtime artifact**: rejected by the function-class
  argument (§LLM Reading Notes); kept as ablation.
- **WASM packaging of the table now**: deferred, not rejected — packaging, not
  function; revisit after a measured gain exists to justify hot-swap.
- **Exploration for coverage**: rejected (§5.3).
- **Extraction F1 as reward**: rejected by ADR-076 (parse oracle is
  conservation honesty, not extraction F1).

## 9. Relations

- ADR-074 (policy modules): this ADR fixes the **artifact form** in the native
  path; WASM packaging and decision-point names unchanged.
- ADR-076 (three automata, freeze): unchanged; the table emits closed enums
  and stamps `policy_hash`; no TEI, no graph, candidate-only.
- D294: training scope stays open; promotion stays explicit.
- D136: unchanged — no LoRA weights enter the registry or the WASM host.
