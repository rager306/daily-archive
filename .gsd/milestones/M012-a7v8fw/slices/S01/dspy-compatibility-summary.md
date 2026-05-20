# DSPy compatibility summary

## Verdict

DSPy is **conditionally compatible for an optional/dev prototype**, but **not ready for production runtime activation**.

## Evidence

- Local source: `/root/vendor-source/dspy`
- Version: `3.2.1`
- Python requirement: `>=3.10, <3.15`
- Import available now: `False`
- Compatibility status: `blocked_missing_dependencies`

## Main blocker

Top-level DSPy import currently fails because `cloudpickle` is missing from the active `daily-archive` environment. No dependency installation was attempted in this spike.

## Safe path

Use DSPy only in an optional/dev probe first. Preserve `ExtractionPatch` as the source of truth, keep optimizers disabled/fail-closed, and do not call external LMs or write production KG data.

## Blocked

- Production DSPy runtime import
- DSPy optimizers / teleprompt compile
- External LM activation
- Positive KG import
- Production LadybugDB writes
