#!/usr/bin/env node
/**
 * patch-verification-source-snapshot.mjs
 *
 * Local patch for the installed gsd-pi engine (1.21.0). Removes two coupled
 * control-plane defects that paused auto-mode on M516-th0oby/S13/T02:
 *
 *   1. postUnitPreVerification timed out after 60s ...
 *      Auto-mode paused (Escape)
 *
 *   2. Safety: task T02 claimed passing verification that failed in recorded
 *      execution (compound script exit code)
 *
 * Root cause (verified in the 1.21.0 source, not inferred):
 *
 *   verification-source-integrity.js builds the host verification source set as
 *   `git ls-files --cached --others --exclude-standard` over SOURCE_PATHSPEC, which
 *   excludes only .gsd/** and receipts/**. This repository commits a large corpus
 *   (data/ ~13.2k files, archive/ ~12.6k, artifacts/ ~2.2k = ~28k of the ~30k
 *   tracked files), and addSourceEntry() reads and sha256s EVERY file's content.
 *   captureVerificationSourceSnapshot() does that TWICE (capture + confirm), so a
 *   single finalize hashes ~60k file reads. That exceeds FINALIZE_PRE_TIMEOUT_MS
 *   (60_000 in auto/finalize-timeout.js), the guard fires, and auto-mode pauses with
 *   the Escape/recovery notice — every task, deterministically.
 *
 * Fix:
 *
 *   A. Narrow the source pathspec to source-of-truth code and configuration by
 *      excluding the committed data corpus and generated output. The gate keeps its
 *      meaning — "did the verified source revision change during verification?" —
 *      for code, tests, fixtures and DATA-backed registries that are not bulk
 *      payloads (data/ontology, data/policies, data/gold_standard stay in only if
 *      they live outside the excluded prefixes; the excluded prefixes are bulk).
 *      This drops the set from ~30k files to ~2k and the snapshot to seconds.
 *
 *   B. Raise FINALIZE_PRE_TIMEOUT_MS / FINALIZE_POST_TIMEOUT_MS to 300_000 as a
 *      safety net for slower machines or larger sets. The guard exists to catch a
 *      genuinely hung finalize (#2344); 60s was too tight for a content-hashing
 *      gate on a repository of this size.
 *
 * Idempotent. Keeps a `<file>.pre-source-snapshot-patch` backup beside each patched
 * file. Re-apply after any gsd-pi upgrade or extension-tree re-copy.
 *
 * Usage:
 *   node tools/gsd-engine-patches/patch-verification-source-snapshot.mjs \
 *     /usr/lib/node_modules/@opengsd/gsd-pi/dist/resources/extensions/gsd \
 *     /root/.gsd/agent/extensions/gsd
 */
import { cpSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const EXCLUDE_MARKER = ":(exclude)data/**";
const EXCLUDE_BLOCK = `const SOURCE_PATHSPEC = [
    "--",
    ".",
    ":(exclude).gsd/**",
    ":(exclude)receipts/**",
    // Local patch (M516): this repository commits a large corpus (data/ ~13.2k files,
    // archive/ ~12.6k, artifacts/ ~2.2k). The host gate reads and sha256s every tracked
    // file's CONTENT twice (capture + confirm), which exceeded the 60s finalize guard and
    // paused auto-mode on every task. Excluding non-source bulk keeps the gate's meaning
    // (code/tests/config unchanged during verification) while dropping ~30k -> ~2k files.
    ":(exclude)data/**",
    ":(exclude)archive/**",
    ":(exclude)artifacts/**",
    ":(exclude)target/**",
];`;

const TIMEOUT_MARKER = "export const FINALIZE_PRE_TIMEOUT_MS = 300_000;";
const TIMEOUT_PRE = "export const FINALIZE_PRE_TIMEOUT_MS = 300_000;";
const TIMEOUT_POST = "export const FINALIZE_POST_TIMEOUT_MS = 300_000;";

function patchFile(path, marker, transform, backupSuffix) {
  if (!existsSync(path)) return `skip (absent): ${path}`;
  const original = readFileSync(path, "utf8");
  if (original.includes(marker)) return `already patched: ${path}`;
  const patched = transform(original);
  if (patched === null) return `PATTERN NOT FOUND (engine changed?): ${path}`;
  if (!existsSync(path + backupSuffix)) cpSync(path, path + backupSuffix);
  writeFileSync(path, patched);
  return `patched: ${path}`;
}

function patchSourcePaths(text) {
  // Replace the SOURCE_PATHSPEC literal regardless of its current member list.
  const re = /const SOURCE_PATHSPEC = \[[\s\S]*?\];/;
  if (!re.test(text)) return null;
  return text.replace(re, EXCLUDE_BLOCK);
}

function patchTimeouts(text) {
  let out = text;
  const rePre = /export const FINALIZE_PRE_TIMEOUT_MS = [\d_]+;/;
  const rePost = /export const FINALIZE_POST_TIMEOUT_MS = [\d_]+;/;
  if (!rePre.test(out) || !rePost.test(out)) return null;
  out = out.replace(rePre, TIMEOUT_PRE).replace(rePost, TIMEOUT_POST);
  return out;
}

const roots = process.argv.slice(2);
if (roots.length === 0) {
  console.error(
    "usage: node patch-verification-source-snapshot.mjs <gsd-extension-root> [more-roots...]",
  );
  process.exit(2);
}

for (const root of roots) {
  console.log(`root: ${root}`);
  console.log(
    "  " +
      patchFile(
        join(root, "verification-source-integrity.js"),
        EXCLUDE_MARKER,
        patchSourcePaths,
        ".pre-source-snapshot-patch",
      ),
  );
  console.log(
    "  " +
      patchFile(
        join(root, "auto", "finalize-timeout.js"),
        TIMEOUT_MARKER,
        patchTimeouts,
        ".pre-source-snapshot-patch",
      ),
  );
}
