# M516-th0oby/S12 — список решений для релиза

**Статус пакета: подготовлен, не авторизован** (`prepared, not authorized`). Этот документ
**готовит** решение для настоящего человека; он его не принимает и не фиксирует. Ни один
агент не является рецензентом: живая запись человека пуста (`reviewer=null`,
`decision=pending`). Проза — этот документ, ADR, коммит или зелёный тест — релиз не
авторизует.

Авторитет для каждого числа и статуса ниже —
`artifacts/parse_compat/grobid_retirement_release_readiness.json` (схема
`grobid-retirement-release-readiness.v1`). Артефакт выведен из закоммиченных носителей, и
каждый sha256 в нём пере-вычислен из закоммиченных байтов. Блок ниже — **проверяемая
проекция** этого артефакта: контракт
`crates/da-cli/tests/release_readiness_receipt_doc_contract.rs` сверяет его с артефактом,
поэтому перепечатанный статус ломает контракт, а не проходит молча.

<!-- readiness-projection:begin -->
```json
{
  "schema_version": "grobid-retirement-release-readiness.v1",
  "claim_scope": "release_readiness_preparation_only",
  "thresholds": "not_accepted",
  "product_coverage_effect": "none",
  "s09_authorized": false,
  "gsd_lifecycle_authorizes": false,
  "human_release_recorded": false,
  "locks": {
    "deployed": "false",
    "import_eligible": "false",
    "auto_promote": "forbidden",
    "production_route": "forbidden",
    "graph_write": "forbidden"
  },
  "gates": [
    {
      "gate_id": "quality",
      "status": "blocked",
      "release_effect": "does_not_authorize_production_use"
    },
    {
      "gate_id": "cost",
      "status": "bounded_not_measured",
      "release_effect": "does_not_authorize_external_downloads"
    },
    {
      "gate_id": "rights",
      "status": "blocked",
      "release_effect": "does_not_authorize_redistribution"
    }
  ]
}
```
<!-- readiness-projection:end -->

## 1. Что решает человек — pending human decisions

Решение принимает настоящий человек; машина его не подменяет. Незакрытыми остаются ровно
пять решений — это `human_decision.authorization_requires` из артефакта:

| # | Что должен сделать человек | Живой статус |
|---|---|---|
| 1 | Назвать себя рецензентом (`named_reviewer`); поле `reviewer` сейчас пусто | `reviewer=null` — не назван |
| 2 | Принять явное решение (`explicit_decision`); поле `decision` сейчас `pending` | `decision=pending` — не принято |
| 3 | Подтвердить прохождение гейта качества (`passing_quality_gate`) | `quality` = `blocked` — не пройден |
| 4 | Подтвердить прохождение гейта стоимости (`passing_cost_gate`) | `cost` = `bounded_not_measured` — не пройден |
| 5 | Подтвердить прохождение гейта прав (`passing_rights_gate`) | `rights` = `blocked` — не пройден |

Пока хотя бы одно из пяти условий не выполнено, релиз не авторизован. Три «гейта» ниже —
это ровно три строки `release_gates` артефакта.

## 2. Три гейта — quality, cost, rights

Каждый гейт несёт доказательство, привязанное в машинно-читаемом артефакте тройкой
**путь + pointer + sha256**. Ниже — эти привязки; каждый sha256 пере-вычислен из
закоммиченных байтов носителя.

### quality — `blocked`

Release effect: `does_not_authorize_production_use`.

| Носитель (путь) | pointer | sha256 |
|---|---|---|
| `data/parse/acceptance_manifest.yaml` | `release_decision_package.quality` | `sha256:4d1c55ea9dd873b4aada48aa964609395f59e8a726bdf42c65d2b0f36865ca06` |
| `data/parse/layout_bakeoff.yaml` | `native_route_corpus` | `sha256:a98d1772e68da35b9deb99c62e4c9ab413e53f0d1ddf918e017ac2242c0d80cc` |
| `artifacts/parse_compat/absorption_progress_threshold_candidates.json` | `threshold_candidates` | `sha256:cfcd77fac15df4d34670c0c9448d229dfd58b4d24fcb7790fbbb476869ff78b4` |
| `artifacts/parse_compat/grobid_retirement_acceptance_package.json` | `mandatory_gates` | `sha256:3aa816164b8e23ae6cc0da6cd0e50446e65923531eaf289aa4eb587e1102a0e7` |

### cost — `bounded_not_measured`

Release effect: `does_not_authorize_external_downloads`.

| Носитель (путь) | pointer | sha256 |
|---|---|---|
| `data/parse/acceptance_manifest.yaml` | `external_holdout_contract.cost_bounds` | `sha256:4d1c55ea9dd873b4aada48aa964609395f59e8a726bdf42c65d2b0f36865ca06` |

### rights — `blocked`

Release effect: `does_not_authorize_redistribution`.

| Носитель (путь) | pointer | sha256 |
|---|---|---|
| `data/parse/acceptance_manifest.yaml` | `release_decision_package.rights` | `sha256:4d1c55ea9dd873b4aada48aa964609395f59e8a726bdf42c65d2b0f36865ca06` |
| `data/parse/grobid_compatibility.yaml` | `parity_ledger.required_scope_downgrades` | `sha256:0065ca553b8b4f647831bec7f372680c4d6c0f9d31701517883d5b17c9d8f4ec` |

## 3. Машина не авторизует ничего — machine authorization = none

Машина ничего не авторизует, и это не декларация, а проверяемое поле артефакта:

- `s09_authorized=false`, `gsd_lifecycle_authorizes=false`, `human_release_recorded=false`;
- `nothing_here_authorizes_a_release=true`;
- `thresholds=not_accepted` — ни один порог не принят;
- `claim_scope=release_readiness_preparation_only`, `product_coverage_effect=none`.

Пять замков закрыты и не сняты ни одним шагом S12:

| Замок | Значение |
|---|---|
| `import_eligible` | `false` |
| `deployed` | `false` |
| `auto_promote` | `forbidden` |
| `production_route` | `forbidden` |
| `graph_write` | `forbidden` |

## 4. Запись решения — decision record

Решение человека фиксируется в `doc/review/HUMAN-REVIEW-TEMPLATE.md` — это и есть
`human_decision.decision_record` артефакта. Шаблон заполняет человек; до этого момента
запись остаётся пустой, а решение — `pending`.

`artifacts/parse_compat/grobid_retirement_release_readiness.json` — авторитет для каждого
числа в этом документе. Если число здесь расходится с артефактом, прав артефакт.
