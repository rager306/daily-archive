# M077 GLM Shell Redacted Inventory

Source: `/root/glm.sh`

Secret values are not printed or persisted. Secret-like variables are shown as `[redacted]`.

- assignment_count: `11`
- secret_values_persisted: `false`
- source_copied: `false`

## Generic Anthropic-compatible variables detected

- `ANTHROPIC_BASE_URL`
- `ANTHROPIC_AUTH_TOKEN`
- `ANTHROPIC_MODEL`
- `ANTHROPIC_SMALL_FAST_MODEL`
- `ANTHROPIC_DEFAULT_SONNET_MODEL`
- `ANTHROPIC_DEFAULT_OPUS_MODEL`
- `ANTHROPIC_DEFAULT_HAIKU_MODEL`

## Assignments

| Line | Name | Kind | Value |
|---:|---|---|---|
| 6 | `ANTHROPIC_BASE_URL` | `url` | `https://api.z.ai/api/anthropic` |
| 7 | `ANTHROPIC_AUTH_TOKEN` | `secret` | `[redacted]` |
| 8 | `API_TIMEOUT_MS` | `config` | `3000000` |
| 9 | `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` | `config` | `1` |
| 10 | `ANTHROPIC_MODEL` | `model` | `glm-5.2[1m]` |
| 11 | `ANTHROPIC_SMALL_FAST_MODEL` | `model` | `GLM-4.5-Air` |
| 12 | `ANTHROPIC_DEFAULT_SONNET_MODEL` | `model` | `glm-5.2[1m]` |
| 13 | `ANTHROPIC_DEFAULT_OPUS_MODEL` | `model` | `glm-5.2[1m]` |
| 14 | `ANTHROPIC_DEFAULT_HAIKU_MODEL` | `model` | `glm-4.5-air` |
| 15 | `CLAUDE_CODE_SUBAGENT_MODEL` | `model` | `glm-5.2[1m]` |
| 16 | `CLAUDE_CODE_AUTO_COMPACT_WINDOW` | `config` | `1000000` |

## Collision note

The detected `ANTHROPIC_*` variables are generic compatibility names. They must not be stored directly in project `.env` as provider source of truth because MiniMax, GLM/Z.ai, and real Anthropic clients can all need Anthropic-compatible names.
