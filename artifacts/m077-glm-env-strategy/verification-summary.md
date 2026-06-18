# M077 Verification Summary

## Scope

M077 defined safe env storage for GLM/Z.ai alongside MiniMax. It used `/root/glm.sh` only as an external local source for redacted variable inventory; the file was not copied into the repository.

## Artifacts

- `glm-sh-redacted-inventory.json`
- `glm-sh-redacted-inventory.md`
- `provider-env-strategy.md`
- `.agents/skills/glm-zai-safe-helper/SKILL.md`
- `.agents/skills/glm-zai-safe-helper/references/zai-findings.md`
- `.env.example`

## Requirement linkage

Updated `R066` with the M077 namespace policy.

## Env strategy

Persistent provider-specific source-of-truth keys:

- `GLM_API_KEY`
- `GLM_ANTHROPIC_BASE_URL`
- `GLM_CHAT_COMPLETIONS_URL`
- `GLM_MODEL`
- `GLM_SMALL_FAST_MODEL`
- `MINIMAX_API_KEY`
- `MINIMAX_ANTHROPIC_BASE_URL`
- `MINIMAX_OPENAI_BASE_URL`
- `MINIMAX_MODEL`

Generic Anthropic-compatible runtime-only mappings:

- `ANTHROPIC_AUTH_TOKEN <- GLM_API_KEY`
- `ANTHROPIC_BASE_URL <- GLM_ANTHROPIC_BASE_URL`
- `ANTHROPIC_MODEL <- GLM_MODEL`

Do not store `ANTHROPIC_AUTH_TOKEN` or `ANTHROPIC_BASE_URL` in project `.env` as provider source-of-truth keys.

## Verification commands

```bash
test -f artifacts/m077-glm-env-strategy/glm-sh-redacted-inventory.json \
  && test -f artifacts/m077-glm-env-strategy/glm-sh-redacted-inventory.md \
  && rg -n "ANTHROPIC_BASE_URL|ANTHROPIC_AUTH_TOKEN|ANTHROPIC_MODEL|redacted|secret" artifacts/m077-glm-env-strategy/glm-sh-redacted-inventory.md \
  && ! rg -n "sk-|eyJ|Bearer " artifacts/m077-glm-env-strategy/glm-sh-redacted-inventory.json artifacts/m077-glm-env-strategy/glm-sh-redacted-inventory.md
```

Result: **PASS**.

```bash
test -f .agents/skills/glm-zai-safe-helper/SKILL.md \
  && test -f .env.example \
  && rg -n "GLM_API_KEY|GLM_ANTHROPIC_BASE_URL|GLM_CHAT_COMPLETIONS_URL|MINIMAX_API_KEY|runtime mapping|Do not store ANTHROPIC_AUTH_TOKEN" .agents/skills/glm-zai-safe-helper .env.example \
  && ! rg -n "sk-|eyJ|Bearer " .agents/skills/glm-zai-safe-helper .env.example
```

Result: **PASS**.

```bash
test -f artifacts/m077-glm-env-strategy/verification-summary.md \
  && rg -n "R066|GLM_API_KEY|MINIMAX_API_KEY|ANTHROPIC_AUTH_TOKEN|no live API|no secrets" artifacts/m077-glm-env-strategy/verification-summary.md
```

Result: **PASS**.

## Boundaries

- no live API calls
- no secrets printed or persisted
- no real `.env` mutation
- no provider implementation
- no graph writes or promotion
