# MiniMax Token Plan limits and usage visibility

## Scope

This report documents current MiniMax Token Plan usage visibility and limit behavior for M014. It captures the user's stated operating posture: **subscription budget is not a current blocker for these tests**, but platform request quotas, rate limits, dynamic traffic controls, evidence hygiene, and project safety boundaries still apply.

## Sources checked

- Token Plan Overview: `https://platform.minimax.io/docs/token-plan/intro`
- Token Plan FAQ: `https://platform.minimax.io/docs/token-plan/faq`
- Rate Limits: `https://platform.minimax.io/docs/guides/rate-limits`

## How to view Token Plan usage and limits

MiniMax documents two ways to check Token Plan usage:

1. **Subscription Management Page**
   - Path: `Billing > Token Plan`
   - URL documented by MiniMax: `https://platform.minimax.io/user-center/payment/token-plan`
   - Purpose: view available Token Plan resources/usage and get the Token Plan Key.

2. **Token Plan remains API endpoint**
   - Endpoint documented by MiniMax:
     ```bash
     curl --location 'https://www.minimax.io/v1/token_plan/remains' \
       --header 'Authorization: Bearer <API Key>' \
       --header 'Content-Type: application/json'
     ```
   - M014 persists only sanitized endpoint status/shape, not the raw body and never the key.

## Token Plan key behavior

- Token Plan Keys are separate from standard pay-as-you-go API keys.
- Token Plan Keys are used for Token Plan quotas and Credits.
- A key may exist before a user/team has paid resources; it becomes usable when a Token Plan seat or Credits are available.
- If both quota and Credits are available, Token Plan quota is used first, then Credits may cover overflow within Token Plan coverage.

## Standard plan quotas

Text model quota uses a 5-hour rolling request window. Non-text models use daily quotas.

| Resource | Starter | Plus | Max |
|---|---:|---:|---:|
| M2.7 | 1,500 requests / 5 hrs | 4,500 requests / 5 hrs | 15,000 requests / 5 hrs |
| Speech 2.8 | unavailable | 4,000 chars / day | 11,000 chars / day |
| image-01 | unavailable | 50 images / day | 120 images / day |
| Hailuo-2.3-Fast 768P 6s | unavailable | unavailable | 2 / day |
| Hailuo-2.3 768P 6s | unavailable | unavailable | 2 / day |
| Music-2.6 | 100 songs / day limited free | 100 songs / day limited free | 100 songs / day limited free |

## Highspeed plan quotas

| Resource | Plus-Highspeed | Max-Highspeed | Ultra-Highspeed |
|---|---:|---:|---:|
| M2.7-highspeed | 4,500 requests / 5 hrs | 15,000 requests / 5 hrs | 30,000 requests / 5 hrs |
| Speech 2.8 | 9,000 chars / day | 19,000 chars / day | 50,000 chars / day |
| image-01 | 100 images / day | 200 images / day | 800 images / day |
| Hailuo-2.3-Fast 768P 6s | unavailable | 3 / day | 5 / day |
| Hailuo-2.3 768P 6s | unavailable | 3 / day | 5 / day |
| Music-2.6 | 100 songs / day limited free | 100 songs / day limited free | 100 songs / day limited free |

## API rate limits

MiniMax rate-limit docs distinguish quota from RPM/TPM throttles.

For text models:

| API | Model | RPM | TPM |
|---|---|---:|---:|
| Text API | MiniMax-M2.7 / MiniMax-M2.7-highspeed | 500 | 20,000,000 |
| Text API | MiniMax-M2.5 / MiniMax-M2.5-highspeed | 500 | 20,000,000 |
| Text API | MiniMax-M2.1 / MiniMax-M2.1-highspeed | 500 | 20,000,000 |
| Text API | MiniMax-M2 | 500 | 20,000,000 |

Other documented examples:

- Speech T2A: 60 RPM
- Voice Design: 20 RPM
- Video Generation: 5 RPM
- Image Generation: 10 RPM
- Music Generation: 120 RPM / 20 connections

## Reset behavior

- Text M2.7 quota: rolling 5-hour request window.
- Other model quotas: daily reset.
- API rate-limit throttles typically reset on short windows; MiniMax describes RPM/TPM controls and dynamic peak traffic behavior.

## Production suitability caveat

MiniMax Token Plan FAQ says Token Plan is designed for individual, interactive developer use, with higher tiers offering increased quotas. It recommends pay-as-you-go for production use. Key limits include RPM/TPM throttles, text request caps, non-text daily quotas, and dynamic traffic rules.

## Platform traffic rules

MiniMax documents dynamic rate limiting during peak hours, especially for high-concurrency automated batch tasks or multi-user sharing patterns. The docs mention peak periods may be dynamically adjusted and that account-level traffic may be controlled to preserve stability.

Documented FAQ details checked in M014:

- Peak traffic hours are dynamically adjusted based on cluster load; the FAQ gives a typical weekday window of `15:00–17:30`.
- Approximate continuous-agent guidance during peak traffic:
  - Starter / Plus: about `1` continuous agent.
  - Max: about `2` continuous agents.
  - Ultra: about `4` continuous agents.
- Weekly usage quota is documented as `10 × the 5-hour quota`.
- The FAQ says users who purchased before `2026-03-22 23:59:59` are not subject to weekly quota limits, while purchases from `2026-03-23 00:00:00` onward are subject to weekly quota limits.

M014 does not know the purchase timestamp or active plan tier for the current key, so it treats this as an operational constraint to check in account UI/API before any sustained use.

## M014 operating posture

- Subscription budget is **not** the current blocker for MiniMax real tests.
- Platform limits still apply: quota windows, RPM/TPM, daily limits, dynamic traffic rules, and production-suitability caveats.
- M014 real tests must stay bounded and interactive, not unattended batch scaling.
- M014 artifacts must not persist secrets, raw response bodies, raw model content, raw paper/chunk/PDF text, embeddings, or vectors.
- MiniMax remains a helper candidate only, not orchestrator or source of truth.
