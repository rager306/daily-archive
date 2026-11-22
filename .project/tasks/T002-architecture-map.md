---
id: T002
title: Draft and commit doc/review/architecture-map.md (every claim cites a code path at HEAD)
wave: 1
deps: [T001]
status: pending
agent: null
base: null
worktree: null
task_branch: null
files:
  - doc/review/architecture-map.md
---

# T002 — Draft and commit doc/review/architecture-map.md

## Context

M001's architecture report: a code-verified single view of the daily-archive architecture for the owner and downstream Path phases. SC1: every architecture/dependency claim cites a concrete path in crates/ or Cargo.toml; no claim line without a path citation, checked over the whole file. Inputs settled by SYNTHESIS: reuse the E1 citation-enumeration Verify pattern proven in T001 (recorded in T001's Log — the dependency's landed effect); the crate set is 12 crates (kg-{ontology,storage,embeddings,algorithms,pipeline} + da-{domain,ports,application,parse,adapters,cli,mcp}); vendor path deps bind builds to /root/vendor-source; GROBID 0.9.1 Java is the production parse route. Evidence budget: static reading + codebase-memory-mcp full toolset + recorded logs; no new cargo/test runs.

## Approach

- N4 (hard limit): the map must not restate historical doc claims as current code facts. Proposed ADRs (042–049) and removed crates (da-graph, legacy/) may appear only explicitly marked as proposed/removed; every implementation claim cites a code path at HEAD; doc-only claims are labeled documentary. Known stale traps from research: AGENTS.md/ADR-INDEX reference a `legacy/` tree that no longer exists; "~86 examples"-style historical counts are stale — render current facts from code, not ledgers.
- Absence claims (e.g. "legacy/ отсутствует") must cite the checked location (E2) — the path that was searched, and the result that counts as absent.
- Live-test boundary: place it at the gate script's provisioning list (`scripts/governor/__main__.py:784-800` — seven-name da-cli example list; `*_real_model.rs` suites in `crates/da-adapters/tests/`), not at crates/da-parse. The 146 da-parse integration suites are service-free and feature-gate-free; wherever suite execution is cited, state explicitly that GitHub CI runs only `cargo test -p da-parse --lib` and the local governor gate (`/tmp/arch-20261123-pass2.log`, 23/23 PASS) is the only recorded execution plane (owner ruling U1 — CI untouched; no CI edit anywhere).
- Proof-weight honesty: any "runs/passes" claim cites a recorded run or carries static-only/unverified.
- N1/N3: this task edits only doc/review/architecture-map.md — no code or config change.

## Interface contract

- None (own file only; consumes T001's proven Verify pattern from its Log, no shared file).

## Intent coverage

- SC1

## Acceptance criteria

1. Every claim line in doc/review/architecture-map.md carries a path citation into crates/ or Cargo.toml (or another repo path), and every cited path resolves at HEAD — the Verify command below exits 0 (E1, Verify-side per Correction C2).
2. Every absence claim cites the checked location (E2).
3. Occurrences of `da-graph` and `legacy/` appear only in sentences explicitly marking removed/proposed/documentary status (N4).
4. The service-dependent-suite boundary sentence names the gate script provisioning list, and every suite-execution mention states the CI `--lib`-only gap (U1).

## Verify

```bash
test -f doc/review/architecture-map.md || { echo "missing doc/review/architecture-map.md"; exit 1; }
python3 - <<'EOF'
import re, subprocess, sys
text = open('doc/review/architecture-map.md', encoding='utf-8').read()
ok = True
if len(text) < 2000:
    print('STUB REPORT'); ok = False
paths = set(re.findall(r'`((?:crates|artifacts|data|scripts|doc|\.github|Cargo\.[a-z]+|\.env\.example)/[^`]+)`', text)) | {'Cargo.toml'}
missing = [p for p in paths if p != 'Cargo.toml' and subprocess.run(['git','cat-file','-e','HEAD:'+p], capture_output=True).returncode != 0]
if missing:
    print('DEAD CITATIONS:', missing); ok = False
# claim lines must cite a path: a non-heading, non-list-marker line with no backticked path
claimless = [l for l in text.splitlines()
             if l.strip() and not l.startswith('#') and not l.startswith('|')
             and '/' not in l and '`' not in l and len(l.strip()) > 40]
if claimless:
    print('UNCITED CLAIM LINES:'); [print(' ', l[:100]) for l in claimless[:5]]; ok = False
for kw in ['da-graph', 'legacy/']:
    for m in re.finditer(re.escape(kw), text):
        ctx = text[max(0, m.start()-120):m.start()+120]
        if not re.search(r'proposed|removed|удал[её]н|предложен|documentary|more no longer exists|not on the runtime path|frozen', ctx, re.I):
            print('UNMARKED', kw, 'near:', text[m.start()-40:m.start()+40].replace('\n',' ')); ok = False
if '__main__.py' not in text:
    print('LIVE-TEST BOUNDARY NOT AT GATE SCRIPT'); ok = False
if '--lib' not in text:
    print('CI --lib GAP NOT STATED'); ok = False
sys.exit(0 if ok else 1)
EOF
```
<!-- Fails when the task is skipped (file absent), when any cited path is dead at HEAD (E1), when a long claim line lacks any path citation, when da-graph/legacy/ appear unmarked (N4), or when the gate-script boundary / CI gap is missing. Names doc/review/architecture-map.md from files. -->

Heavy: no

## Log

- 2026-11-22 — created by planner
