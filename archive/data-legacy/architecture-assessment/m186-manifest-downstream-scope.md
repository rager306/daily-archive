# M186 Manifest Downstream Scope

## Decision

**Do not continue opportunistic manifest residual wiring under preserve-ratchet.**

## Rationale

S10 proved that the movement pattern can pass focused behavior checks while still violating the active strict drift ratchet. S11 converted that blocker into a machine-checkable transition contract. S12 and S13 then closed the remaining residuals as no-move under that contract.

## Downstream choices

Future slices have two safe options:

1. Keep preserve-ratchet and focus on unrelated baseline drift or integration cleanup.
2. Explicitly switch to transition-ratchet with a canonical inventory baseline update and decision artifact before moving any residual writer.

A mixed approach is not allowed: wiring residuals while claiming strict `script-only=4` preservation is now known to be contradictory.
