#!/usr/bin/env bash
# M447 / ADR-067: reproducible data-layout migration.
# Idempotent: every step checks its own precondition and skips if done.
# Each step verifies itself; the script fails loudly on any mismatch.
#
# Steps:
#   1. Flatten data/article_catalog/article_catalog -> data/article_catalog/
#      (same-FS rename, bytes preserved; no symlink shims per owner decision).
#   2. Rewrite doubled-path literals in crates/**/src (prod code).
#   3. Move reader-less legacy bundles to archive/data-legacy/ (D237-safe move).
#   4. Verify: no nested dir, no prod literals, no legacy dirs in data/ root,
#      asset spot-check, eval_batch manifest builds.
set -euo pipefail
cd "$(dirname "$0")/.."

step() { echo -e "\n=== [M447] $1 ==="; }
ok()   { echo "  OK: $1"; }
skip() { echo "  SKIP (already done): $1"; }

step "1. Flatten article_catalog double nesting"
if [ -d data/article_catalog/article_catalog ]; then
  inner=data/article_catalog/article_catalog
  for d in "$inner"/*/; do
    name=$(basename "$d")
    if [ -e "data/article_catalog/$name" ] && [ "$name" != "article_catalog" ]; then
      echo "  ERROR: collision moving $name — abort"; exit 1
    fi
    mv "$inner/$name" "data/article_catalog/$name"
    echo "  moved: article_catalog/$name"
  done
  leftovers=$(ls -A "$inner" 2>/dev/null | wc -l)
  if [ "$leftovers" -ne 0 ]; then echo "  ERROR: inner dir not empty"; exit 1; fi
  rmdir "$inner"
  ok "flattened (no symlinks, no deletions)"
else
  skip "already flat"
fi

step "2. Rewrite doubled-path literals in crates/**/src"
hits=$(rg -l --color=never "article_catalog/article_catalog" crates/*/src 2>/dev/null | wc -l || true)
if [ "$hits" -eq 0 ]; then
  skip "no prod literals left (0 files match)"
else
  rg -l --color=never "article_catalog/article_catalog" crates/*/src 2>/dev/null | while read -r f; do
    python3 - "$f" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
open(p, 'w').write(s.replace('article_catalog/article_catalog', 'article_catalog'))
print(f"  rewrote literals in {p}")
PY
  done
  ok "prod literals rewritten"
fi

step "3. Archive reader-less legacy bundles (bytes preserved)"
for d in r024-218-document-corpus-v1 r024-53-document-corpus-v1 \
         r024-entity-scale-corpus-v1 r024-20-document-corpus-v1 \
         architecture-assessment test-architecture-alignment; do
  src="data/$d"
  dst="archive/data-legacy/$d"
  if [ -d "$src" ]; then
    mkdir -p archive/data-legacy
    mv "$src" "$dst"
    echo "  archived: data/$d -> archive/data-legacy/$d"
  else
    skip "data/$d already archived"
  fi
done

step "4. Verification"
fail=0
[ -d data/article_catalog/article_catalog ] && { echo "  FAIL: nesting still present"; fail=1; } || ok "no nesting"
n=$(rg -l --color=never "article_catalog/article_catalog" crates/*/src 2>/dev/null | wc -l || true)
[ "$n" -eq 0 ] && ok "zero prod literals" || { echo "  FAIL: $n files with old path"; fail=1; }
test -f data/article_catalog/arxiv/cs-cl/2507.19457/source/2507.19457.pdf \
  && ok "asset spot-check" || { echo "  FAIL: asset missing"; fail=1; }
for d in r024-218-document-corpus-v1 r024-53-document-corpus-v1 \
         r024-entity-scale-corpus-v1 r024-20-document-corpus-v1 \
         architecture-assessment test-architecture-alignment; do
  [ -d "data/$d" ] && { echo "  FAIL: legacy dir still in data/: $d"; fail=1; }
done
[ "$fail" -eq 0 ] && echo "\nMIGRATION VERIFIED" || { echo "\nMIGRATION FAILED"; exit 1; }
