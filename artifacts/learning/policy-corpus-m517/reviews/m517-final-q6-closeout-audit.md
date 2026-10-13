# M517-1tplxa: final Q6 and free-form closeout audit

Read-only audit, 2026-10-13T12:09:09.449832+00:00. Installed engine: `/root/.gsd/agent/extensions/gsd`. No gate/status operation, live DB opening, SQL, cargo, provider/config edit, or repository write was performed. Only this outside-repository report was written. No constants changed.

## Conclusion

**Supported, conditional engine path:** retain all three Q6 verdicts as `flag`; cancel the two GSD Slice executions through SkipSlice with current cancellation waivers; record a passing, current source-bound Milestone validation; then complete the Milestone. Completion need not fabricate completed Tasks or Task Attempts.

**Acceptance judgment, not an engine guarantee:** this can honestly close the *bounded research delivery* because the recorded success criteria require corrected claims, paired fixed-holdout evaluation, three expanding cycles, retained hashes and locks—not overall quality gain or tenfold capacity. Negative results can satisfy that delivery. A passing validation must explicitly retain “no overall quality gain”, adaptive diagnostic validation, candidate-only scope, and the unmeasured Q6 limits. If a promised criterion or any planned verification class remains unmet, use `needs-attention`/`needs-remediation`; do not use this path to hide it.

This audit does **not** establish present closeout authorization. Parent-owned architecture, disk-floor and current-source evidence remain prerequisites. The retained readout shows the pre-closeout active/pending state; it is not a fresh live-state read.

## Why Q6 can remain flagged

- `gsd_save_gate_result` accepts `pass`, `flag`, `omitted`. Q6 asks **“What is the 10x load breakpoint?”** and is task-scoped. A runtime load dimension exists, so lack of measurement is a `flag`, not `omitted`.
- `gsd-db.js::saveGateResult` makes the exact row `status=complete` regardless of verdict. For `flag` it also records `gate_runs.outcome=fail`, `failureClass=verification`, with the real findings. Thus “evaluated” does not mean “passed”. The failing ledger evidence stays visible.
- `quality-gate-closure.js::planQualityGateClosure` skips already-complete non-validation-owned gates. Completed Q6 flags are neither rejected nor rewritten by this closure path. Milestone completion checks *unresolved/pending gates* and canonical validation—not “every gate verdict must be pass”.
- **Save all flags explicitly before completion.** `handleCompleteMilestone` first auto-closes pending gates from artifact sections. Any populated `Load Profile` can be inferred as `pass` without understanding its limitation text. Leaving Q6 pending and relying on cancellation or summary prose risks a false pass. The read-only consistency preview also ignores task gates belonging to cancelled Tasks; that preview alone is not proof that the completion path will preserve pending Q6.

Exact Q6 identities:

| milestoneId | sliceId | gateId | taskId | verdict |
|---|---|---|---|---|
| M517-1tplxa | S01 | Q6 | T01 | flag |
| M517-1tplxa | S02 | Q6 | T01 | flag |
| M517-1tplxa | S02 | Q6 | T02 | flag |

Use the actual 15 seeded rows (6 S01 + 9 S02), not a blanket disposition or invented rows. Gate input is `{milestoneId,sliceId,gateId,taskId,verdict,rationale,findings}`; use `taskId: ""` only for the actual slice-scoped identities. The draft contains review metadata: send only writer fields. `saveGateResult` throws if the exact row is absent.

## Supported lifecycle and caller

`executeSummarySave` records external-delivery artifacts without completing Tasks. `executeSkipSlice(params, base, invocation)` calls canonical `cancelSlice`: canonical `cancelled`, legacy `skipped`, preserving existing completed work, and grants one policy Slice waiver when invoked as an agent. `completeMilestoneHierarchy` accepts terminal cancelled Slices with exactly one current cancellation waiver and cancelled child Tasks under that Slice waiver. It stores `cancelledSliceIds`, `cancelledTaskIds`, and waiver IDs separately from completed work. Do **not** follow SkipSlice with SliceComplete or TaskComplete.

A Slice cancellation waiver authorizes cancelled execution/dependency satisfaction. It does **not** authorize capacity success, production adoption, or a validation pass. No separate Q6 waiver is required by this direct engine path. A source-bound Milestone-validation Attempt is legitimate validation bookkeeping; it is not a Task Attempt or proof of host-dispatched Task execution.

The prepared `/tmp/policy-resume-audit/m517-closeout-engine.mjs` uses supported installed exports. Its Node syntax check passed, and a read-only import probe confirmed all used executor exports exist. Its input switch is:

| operation | Input fields | Actual call |
|---|---|---|
| summaries-and-cancellation | `summaries: SummarySaveParams[]`, `slices: {id,reason}[]` | summary executor, then skip executor with `manual:M517-1tplxa:skip:<sid>:external-v1` |
| gates | `gates: GateResultParams[]` | `db.saveGateResult(entry)`; prints pending rows |
| validate | `invocationKey`, `params: ValidateMilestoneParams` | `executeValidateMilestone(params, base, {invocation: inv(key)})` |
| complete | `invocationKey`, `params: CompleteMilestoneParams` | read-only consistency preview, then `executeCompleteMilestone(params, base, inv(key))` |

**Exact launcher caveat:** the script reads `process.argv[1]`. Plain `node m517-closeout-engine.mjs INPUT.json` instead makes it read its own JavaScript as JSON. This invocation supplies the correct argument without editing the caller:

```sh
node --input-type=module -e 'await import("file:///tmp/policy-resume-audit/m517-closeout-engine.mjs")' /tmp/policy-resume-audit/INPUT.json
```

`node -e` argv behavior was checked separately without invoking this mutating caller. Use an existing operation-specific JSON file, not the placeholder. The caller has no JSON-schema or milestone-identity validation: ensure every `params.milestoneId`, gate `milestoneId`, and summary `milestone_id` is **M517-1tplxa**. It checkpoints after each operation. Do not invoke it for a read-only preview; even its opening path is writable/startup-maintenance capable.

`internalExecutionInvocation(key)` defaults to `sourceTransport=internal`, `actorType=agent`. Do not change that into invented user authorization. Stable keys are exact-retry identities only; changed evidence/revalidation needs a fresh key. Its result checker rejects `isError`, `superseded`, and `current=false`, but does not reject `stale`; `stale` is a readable-projection repair notice, not proof that no DB write occurred.

## Exact registered validation/completion schema

Verified through the installed engine's `registerDbTools` interface, with registration capture only and no tool execution.

Validation required fields: `milestoneId`, `verdict` (`pass|needs-attention|needs-remediation`), `remediationRound`, `successCriteriaChecklist`, `sliceDeliveryAudit`, `crossSliceIntegration`, `requirementCoverage`, `verdictRationale`. `verificationClasses`, `verificationEvidence`, and `remediationPlan` are schema-optional, but all four M517 planned classes are applicable: **Contract, Integration, Operational, UAT**. Each needs its canonical table row and structured evidence. Prose alone cannot authorize it.

Each evidence row requires:

```js
{ verificationClass: "Contract", // Contract | Integration | Operational | UAT
  evidenceClass: "command",     // command | runtime | browser | artifact
  rationale, commandOrTool, workingDirectory, startedAt, endedAt,
  observation: "passed",       // passed | failed | inconclusive
  durableOutputRef,
  testedSourceRevision,         // exact engine aggregate sha256:<64 lowercase hex>
  environment,                 // nonempty actual object
  // optional: exitCode, sliceId
}
```

All rows in one verification class must use one `evidenceClass`. `testedSourceRevision` must match the source aggregate captured by the validation tool; git SHA and frozen runner/per-file hashes are not substitutes. Preserve frozen measurement recipe identities. Do not relabel old measurements as rebuilt current-source execution. Treat timeout, abort or signal as failure regardless of exit code. Artifact UAT proves retained artifacts, not a new live parse cycle. Current disk safety and the planned isolated live-capture evidence still need honest Operational coverage; neither follows from Q6 status closure.

Completion required fields: `milestoneId`, `title`, `oneLiner`, `narrative`, `verificationPassed` (boolean). Optional fields: `successCriteriaResults`, `definitionOfDoneResults`, `requirementOutcomes`, `keyDecisions[]`, `keyFiles[]`, `lessonsLearned[]`, `followUps`, `deviations`, `actorName`, `triggerReason`. For adopted lifecycles, the engine derives readiness from canonical source-bound validation; setting `verificationPassed:true` cannot override it. Use it only after the scoped delivery checks actually pass.

Order: finish source edits and checks → save honest summaries/assessments and SkipSlice waivers → save exact 15 gate dispositions, preserving Q6 flags → final source-bound validation → inspect authorization/pending rows → complete. Later source drift or descendant lifecycle transitions stale validation; use fresh source/evidence and a fresh validation key. Keep all candidate/graph/import/deployment locks unchanged. No validation-waiver/preferences bypass or fictitious subjective user answer is justified here.

## Evidence boundary

Supported exports/schemas were runtime-inspected via normal Node imports and registration; caller syntax and Node argv were checked. Lifecycle/Q6 consequences were traced in the installed source, not exercised by changing live state. The engine is outside the indexed repository; GitNexus query returned no engine flows (daily-archive index 2026-10-13T11:07:18.025Z; FTS disabled). No claim of complete graph analysis or successful live closeout is made. Auto-mode response to retained failing gate ledger entries was not tested; this report concerns the prepared direct executor path.

Exact installed-source references:

- `bootstrap/db-tools.js:1282–1445,1961–1987`: public skip/gate/validate/complete schemas and semantics.
- `gsd-db.js:1078–1127`; `db/queries.js:1154–1174`: gate completion + fail ledger, pending-read semantics.
- `quality-gate-closure.js:46–86,132–161`: artifact-based inference and preservation of evaluated Q6.
- `tools/complete-milestone.js:101–181`; `milestone-lifecycle-domain-operation.js:328–386`: authorization and pending-gate closure.
- `slice-lifecycle-domain-operation.js:61–126`; `db/writers/milestone-lifecycle.js:238–280,428–540`: supported cancellation waiver and distinct cancelled receipt lists.
- `tools/validate-milestone.js:61–79,118–176,268–310`; `db/milestone-closeout-readiness.js:415–501,647–714`: required classes, source binding, pass/current authorization.
- `task-completion-compatibility-adapter.js:100–159`: no-Attempt completion refusal.

Closeout wording that preserves the result: **“Bounded paired-policy research delivery verified externally and sealed with waived GSD Task/Slice execution. Three cycles completed; no overall quality gain or tenfold capacity acceptance; Q6 findings retained; no deployment/import/promotion/graph writes.”**
