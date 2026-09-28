#!/usr/bin/env bash
# Prove GROBID rollback readiness by starting the rollback release, not by reading a
# record of it (M513-r7dg4z/S01/T03).
#
# WHY A DRILL: `.docker/grobid-release.override.yml` warns that a locally cached image
# is not a durable guarantee — it can disappear under `docker image prune`, and the
# documented rollback command then fails with "No such image". That already happened
# once and went unnoticed because nothing checked it. A rollback target nobody has
# started is an assumption, so this script starts it.
#
# WHAT IT DELIBERATELY DOES NOT DO: it never stops, recreates or reconfigures the
# running production container. The rollback image is started as a separate,
# throwaway container on a spare port and removed afterwards, so a readiness check
# cannot become an outage. The swap procedure itself stays where it is documented,
# in the override file.
#
# Usage: bash .docker/verify_rollback_readiness.sh
# Exit: 0 when the rollback release is retrievable, starts, and reports its version;
#       1 on any failed step, naming which step failed.

set -uo pipefail

ROLLBACK_DIGEST="sha256:24ba90eb1c959f65d812bcdb2cf79c677fa5fd7b95235de616b8bc9fa1317849"
ROLLBACK_TAG="0.9.0-crf"
PRODUCTION_DIGEST="sha256:223957791ac2bbe48609dcc58a689b16b60baeae13a8734ef440ae6bfb38f4cd"
PRODUCTION_TAG="0.9.1-crf"
REPOSITORY="grobid/grobid"

# A spare port so the drill cannot collide with the live service on 8070.
DRILL_PORT="${GROBID_ROLLBACK_DRILL_PORT:-8071}"
CONTAINER="grobid-rollback-drill-$$"
START_TIMEOUT_SECONDS="${GROBID_ROLLBACK_DRILL_TIMEOUT:-240}"
LIVE_URL="${DA_GROBID_URL:-http://127.0.0.1:8070}"

fail() {
  echo "ROLLBACK-READINESS FAILED at: $1" >&2
  exit 1
}

note() { printf '%s\n' "$*"; }

read_version() {
  # GROBID answers /api/version with {"version":"...","revision":"..."}.
  curl -sS -m 10 "$1/api/version" 2>/dev/null |
    sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p'
}

note "== GROBID rollback readiness drill =="
note "rollback   : ${REPOSITORY}:${ROLLBACK_TAG} @ ${ROLLBACK_DIGEST}"
note "production : ${REPOSITORY}:${PRODUCTION_TAG} @ ${PRODUCTION_DIGEST}"

# ── 1. The rollback image must be present, or retrievable by digest ────────────
if docker image inspect "${REPOSITORY}@${ROLLBACK_DIGEST}" >/dev/null 2>&1; then
  note "STEP 1 rollback image present locally"
else
  note "STEP 1 rollback image absent locally; retrieving by digest (not by tag)"
  docker pull "${REPOSITORY}@${ROLLBACK_DIGEST}" >/dev/null 2>&1 ||
    fail "step 1: cannot retrieve ${REPOSITORY}@${ROLLBACK_DIGEST} by digest"
  note "STEP 1 rollback image retrieved by digest"
fi

# ── 2. Start it as a throwaway container on the spare port ────────────────────
cleanup() {
  docker stop "${CONTAINER}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker run -d --rm --name "${CONTAINER}" \
  -e JAVA_OPTS="${GROBID_JAVA_OPTS:--Xmx2g}" \
  -p "127.0.0.1:${DRILL_PORT}:8070" \
  "${REPOSITORY}@${ROLLBACK_DIGEST}" >/dev/null 2>&1 ||
  fail "step 2: rollback container did not start from ${ROLLBACK_DIGEST}"
note "STEP 2 rollback container started on 127.0.0.1:${DRILL_PORT}"

# ── 3. It must answer, and report the rollback release ───────────────────────
observed=""
deadline=$((SECONDS + START_TIMEOUT_SECONDS))
while [ "$SECONDS" -lt "$deadline" ]; do
  if [ "$(docker inspect -f '{{.State.Running}}' "${CONTAINER}" 2>/dev/null)" != "true" ]; then
    fail "step 3: rollback container exited before answering"
  fi
  observed="$(read_version "http://127.0.0.1:${DRILL_PORT}")"
  [ -n "$observed" ] && break
  sleep 3
done
[ -n "$observed" ] || fail "step 3: rollback service did not answer /api/version within ${START_TIMEOUT_SECONDS}s"

# The expected version is the rollback tag's version component, read from the tag
# rather than hardcoded again, so renaming the tag cannot leave a stale literal.
expected_version="${ROLLBACK_TAG%%-*}"
[ "$observed" = "$expected_version" ] ||
  fail "step 3: rollback reported version=${observed}, expected ${expected_version}"
note "STEP 3 rollback service answered version=${observed}"

# ── 4. Tear the drill down before touching anything else ─────────────────────
docker stop "${CONTAINER}" >/dev/null 2>&1 || true
trap - EXIT
note "STEP 4 rollback drill container removed"

# ── 5. The live production release is untouched and still declared ───────────
live="$(read_version "${LIVE_URL}")"
if [ -z "$live" ]; then
  note "STEP 5 live service at ${LIVE_URL} did not answer; rollback was still proven"
else
  expected_production="${PRODUCTION_TAG%%-*}"
  [ "$live" = "$expected_production" ] ||
    fail "step 5: live service reports version=${live}, expected ${expected_production}"
  note "STEP 5 live production service reports version=${live} (declared release intact)"
fi

note "ROLLBACK-READY rollback=${ROLLBACK_TAG} observed=${observed}"
exit 0
