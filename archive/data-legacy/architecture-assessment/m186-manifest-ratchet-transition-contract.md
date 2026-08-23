# M186 Manifest Ratchet Transition Contract

## Verdict

**Current mode: preserve-ratchet.**

Residual manifest scripts must not be wired to the S09 atomic writer while strict drift is required to stay at `script-only=4`, `unknown=0`, `shared-state=0`.

## Allowed modes

### preserve-ratchet

- Residual wiring allowed: false
- Required counts: `script-only=4`, `unknown=0`, `shared-state=0`
- Required evidence: strict drift pass, inventory tests pass, lifecycle contract keeps residual blocked

### transition-ratchet

- Residual wiring allowed: true
- Required counts: explicit new `script-only` baseline, `unknown=0`, `shared-state=0`
- Required evidence: exact GitNexus impact, focused residual tests, strict drift delta explanation, canonical inventory baseline update, ratchet decision artifact

## Next-slice rule

S12-S13 must either remain no-move under preserve-ratchet or first switch this contract to transition-ratchet with evidence.
