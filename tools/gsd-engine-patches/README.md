# gsd-engine-patches

Local patches for the **installed** `gsd-pi` engine (`@opengsd/gsd-pi` 1.21.0), applied
outside this repository. They exist because a control-plane defect in the engine wedged
milestone M516-th0oby repeatedly. Each patch is idempotent and keeps a
`*.pre-projection-lock-patch` backup next to the file it changes.

**These patches are local machine state, not product code.** They are overwritten by any
`gsd-pi` upgrade or re-copy of the extension tree, so re-apply them after an upgrade:

```bash
node tools/gsd-engine-patches/patch-projection-lock.mjs \
  /usr/lib/node_modules/@opengsd/gsd-pi/dist/resources/extensions/gsd \
  /root/.gsd/agent/extensions/gsd
```

## patch-projection-lock.mjs

**Symptom it removes:** `gsd-db: Projection mutation remains fenced after the contention
timeout` aborted `execute-task` and `complete-slice` units on Linux, paused auto-mode, and
required a manual `gsd_task_recovery_resume` for every occurrence. Observed three times on
M516-th0oby (S05/T03 twice, S05 closeout once) with the identical failure fingerprint
`9e4bb28d64aee918e42e5df849bb5c154bda595df6cc7daf6e0ab90d79cf06d5`, recorded in
`workflow_attempt_results` as `failure_class=runtime-unknown`, `summary=unit-aborted`.

**Root cause (verified in the 1.21.0 source, not inferred):**

1. `database-maintenance-fence.js` throws the contention timeout as a **plain**
   `GSDError(GSD_STALE_STATE, ...)` after `PROJECTION_CLAIM_MAX_ATTEMPTS` (1000) ×
   `PROJECTION_CLAIM_RETRY_MS` (5 ms) — about five seconds of contention.
2. `isTransientProjectionLockError()` (`projection-root-errors.js`) returns true only for a
   `ProjectionLockTransientError` instance or a message matching
   `WINDOWS_SHARING_VIOLATION` = `/(?:transient projection root sharing violation|projection root operation failed:.*(?:os error 32|sharing violation))/i`.
   That is Windows-only wording, so the Linux message never matches, and nothing routes the
   fence throw through `throwIfTransientProjectionLockError`.
3. `inferFailureKind()` therefore falls through to `runtime-unknown`, whose taxonomy action is
   `escalate`. The idle watchdog's catch (`auto-timers.js`) cancels the agent outright, so the
   unit ends as `unit-aborted` and auto pauses with `task-recovery-abort`.
4. The engine already ships the correct remedy for this class — `projection-lock-transient`
   is `action: retry` with `PROJECTION_LOCK_TRANSIENT_BACKOFF_MS = [1000, 2000, 4000, 8000,
   16000, 30000]` and an explicit note in `auto/loop.js` that this class must not consume the
   generic three-error budget. That ladder was unreachable for the exact error it was written
   for.

**What the patch changes:**

- `database-maintenance-fence.js`: `PROJECTION_CLAIM_RETRY_MS` 5 → 25 and
  `PROJECTION_CLAIM_MAX_ATTEMPTS` 1_000 → 2_400, so the claim budget becomes **60 s instead of 5 s**.
  Contention here is transient (measured: the claim frees in milliseconds), so a 5 s budget turned
  ordinary overlap into a fatal projection failure on paths that never consult the transient
  classifier. The one that stopped auto-mode on M516/S06 was the task-completion summary projection:
  `task-completion-compatibility-adapter.js` re-wraps whatever `renderTaskSummary` threw as a plain
  `Error("Task completion summary projection failed: ...")`, and `auto.js` then classifies the stop as
  `failed` because the reason contains "fail". Widening the wait fixes that path without touching any
  control flow.
- `projection-root-errors.js`: adds `PROJECTION_CONTENTION_TIMEOUT =
  /projection mutation remains fenced after the contention timeout/i` and returns `true` for
  it, so the fence error classifies as `projection-lock-transient` and
  `throwIfTransientProjectionLockError` now re-wraps it as a `ProjectionLockTransientError`.
- `auto-timers.js`: in the idle-watchdog catch, a transient projection-lock error is logged,
  notified, and **skipped for that tick** instead of cancelling the unit. The unit hard
  timeout remains the backstop for a lock that never clears.

**Verified after patching** (`node --check` on the patched files plus a behaviour probe):

| probe | before | after |
|-------|--------|-------|
| `isTransientProjectionLockError(fence message)` | `false` | `true` |
| `classifyFailure({error: fence})` | `runtime-unknown` / `escalate` | `projection-lock-transient` / `retry`, backoff `[1000,2000,4000,8000,16000,30000]` |
| `isTransientProjectionLockError(maintenance-intent fence)` | `false` | `false` (unchanged) |
| `isTransientProjectionLockError(provider message)` | `false` | `false` (unchanged) |

**Contention is transient, not a stuck lock.** `claimProjectionMaintenance(dbPath)` acquired
the real claim 6/6 times in 7–29 ms while an unrelated long-lived `gsd` process on the same
database was alive, so the on-disk claim is free between bursts. The engine guidance itself
says of this class: "Retry through the existing transient budget; if contention persists, use
`/gsd doctor` to inspect stale workers and orphaned worktrees."

**Upstream report.** `database-maintenance-fence.js` should throw the contention timeout as a
`ProjectionLockTransientError` (or route it through `throwIfTransientProjectionLockError`)
rather than a bare `GSDError(GSD_STALE_STATE, ...)`, and the idle-watchdog catch should not
cancel the unit for a transient projection-lock failure. Both are one-liners in intent; the
retry machinery already exists.

## Unrelated observation recorded during the same investigation

`workflow_projection_work` holds 6848 rows — 3573 `pending` (oldest 2026-08-03) and 3275
`rendered` — with `rendered_at` never later than `2026-09-20T06:25:20Z` and no row ever
carrying `last_error`. The queue is enqueued by `captureCurrentProjectionWork()` and settled
by `settleProjectionWork()` from `projection-worker.js`; nothing has settled for over two
weeks. This did **not** cause the aborts above (the delivery worker holds no claim between
runs) and is left uninvestigated.

## patch-verification-source-snapshot.mjs

**Symptom it removes:** `postUnitPreVerification timed out after 60s ... pausing auto-mode
for recovery` → `Auto-mode paused (Escape)` on **every** task finalize, paired with
`Safety: task ... claimed passing verification that failed in recorded execution` when the
task's probe used a compound script.

**Root cause (verified in the 1.21.0 source, not inferred):**

1. `verification-source-integrity.js` builds the host verification source set from
   `git ls-files --cached --others --exclude-standard` over `SOURCE_PATHSPEC`, which
   excludes only `.gsd/**` and `receipts/**`.
2. `addSourceEntry()` reads and sha256s **every** listed file's content, and
   `captureVerificationSourceSnapshot()` runs that **twice** (capture + confirm).
3. This repository commits a large corpus: `data/` (~13.2k files), `archive/` (~12.6k),
   `artifacts/` (~2.2k) — ~28k of ~30k tracked files. Two content passes over ~30k files
   exceeds `FINALIZE_PRE_TIMEOUT_MS` (60_000 in `auto/finalize-timeout.js`), the guard
   fires, and auto pauses with the Escape/recovery notice.

**Fix:** (A) exclude the committed corpus and build output from the source pathspec so the
gate still proves code/tests/config are unchanged during verification (30,163 → 2,155
files; `git ls-files` 0.76s); (B) raise `FINALIZE_PRE_TIMEOUT_MS` /
`FINALIZE_POST_TIMEOUT_MS` to 300_000 as a safety net for slower hosts.

Apply alongside the projection-lock patch:

```bash
node tools/gsd-engine-patches/patch-verification-source-snapshot.mjs \
  /usr/lib/node_modules/@opengsd/gsd-pi/dist/resources/extensions/gsd \
  /root/.gsd/agent/extensions/gsd
```
