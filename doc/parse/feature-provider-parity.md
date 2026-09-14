# Feature ABI и provider parity S01

## Назначение

Этот документ фиксирует фактический provider-neutral ABI, реализованный в
`da-domain`, и test-only harness сравнения двух provider fixtures. Harness
является offline candidate evidence: он не запускает Java GROBID, не пишет граф,
не меняет production route и не меняет `import_eligible=false`.

Инвентарный ledger `data/parse/feature_tagger_training_parity.yaml` остаётся
источником сведений о Java GROBID 0.9.0-crf и прямо помечает implementation
parity как неподтверждённую. Поэтому этот документ не объявляет Rust ABI
эквивалентным Java training vector; он фиксирует только проверяемый контракт
T03 и способ сравнения одинаковых входов.

## Исполняемый ABI

Публичные типы находятся в `crates/da-domain/src/feature_abi.rs`:

- `FeatureToken`: `text`, `page`, `x`, `y`, `width`, `height`, `bold`, `italic`;
- `FeatureChannel`: строго упорядоченные `key` и отсортированные `entries`;
- `FeatureTemplate`: строго упорядоченные шаблоны с `channel_keys` и
  `feature_inputs`;
- `FeatureAbi`: `channels`, `templates` и необязательный
  `expected_dimension`;
- `FeatureVector`: плоский `Vec<f32>`.

Поддержанные входы шаблона ограничены следующими строками:

```text
token.text, token.x, token.y, token.width, token.height,
token.page, token.bold, token.italic
```

Порядок вывода является частью ABI и не зависит от порядка входного массива:

1. токены канонически сортируются по `(page, y, x, text)`;
2. для каждого токена перебираются `channels` в объявленном порядке;
3. затем `templates` в объявленном порядке;
4. затем `feature_inputs` в объявленном порядке.

`token.text` выдаёт membership `0.0/1.0` через binary search по entries канала;
layout-поля выдаются как числовые значения. Dimension вычисляется checked
арифметикой до выделения вектора. Unknown channel/input, overflow, non-finite
layout и mismatch фиксированной размерности дают типизированную ошибку
`FeatureAbiError`, а не silent approximation.

## Provider comparison harness

Тест `crates/da-adapters/tests/feature_provider_parity.rs` использует
`crates/da-adapters/tests/fixtures/` и common loader
`crates/da-adapters/tests/common/feature_provider_parity.rs`.

Оба фиксированных provider fixtures содержат одну страницу и одинаковые
логические токены:

- `provider_pdfalto_page.json` — provider id `pdfalto-fixture`;
- `provider_pdf_inspector_page.json` — provider id `pdf-inspector-fixture`.

Независимо ожидаемый vector для ABI harness равен:

```text
[1.0, 10.0, 10.0, 1.0, 0.0, 10.0, 20.0, 0.0]
```

Это доказывает одинаковый vector на одинаковом входе, но не доказывает
эквивалентность реальных PDF engines или Java model outputs.

Изменение координаты в одной fixture приводит к typed
`ProviderParityError::OutputMismatch`; оно не превращается в PASS по среднему
результату и не скрывается в диагностике. Missing fixture и malformed fixture
также завершаются fail-closed. Диагностика содержит только имя fixture/provider,
тип ошибки и ABI error: payload, абсолютные пути и raw document content не
возвращаются.

## Verification boundary

Harness запускается адресно:

```text
cargo test -p da-adapters --test feature_provider_parity
```

Используются только committed test fixtures; модельные байты GROBID не
добавляются (D358). Реальное различие PDF-провайдеров должно быть измерено
отдельным будущим evidence run на одном и том же PDF и тем же ABI; отсутствие
такого входа является missingness, а не успешным сравнением.
