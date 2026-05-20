# S01: Token Plan limits and quota observability — UAT

**Milestone:** M014-65dlgp
**Written:** 2026-05-20T11:13:54.132Z

# S01: Token Plan limits and quota observability — UAT

## Result

- Usage UI documented: `Billing > Token Plan`
- Usage endpoint documented: `https://www.minimax.io/v1/token_plan/remains`
- Current remains endpoint probe attempted: `true`
- Current remains endpoint HTTP status: `403`
- Raw response persisted: `false`
- Credential value logged: `false`
- Subscription budget non-blocking: `true`
- Platform limits still apply: `true`
- S02 live-call cap: `6`

## Meaning

The project knows where to inspect Token Plan limits and how to call the remains endpoint. The current key did not authorize remains access, but MiniMax text calls remain available for bounded real tests.
