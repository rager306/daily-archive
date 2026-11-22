---
id: T003
title: Draft and commit doc/review/functionality-ledger.md (closed enumeration, verdict vocabulary + proof-weight tags)
wave: 1
deps: [T001]
status: pending
agent: null
base: null
worktree: null
task_branch: null
files:
  - doc/review/functionality-ledger.md
---

# T003 — Draft and commit doc/review/functionality-ledger.md

## Context

M001's functionality ledger: closed enumeration of CLI commands × application use-cases with verdicts. SC2: every subcommand from the dispatch match in `crates/da-cli/src/main.rs` plus every use-case in crates/da-application, each with one verdict from exactly {работает, кандидат, заглушка, unverified} and an evidence path; no dispatch entry omitted (E4: closed over the dispatch match in main.rs, help-only/hidden subcommands included — not over README). Reuses the E1 citation-enumeration Verify pattern proven in T001 (recorded in T001's Log — the dependency's landed effect). Evidence budget: static reading + codebase-memory-mcp full toolset + recorded logs; no new cargo/test runs.

## Approach

- Enumeration method (settled): read the dispatch match in `crates/da-cli/src/main.rs` directly (all arms, including help-only/hidden); pair each subcommand with its da-application use-case(s); a command with no use-case row still gets a ledger row. Render counts from code, never from README or historical ledgers.
- E3 (Verify-side per Correction C2): the verdict vocabulary is exactly {работает, кандидат, заглушка, unverified}; one verdict per command. The Verify below greps the finished ledger for out-of-vocabulary verdicts.
- N5 (hard limit): a verdict must not silently become «работает» without execution evidence — every «работает» cites ran-in-gate or recorded-run evidence (e.g. `/tmp/arch-20261123-pass2.log` or another recorded run); without it, downgrade to кандидат/заглушка/unverified.
- Every entry carries a proof-weight tag (ran-in-gate / static-only / unverified) and an evidence path that resolves at HEAD.
- Where suite execution is cited, state explicitly that GitHub CI runs only `cargo test -p da-parse --lib` and the local governor gate is the only recorded execution plane (owner ruling U1); never claim a suite is CI-protected.
- N1/N3: this task edits only doc/review/functionality-ledger.md — no code or config change.

## Interface contract

- None (own file only; consumes T001's proven Verify pattern from its Log, no shared file).

## Intent coverage

- SC2

## Acceptance criteria

1. The ledger enumerates every dispatch arm from `crates/da-cli/src/main.rs` (help-only/hidden included) plus the da-application use-cases — no dispatch entry omitted (E4); a reviewer diffing the ledger's command list against the match arms finds no gap.
2. Every verdict is exactly one of {работает, кандидат, заглушка, unverified}, one verdict per command — the Verify command below exits 0 (E3, Verify-side per Correction C2).
3. Every «работает» row cites a recorded-run/gate-log evidence path (N5); every row carries a proof-weight tag and a path citation that resolves at HEAD.
4. Suite-execution mentions state the CI `--lib`-only gap and the local gate as the execution plane of record (U1).

## Verify

```bash
test -f doc/review/functionality-ledger.md || { echo "missing doc/review/functionality-ledger.md"; exit 1; }
python3 - <<'EOF'
import re, subprocess, sys
text = open('doc/review/functionality-ledger.md', encoding='utf-8').read()
ok = True
if len(text) < 2000:
    print('STUB REPORT'); ok = False
vocab = {'работает', 'кандидат', 'заглушка', 'unverified'}
rows = [l for l in text.splitlines() if l.strip().startswith('|') and '---' not in l]
verdict_cells = 0
for l in rows:
    for cell in l.strip().strip('|').split('|'):
        c = cell.strip()
        if c in vocab:
            verdict_cells += 1
        elif c and re.match(r'^(sha|static|ran|непровер|не работает|works|stub|candidate)', c, re.I) and len(c) < 20 and not re.search(r'[/`(\[]', c):
            print('OUT-OF-VOCAB VERDICT?:', repr(c), 'in row:', l[:80]); ok = False
if verdict_cells == 0:
    print('NO VERDICT CELLS FOUND'); ok = False
if verdict_cells < 5:
    print('SUSPICIOUSLY FEW VERDICTS:', verdict_cells); ok = False
paths = set(re.findall(r'`((?:crates|artifacts|data|scripts|doc)/[^`]+)`', text))
missing = [p for p in paths if subprocess.run(['git','cat-file','-e','HEAD:'+p], capture_output=True).returncode != 0]
if missing:
    print('DEAD CITATIONS:', missing); ok = False
if 'работает' in text and 'arch-20261123-pass2' not in text and 'recorded' not in text:
    print('«работает» WITHOUT RECORDED-RUN CITATION'); ok = False
for tag in ['ran-in-gate', 'static-only', 'unverified']:
    if tag not in text:
        print('PROOF-WEIGHT TAG MISSING:', tag); ok = False
if 'crates/da-cli/src/main.rs' not in text:
    print('DISPATCH SOURCE NOT CITED'); ok = False
if '--lib' not in text:
    print('CI --lib GAP NOT STATED'); ok = False
sys.exit(0 if ok else 1)
EOF
```
<!-- Fails when the task is skipped (file absent), when verdicts fall outside the closed vocabulary (E3), when proof-weight tags, the dispatch source citation, or the CI gap statement are missing, or when cited evidence paths are dead at HEAD. Names doc/review/functionality-ledger.md from files. -->

Heavy: no

## Log

- 2026-11-22 — created by planner
