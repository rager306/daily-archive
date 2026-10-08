// LOCAL ENGINE PATCH (daily-archive / M516-th0oby)
// Makes the local (non-Windows) projection-contention timeout classify as
// `projection-lock-transient` instead of `runtime-unknown`, and stops the idle
// watchdog from cancelling the unit when it hits that contention.
//
// Idempotent: re-running on an already patched tree is a no-op.
import { readFileSync, writeFileSync, copyFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const dirs = process.argv.slice(2);
if (dirs.length === 0) {
  console.error("usage: patch-projection-lock.mjs <extensions-gsd-dir> [...]");
  process.exit(2);
}

const MARK_ROOT = "PROJECTION_CONTENTION_TIMEOUT";
const MARK_GUARD = "LOCAL PATCH (daily-archive)";

let changed = 0;
for (const dir of dirs) {
  const rootPath = join(dir, "projection-root-errors.js");
  const timerPath = join(dir, "auto-timers.js");
  if (!existsSync(rootPath) || !existsSync(timerPath)) {
    console.error(`SKIP ${dir}: expected files not found`);
    process.exitCode = 1;
    continue;
  }

  // ---- 1. projection-root-errors.js: recognize the contention message ----
  let root = readFileSync(rootPath, "utf8");
  if (!root.includes(MARK_ROOT)) {
    const anchor = 'const WINDOWS_SHARING_VIOLATION = /(?:transient projection root sharing violation|projection root operation failed:.*(?:os error 32|sharing violation))/i;';
    if (!root.includes(anchor)) {
      console.error(`SKIP ${rootPath}: anchor not found`);
      process.exitCode = 1;
      continue;
    }
    root = root.replace(
      anchor,
      anchor +
        "\n// LOCAL PATCH (daily-archive): database-maintenance-fence.js throws a plain GSDError for\n" +
        "// local (non-Windows) projection contention. Without this pattern that error classifies as\n" +
        "// runtime-unknown -> escalate, so the engine aborts the unit and pauses auto instead of\n" +
        "// using PROJECTION_LOCK_TRANSIENT_BACKOFF_MS.\n" +
        "const PROJECTION_CONTENTION_TIMEOUT = /projection mutation remains fenced after the contention timeout/i;",
    );
    const testAnchor = "        if (WINDOWS_SHARING_VIOLATION.test(message))\n            return true;";
    if (!root.includes(testAnchor)) {
      console.error(`SKIP ${rootPath}: test anchor not found`);
      process.exitCode = 1;
      continue;
    }
    root = root.replace(
      testAnchor,
      testAnchor + "\n        if (PROJECTION_CONTENTION_TIMEOUT.test(message))\n            return true;",
    );
    copyFileSync(rootPath, `${rootPath}.pre-projection-lock-patch`);
    writeFileSync(rootPath, root);
    console.log(`patched ${rootPath}`);
    changed++;
  } else {
    console.log(`already patched ${rootPath}`);
  }

  // ---- 2. auto-timers.js: do not cancel the unit on transient contention ----
  let timer = readFileSync(timerPath, "utf8");
  if (!timer.includes(MARK_GUARD)) {
    const importAnchor = 'import { logWarning, logError } from "./workflow-logger.js";';
    if (!timer.includes(importAnchor)) {
      console.error(`SKIP ${timerPath}: import anchor not found`);
      process.exitCode = 1;
      continue;
    }
    timer = timer.replace(
      importAnchor,
      importAnchor + '\nimport { isTransientProjectionLockError } from "./projection-root-errors.js";',
    );

    const catchAnchor =
      "            const message = err instanceof Error ? err.message : String(err);\n" +
      '            logError("timer", `[idle-watchdog] Unhandled error: ${message}`);';
    if (!timer.includes(catchAnchor)) {
      console.error(`SKIP ${timerPath}: catch anchor not found`);
      process.exitCode = 1;
      continue;
    }
    const guard =
      "            const message = err instanceof Error ? err.message : String(err);\n" +
      "            // LOCAL PATCH (daily-archive): projection-root contention is a transient by\n" +
      "            // projection-root-errors.js, so it must not cancel the unit here. Cancelling the\n" +
      "            // agent surfaced as failure_class=runtime-unknown + task-recovery-abort and\n" +
      "            // bypassed the projection-lock retry ladder. Skip this tick; the unit hard\n" +
      "            // timeout remains the backstop for a lock that never clears.\n" +
      "            if (isTransientProjectionLockError(err)) {\n" +
      '                logWarning("timer", `[idle-watchdog] Projection contention (transient) - skipping this tick: ${message}`);\n' +
      "                try {\n" +
      "                    ctx.ui.notify(`Projection lock busy (transient) - continuing unit ${unitType} ${unitId}.`, \"warning\");\n" +
      "                }\n" +
      "                catch (notifyErr) { /* best effort */ }\n" +
      "                return;\n" +
      "            }\n" +
      '            logError("timer", `[idle-watchdog] Unhandled error: ${message}`);';
    timer = timer.replace(catchAnchor, guard);
    copyFileSync(timerPath, `${timerPath}.pre-projection-lock-patch`);
    writeFileSync(timerPath, timer);
    console.log(`patched ${timerPath}`);
    changed++;
  } else {
    console.log(`already patched ${timerPath}`);
  }
}
console.log(`files changed: ${changed}`);
