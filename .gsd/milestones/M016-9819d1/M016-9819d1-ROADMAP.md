# M016-9819d1: MiniMax Limits via 9router Implementation

**Vision:** Correct MiniMax Token Plan limit checking by using 9router's known implementation instead of further guesswork.

## Success Criteria

- 9router implementation is used as primary reference.
- M015 endpoint omission is explicitly corrected.
- Live probe follows 9router endpoint order.
- Final verdict is precise and reviewed by artifact assertions.
- No production import/write or raw secret/quota persistence occurs.

## Slices

- [x] **S01: S01** `risk:medium` `depends:[]`
  > After this: After S01, we know exactly how 9router checks MiniMax limits.

- [x] **S02: S02** `risk:medium` `depends:[]`
  > After this: After S02, M016 has corrected MiniMax limit-check verdict using 9router algorithm.

## Boundary Map

| Area | In scope | Out of scope |
|---|---|---|
| 9router source | Clone/index/read usage implementation and tests | Vendor code modification |
| MiniMax limits | Correct endpoint/fallback/parsing based on 9router | Raw response/secret persistence |
| Project | Update GSD evidence and recommendation | Production KG import/write |
