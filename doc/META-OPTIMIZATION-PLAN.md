# Rust 2026 Meta-Optimization Plan

**Status:** Design (2026-07-29)
**Based on:** Rust 2026 best practices research (Edition 2024, Tokio, performance patterns)

## Current state

- Edition: 2021, rust-version 1.75, rustc 1.94.1
- 10,733 lines across da-* crates, 229 tests green
- Release profile: opt-level=3, lto="thin"
- No `extern` blocks, no `static mut` → edition 2024 ready

## Wave 1: Edition 2024 migration (HIGH IMPACT)

Best practice 2026: "Use 2024 Edition, async closures, precise capturing"
Benefits: never type fallback, async closures, RPIT lifetime capture, `gen` blocks.

Steps:
1. `cargo fix --edition` (automated migration)
2. Update `edition = "2024"` in Cargo.toml
3. Update `rust-version = "1.85"`
4. Verify tests pass

Risk: LOW — no extern blocks, no static mut, no unsafe in our code.

## Wave 2: Release profile optimization (MEDIUM IMPACT)

Best practice: "Tune release profile (LTO, codegen-units=1, panic=abort)"
Current: lto="thin", no codegen-units, no panic setting.

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

Also add dev profile optimization:
```toml
[profile.dev]
opt-level = 0
debug = 1  # faster builds, still debuggable
```

Risk: LOW — config-only change. `panic=abort` is safe for our use case.

## Wave 3: Hot-path extraction optimization (MEDIUM IMPACT)

Best practice: "Reuse allocations, pre-compute, avoid repeated string ops"

### 3a: Pre-lowercase config data
Problem: `to_lowercase()` called 32× in extraction hot path.
Fix: Store lowercase versions of config lists once at load time.
```rust
struct ExtractionConfig {
    // ... existing ...
    methods_lower: MethodConfigLower, // pre-lowercased
}
```

### 3b: Pre-allocate result vectors
Problem: `Vec::new()` in extraction methods.
Fix: `Vec::with_capacity(8)` based on expected entity count.

### 3c: Reduce `.to_string()` in pattern matching
Problem: 72 `.to_string()` calls in hot path.
Fix: Use `&str` references where possible.

## Wave 4: Dependency hygiene (LOW IMPACT)

- Verify all workspace deps are needed
- Check for duplicate transitive deps
- Pin major versions

## Wave 5: Idiomatic patterns (LOW IMPACT)

- Replace manual `for` loops with iterator chains where natural
- Use `let-else` where applicable (stable since 1.65)
- Use `if let chains` (edition 2024)
- Add `#[inline]` on small hot-path functions

## NOT doing

- rkyv/zero-copy deserialization — premature optimization for our scale
- Custom allocator (jemalloc/mimalloc) — not a server, CLI-only
- SIMD — extraction is string matching, not numeric
- `unsafe` — no proven bottleneck warrants it
