# M517-1tplxa: supported free-form closeout path

Read-only audit, 2026-10-13. Inspected the installed engine under `/root/.gsd/agent/extensions/gsd`, current M517 PLAN/ROADMAP projections, and `git show f7a65ae81 -- AGENTS.md` (the MEM567/D144/M306 worked sequence). No engine mutation, DB query, product edit, or cargo command was run by this audit.

## Bottom line

There is no supported current path that makes these three **no-Attempt Tasks completed** merely by supplying external test evidence. The project-sanctioned alternative is **terminal cancellation/waiver closeout**:

1. Finish and verify delivery outside GSD.
2. Save honest external-delivery summary artifacts through the engine.
3. Cancel/skip S01 and S02 through `executeSkipSlice`; this cascades S01/T01 and S02/T01,T02 to canonical `cancelled`, legacy `skipped`, and grants a current Slice waiver.
4. Record real evidence and evaluate actual pending gates with their exact task IDs.
5. Capture the aggregate `sha256:…` source revision; validate against that exact source and external evidence.
6. Inspect closeout consistency, complete the Milestone, checkpoint.

Do **not** call `executeSliceComplete` after skipping. Those Slices are already terminal. Describe the Milestone as delivered/verified externally and sealed with waived GSD execution; do not claim a host-verified Task Attempt occurred.

`executeTaskSettle` is not the missing door: reconcile requires a running/interrupted/succeeded prior Attempt (or receipt-gated verification pause). `blocker-accepted` requires a failed `blocker-discovered` Result at a route head. `recordTaskTechnicalVerdict` also requires an existing Attempt scope.

## Exact engine setup and calls

These are examples for the parent to run, **not executed by this audit**. Use the project root as `base`, not its `.gsd` directory. Do not dispatch auto execution or use raw SQL.

```js
const root = '/root/.gsd/agent/extensions/gsd';
const base = '/root/daily-archive';
const mid = 'M517-1tplxa';
const dbw = await import(`${root}/db-workspace.js`);
const db = await import(`${root}/gsd-db.js`);
const wf = await import(`${root}/tools/workflow-tool-executors.js`);
const { internalExecutionInvocation } = await import(`${root}/execution-invocation.js`);
const { loadEffectiveGSDPreferences } = await import(`${root}/preferences.js`);
const { captureMilestoneVerificationSourceRevision } =
  await import(`${root}/verification-source-integrity.js`);
const opened = dbw.openExistingWorkflowDatabase(base);
if (!opened.ok) throw new Error(`workflow DB open failed: ${opened.reason}`);
// Stable key per logical operation. Reuse it only for an exact retry.
const invocation = (name) => internalExecutionInvocation(`manual:${mid}:${name}`);
const assertResult = (r) => {
  if (r.isError || r.details?.superseded || r.details?.current === false)
    throw new Error(JSON.stringify(r));
  return r;
};
// Typed read API, not SQL: inspect planned verification classes and real gates.
console.log(JSON.stringify({
  milestone: db.getMilestone(mid),
  slices: db.getMilestoneSlices(mid).map(s => ({
    slice: s, tasks: db.getSliceTasks(mid, s.id),
    gates: db.getGateResults(mid, s.id)
  }))
}));
```

### 1. Keep the external-delivery record honest

Use `executeSummarySave`, which records artifacts and writes their projections; it does not make Tasks complete. Save one note for each of S01/T01, S02/T01,T02 and a Slice delivery note for S01/S02. Example:

```js
assertResult(await wf.executeSummarySave({
  milestone_id: mid, slice_id: 'S01', task_id: 'T01',
  artifact_type: 'SUMMARY',
  content: deliveryMarkdown // real files, commits, commands, results, log/hash refs
}, base));
// Slice note: same shape, omit task_id.
```

State the actual delivery, verification coverage, limitations, and cancellation waiver disposition. Never label external logs as a canonical Task Technical Verdict. Do not use summary saving as a substitute for a structured UAT result when claiming a GSD run-uat assessment.

### 2. Waive GSD execution with the sanctioned cancellation operation

Only after the relevant delivery and external checks really exist:

```js
for (const sid of ['S01', 'S02']) {
  assertResult(await wf.executeSkipSlice({
    milestoneId: mid, sliceId: sid,
    reason: deliveredReasonBySlice[sid]
    // Include actual delivery commit/hash refs, evidence refs, and the fact
    // that free-form delivery bypassed GSD task execution.
  }, base, invocation(`skip:${sid}`)));
}
```

`internalExecutionInvocation` defaults to an agent identity. Do not impersonate a user or invent user approval. This call creates the policy-authorized slice waiver that the project MEM567/M306 closeout path uses.

### 3. Record verification and evaluate gates

For a formal GSD UAT assessment, use `tools/exec-tool.js::executeUatExec(params, deps)` for each check, then `wf.executeUatResultSave(params, base)` once per Slice. `executeUatExec` writes typed `.gsd/exec` metadata; no Task Attempt is needed. Its deps are `{baseDir: base, preferences: loadEffectiveGSDPreferences(base)?.preferences ?? null}` as used by `bootstrap/exec-tools.js` (its `loadContextModePreferences` helper is private, not an importable export).

UAT check capture params:

```js
{ milestoneId: mid, sliceId: 'S02', checkId: 'UAT-01',
  intent: 'uat-runtime-check', runtime: 'node', script: actualCheck,
  expected: actualExpectation, timeout_ms: 600000 }
```

Use `uat-artifact-check` when only checking retained artifacts. A JSON/log artifact check proves those artifacts, not a newly executed parse/learning cycle. Retain and reference the real earlier capture logs/hashes separately. Treat `timed_out`, `aborted`, or any signal as failure even if an exit field says zero.

Formal UAT result shape:

```js
{ milestoneId: mid, sliceId: 'S02', uatType: 'runtime-executable',
  verdict: 'PASS', attempt: 'auto', notes: actualRationale,
  checks: [{id: 'UAT-01', description: actualDescription, mode: 'runtime',
    result: 'PASS', evidence: [{kind:'gsd_uat_exec', ref: returnedExecId}]}],
  presentation: actualPresentation }
```

Evidence is an **array**. At least one check must cite a fresh typed `gsd_uat_exec` ID. The assessment rejects PASS when that metadata shows nonzero exit, timeout, signal, or abort. For a real run-uat tool presentation the required names are `gsd_uat_exec`, `gsd_uat_result_save`, `gsd_resume`, `gsd_milestone_status`, `gsd_journal_query`; block `gsd_exec`, `gsd_summary_save`, `gsd_save_gate_result` with reasons. The engine can supply its canonical default presentation if omitted, but that is not evidence that the free-form harness actually presented those tools: **do not fabricate presentation history**. If that surface was not available, prefer honest external command/artifact evidence in milestone validation instead of claiming a canonical tool presentation. M517 is backend-only and its observed PLAN/ROADMAP text does not require browser evidence.

For each actual pending gate, evaluate its real question. Call the typed engine writer, using the exact row identity:

```js
// `evaluatedGates` is authored from real review/evidence, never blanket-pass.
for (const g of evaluatedGates) db.saveGateResult({
  milestoneId: mid, sliceId: g.slice_id, gateId: g.gate_id,
  taskId: g.task_id ?? '', // Q5/Q6/Q7: T01/T02; slice gates: ''
  verdict: g.verdict, rationale: g.rationale, findings: g.findings ?? ''
});
console.log(JSON.stringify(['S01','S02'].map(sid => ({
  sid, pending: db.getPendingGates(mid, sid)
}))));
```

Q3 abuse; Q4 affected requirements; Q5 dependency failures; Q6 10x load; Q7 specific negative tests; Q8 operational health/recovery. Valid gate dispositions are `pass`, `flag`, `omitted`. Use `omitted` only with a real not-applicable rationale. Do not declare readiness with a substantive flagged finding just because a row is no longer pending. Save UAT via structured result when claiming a run-uat assessment; the raw gate writer can also close a UAT row from honest external UAT evidence, but is not equivalent to a GSD UAT assessment.

Current `executeSaveGateResult` is also usable: unlike the old worked-sequence version, it commits the gate first and catches skipped-PLAN rendering failure as `details.stale`. Direct `saveGateResult` remains the smaller sanctioned writer. It throws if the exact seeded gate row does not exist. Never substitute an empty task ID for a task-scoped gate.

### 4. Capture source and validate

Run all covered edits/artifact regeneration before source capture. Ideally capture before and after verification, and require equality. If binding older test logs to current source, demonstrate that they tested the same source bytes; do not copy a new hash over stale evidence.

```js
const prefs = loadEffectiveGSDPreferences(base)?.preferences;
const source = captureMilestoneVerificationSourceRevision(base, prefs);
if (!source.ok) throw new Error(source.error);
const testedSourceRevision = source.sourceRevision; // sha256:…, not git SHA
assertResult(await wf.executeValidateMilestone({
  milestoneId: mid, verdict: 'pass', remediationRound: 0,
  successCriteriaChecklist: actualCriteriaEvidence,
  sliceDeliveryAudit: honestExternalDeliveryAndWaiverAudit,
  crossSliceIntegration: actualPairedEndToEndEvidence,
  requirementCoverage: actualRequirementEvidence,
  verificationClasses: completeApplicableClassTable,
  verificationEvidence: actualEvidence.map(e => ({...e, testedSourceRevision})),
  verdictRationale: actualRationale
}, base, {invocation: invocation('validate:v1')}));
```

**Signature gotcha:** validation's third argument is `{invocation}`, not the invocation itself. Each evidence row must contain:

```js
{ verificationClass: 'Contract', // or Integration / Operational / UAT
  evidenceClass: 'command', // command / runtime / browser / artifact
  rationale: actualRationale, commandOrTool: actualCommand,
  workingDirectory: actualCwd, startedAt: actualISOStart, endedAt: actualISOEnd,
  exitCode: actualExit, observation: 'passed', durableOutputRef: actualRetainedRef,
  testedSourceRevision, environment: nonemptyActualEnvironment,
  // optional sliceId, e.g. 'S02'
}
```

Inspect `getMilestone(mid).verification_contract/integration/operational/uat`; every applicable planned class requires structured evidence plus its exact canonical name in the class table. One class must use only one `evidenceClass`, even if it has multiple entries. `N/A`/`none`/`not required` plans are not applicable. Use `needs-attention`/`needs-remediation`, not `pass`, for genuine gaps. Engine validation records a source-bound Milestone validation Attempt; it does not invent Task execution history.

Do cancellation/lifecycle changes, subjective criteria (if genuinely needed), and UAT recording **before** the final validation. Descendant lifecycle changes after validation stale the authorization. Source changes after validation also invalidate it. For a revalidation use a **new** invocation key, not an exact replay of `validate:v1`. No `skipBrowserEvidenceGate`, fabricated subjective user answer, or broad waiver is needed for these backend plans.

### 5. Inspect and seal

```js
const { checkCloseoutConsistencyGate } = await import(`${root}/closeout-consistency-gate.js`);
const check = checkCloseoutConsistencyGate(mid, {
  allowOpenMilestone: true, artifactBasePath: base, readOnly: true
});
if (!check.ok) throw new Error(JSON.stringify(check));
assertResult(await wf.executeCompleteMilestone({
  milestoneId: mid, title: actualTitle, oneLiner: actualSummary,
  narrative: honestDeliveryAndWaiverNarrative, verificationPassed: true,
  successCriteriaResults: actualCriteriaEvidence,
  definitionOfDoneResults: actualDoDEvidence,
  deviations: 'Delivery and verification were performed in a free-form session; GSD Task/Slice execution was cancelled with current slice waivers.'
}, base, invocation('complete:v1')));
dbw.checkpointWorkflowDatabase();
```

Complete-Milestone rechecks current source-bound validation and current cancellation authorization. `readOnly:true` prevents the consistency inspection from repairing bookkeeping. Keep pending gates empty: the consistency helper now ignores unreachable skipped-task gates, but `completeMilestone` still inspects their closure and can reject pending rows. A clean preview alone is not a substitute for inspecting all pending gate rows.

## Sources and boundaries

- `git show f7a65ae81 -- AGENTS.md`: canonical manual closeout (M306-417kcl), MEM567/D144; old paths/presentation instructions need current-version checks.
- `tools/workflow-tool-executors.js:699,1023,1265,1549,1605,1774,1851`: public executor signatures and private identity requirements.
- `task-completion-compatibility-adapter.js:100`: no running/replay-matched Attempt prevents canonical Task completion.
- `task-settle.js:337,432,607`: reconciliation/publication/blocker-accepted are evidence-backed Attempt recovery, not arbitrary external completion.
- `slice-lifecycle-domain-operation.js:61`: cancellation + Slice waiver, canonical cancelled/legacy skipped, preserves completed work.
- `db/writers/slice-lifecycle.js:485`: completion requires current passing Task proof or authorized cancellation and refuses terminal Slices.
- `tools/validate-milestone.js:120,268`: required classes, exact source binding, canonical validation.
- `bootstrap/db-tools.js:1407`: complete structured verification evidence schema.
- `verification-source-integrity.js:329`: aggregate source capture; normal root is repository base.
- `quality-gate-closure.js:132`, `milestone-lifecycle-domain-operation.js:328`, `closeout-consistency-gate.js:181`: preview/complete checks differ for skipped-task pending gates.
- `gsd-db.js:1078`: atomic gate update + ledger insertion through typed engine wrapper; no raw SQL needed.
- `uat-run.js:229,265,274,393`; `unit-registry.js:28`: UAT evidence/presentation checks.

This audit makes no claim that M517 delivery, the 320-PDF capture, three cycles, quality proxies, or final verifications have passed; those are the parent's work and must drive the actual dispositions. No constants or policies were changed.
