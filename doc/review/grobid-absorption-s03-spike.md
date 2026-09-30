# Spike: измерение существующего Rust PDF/layout входа

- **Milestone:** M514-1lss2s / **Slice:** S03 (Независимый PDF и layout путь) / **Task:** T01
- **Дата:** 2026-09-30
- **Роль документа:** recorded evidence для S03 и вход в ADR S04. Не ADR, не решение о качестве.
- **Границы:** измерение имеющегося кода и provenance. Новый PDF engine не создавался, ни один
  коммитнутый артефакт не изменён.

---

## 1. Вопрос

Slice contract требует сначала измерить имеющийся Rust PDF/layout путь и только затем выбирать
low-level engine в ADR. Вопрос spike формулируется так:

1. Существует ли Rust-маршрут PDF → layout → segmentation, который не зовёт Java?
2. Какие у него измеренные характеристики и пробелы?
3. Нужен ли **второй** PDF engine? Нет или да — с границей.

## 2. Найденный маршрут (code evidence)

Единственная точка входа в layout — `crates/da-adapters/src/layout_donors.rs`, feature `pdf-layout`.

| Роль | Символ | Файл:строка | Природа |
|---|---|---|---|
| positioned tokens | `PdfInspectorLayoutDonor` | `crates/da-adapters/src/layout_donors.rs:913` | pure Rust: `pdf_inspector::extract_text_with_positions_mem` (`:928`) |
| reading-order reorder | `XyCutLayoutDonor` | `crates/da-adapters/src/layout_donors.rs:354` | pure Rust, `reorder_shadow_with_xy_cut` |
| external lane | `PdfaltoLayoutDonor` | `crates/da-adapters/src/layout_donors.rs:1066` | subprocess (native C++, не Java) |
| document segmentation | `segment_document_full` | `crates/da-parse/src/native_segmentation.rs:842` | pure Rust |
| lattice validation | `LayoutLattice::validate_against` | `crates/da-parse/src/layout_lattice.rs:195` | pure Rust |
| runner: bakeoff | `layout_bakeoff` | `crates/da-cli/examples/layout_bakeoff.rs` | feature `pdf-layout` |
| runner: segmentation shadow | `document_segmentation_shadow` | `crates/da-cli/examples/document_segmentation_shadow.rs` | feature `pdf-layout` |

Оба runner'а зарегистрированы в `crates/da-cli/Cargo.toml` с `required-features = ["pdf-layout"]`
(строки 12–17).

### 2.1 Java-free доказательство

```bash
grep -n 'Command::new' crates/da-adapters/src/layout_donors.rs
# 1136:            let mut command = Command::new(&self.executable);
# 1230:            let mut child = Command::new(&self.executable)
grep -n 'Command::new\|reqwest\|grobid\|java\|http' crates/da-parse/src/native_segmentation.rs
# (пусто)
grep -n 'Command::new\|reqwest\|grobid\|java' \
  crates/da-cli/examples/document_segmentation_shadow.rs crates/da-cli/examples/layout_bakeoff.rs | grep -v java_page
# crates/da-cli/examples/layout_bakeoff.rs:1278: rollback_target: "java-grobid-http@0.9.0-crf".into(),
```

Чтение результата:

- Оба `Command::new` в `layout_donors.rs` — внутри `PdfaltoLayoutDonor` (native C++ subprocess), не Java.
- `native_segmentation.rs` не имеет ни subprocess, ни HTTP, ни grobid-зависимости: segmentation
  потребляет уже готовую `LayoutLattice`.
- Единственное вхождение `java` в runners — строковый литерал `rollback_target`
  (`layout_bakeoff.rs:1278`). Это **объявленная identity rollback-цели** в отчёте, не вызов.
  В коммитнутом `layout_bakeoff.json` поле `rollback_target` имеет то же значение.
- `java_page_number` в структурах — только имя поля (1-based page identity конвенции pdfalto/Java),
  см. `coord`-комментарий `layout_donors.rs:804`.

**Вывод по вопросу 1:** маршрут существует и на вычислительном пути Java не вызывает.
«Java-free» здесь означает буквально отсутствие Java; `pdfalto` — native C++ и тоже не Java.

## 3. Измерение

### 3.1 Что переисполнялось, а что нет

- **Переисполнено в temp-путь** (`/tmp/s03/`, коммитнутые артефакты не тронуты): Rust-полосы
  `pdf_inspector_positioned` и `pdf_inspector_positioned_xy_cut`, плюс native segmentation.
- **НЕ переисполнено:** полоса `pdfalto`. Бинарь отсутствует (`command -v pdfalto` → not found;
  в `/root/vendor-source/pdfalto` только исходники). Полоса pdfalto является **recorded evidence**,
  а не воспроизводимым измерением. Это ограничение названо явно в §6.

### 3.2 Сборка (slice-plan verify)

```bash
cargo build -p da-cli --features pdf-layout \
  --example document_segmentation_shadow --example layout_bakeoff
# exit 0, 57.41s, 1 warning (unused closure variable в layout_bakeoff)
```

### 3.3 Rust-полосы: свежий прогон в /tmp

```bash
target/debug/examples/document_segmentation_shadow \
  --report /tmp/s03/document_segmentation_shadow.json --workers 1 --queue-capacity 1
target/debug/examples/layout_bakeoff \
  --manifest data/parse/layout_bakeoff.yaml --report /tmp/s03/layout_bakeoff.json \
  --workers 1 --queue-capacity 1 --max-in-flight 1
```

Свежие числа (S13 corpus, 3 документа, 202 страницы):

| document | pages | `pdf_inspector_positioned` tokens | `pdf_inspector_positioned_xy_cut` tokens | coordinate coverage |
|---|---|---|---|---|
| hub-2507.19457 | 96 | 14118 | 14118 | complete |
| rlm-2512.24601 | 43 | 8330 | 8330 | complete |
| reca-2605.26525v1 | 63 | 5430 | 5430 | complete |

`coordinate_valid == token_count` для всех шести Rust-юнитов; `span_count = 0`.

Свежий прогон воспроизводит коммитнутые значения **ток в ток** (см. §3.4) для обеих Rust-полос,
на том же входе и том же page denominator.

### 3.4 Коммитнутые артефакты: per-lane числа (recorded evidence)

Команда извлечения каждого числа:

```bash
python3 -c "
import json; d=json.load(open('artifacts/parse_compat/layout_bakeoff.json'))
[print(o['document_id'], o['lane'], o['status'], o['token_count'], o['coordinate_valid'], o.get('coordinate_coverage'))
 for o in d['outcomes']]"
```

| document | pages | `pdfalto` tokens | `pdf_inspector_positioned` tokens | gap |
|---|---|---|---|---|
| hub-2507.19457 | 96 | 40501 | 14118 | ×2.87 |
| rlm-2512.24601 | 43 | 23700 | 8330 | ×2.85 |
| reca-2605.26525v1 | 63 | 27560 | 5430 | ×5.08 |
| **итого** | 202 | **91761** | **27878** | **×3.29** |

Все девять юнитов `ok`, `failure_code=None`, `coordinate_valid == token_count`,
`coordinate_coverage=complete`. `gate.result="candidate_evidence_complete"`,
`gate.hard_vetoes=[]`, `report_state="final"`, `lane_page_denominators` = 202/202/202,
`total_lane_pages=606`. `python3 -m json.tool artifacts/parse_compat/layout_bakeoff.json >/dev/null`.

### 3.5 Token-coverage gap (названный пробел)

`pdf_inspector_positioned` даёт в 2.87× меньше токенов на hub (40501 → 14118) и в 3.29× меньше
по корпусу. Причина в этом spike **не установлена**: `evaluated_population.status = not_evaluated`
(§3.6), поэтому нет никакого claim'а о качестве. Два взаимоисключающих объяснения, между которыми
из имеющихся данных выбрать нельзя:

- (a) разница в гранулярности токенизации (pdfalto дробит на словоформы/глифы, pdf-inspector — на
  более крупные span'ы), при сохранении тела текста;
- (b) реальная потеря текста на pdf-inspector-полосе.

Разрешение (a)/(b) — это S04/S05, а не T01: нужен независимый oracle на *символы* тела, а не на
токены. Gap назван, не починен.

### 3.6 `evaluated_population.status`

```bash
python3 -c "
import json; d=json.load(open('artifacts/parse_compat/document_segmentation_shadow.json'))
print(json.dumps(d['evaluated_population']))"
# {"documents": 0, "oracle": "reviewed_three_document_segmentation_oracle",
#  "reason": "shadow_boundary_diagnostics_without_a_quality_claim", "status": "not_evaluated"}
```

Свежий прогон в /tmp даёт **тот же** результат: `status=not_evaluated`, `documents=0`,
тот же `reason`. То есть shadow-носитель сознательно не делает quality claim'а — числа выше
являются diagnostics, а не acceptance.

## 4. Provenance-находка: два носителя противоречат друг другу по полосе pdfalto

Один и тот же документ, одна и та же полоса, два коммитнутых носителя:

| носитель | hub-2507.19457 : pdfalto | rlm-2512.24601 : pdfalto | reca : pdfalto |
|---|---|---|---|
| `artifacts/parse_compat/layout_bakeoff.json` | `ok` / 40501 токенов | `ok` / 23700 | `ok` / 27560 |
| `artifacts/parse_compat/document_segmentation_shadow.json` | `terminal_failure` / `malformed` | `invalid_evidence` / `digest_mismatch` | `terminal_failure` / `malformed` |

Расхождение **реально** и не косметическое: один носитель утверждает полное покрытие pdfalto-полосы
(с координатами и page receipts), другой — терминальный отказ на двух документах из трёх.

Наиболее вероятное объяснение, подтверждённое свежим прогоном: носители произведены при **разной
доступности бинаря pdfalto**. Без бинаря `layout_bakeoff` подставляет `UnavailableDonor`
(`layout_bakeoff.rs:1417`), а полоса помечается `terminal_failure/malformed` — в точности то, что
записано в `document_segmentation_shadow.json` для hub и reca.

Механизм этой маскировки — отдельная диагностическая находка:

```rust
// crates/da-adapters/src/layout_donors.rs:92-102 — default в trait
fn extract_shadow(...) -> Result<LayoutShadowObservation, LayoutDonorError> {
    Err(LayoutDonorError::Malformed {
        detail: "layout donor has no shadow lexical observation".into(),
    })
}
```

`UnavailableDonor` реализует только `extract`, поэтому `extract_shadow` попадает в дефолт и
«нет pdfalto» неотличимо от «pdfalto вернул битые данные». Свежий прогон без бинаря:
`failure_code="malformed"` для всех трёх документов, `gate.hard_vetoes = ["hub…:pdfalto:malformed",
"rlm…:pdfalto:malformed", "reca…:pdfalto:malformed"]`, `gate.result="no_selection"`,
`exact_page_denominator=false`, `lane_page_denominators.pdfalto=0`.

Это **провенанс-находка, не баг**: носители остаются как есть; расхождение зафиксировано, чтобы
S04/S05 (и любой будущий reader) не приняли `ok/40501` за факт о pdfalto-полосе.

Что при этом работает правильно, и это важно для Q5: отсутствие обязательной полосы **не**
приводит к «успеху с пропущенной полосой». Gate fail-closed: `hard_vetoes` непуст → `no_selection`,
`conservation_complete=false`, `oracle_complete=false`. Именно так и должен вести себя bakeoff.

## 5. Лицензии и provenance

| компонент | статус | evidence |
|---|---|---|
| `pdf_inspector` | MIT, `license_status: verified` | `data/parse/license_inventory.yaml:310-323` |
| `pdf_inspector` resolved version | `0.1.7` из crates.io | `Cargo.lock:3221` |
| vendored `pdf-inspector` | `1.17.0` | `/root/vendor-source/pdf-inspector/Cargo.toml:3` |
| `pdfalto_runtime` | GPL-2.0, `license_status: blocked`, `boundary: subprocess` | `data/parse/license_inventory.yaml:291-308` |
| `lopdf` | **отсутствует** в inventory | `Cargo.lock:2733` (`0.41.0`), транзитивный к `pdf-inspector` |

Три названных пробела:

1. **Расхождение версий pdf-inspector.** Разрешается `crates.io pdf-inspector 0.1.7` (`Cargo.lock:3221`),
   а `license_evidence` в inventory указывает на vendored-дерево версии `1.17.0`. В workspace
   `Cargo.toml` **нет секции `[patch]`** (`grep -n '\[patch' Cargo.toml` → пусто), то есть vendored
   дерево не потребляется. MIT действует для обеих версий, но license evidence привязан не к той
   версии, которая компилируется. Граница: это не блокер для P1/P2, но ADR S04 обязан либо привязать
   evidence к resolved-версии, либо добавить `[patch]` и переизмерить.
2. **Лицензионный пробел `lopdf`.** `lopdf 0.41.0` — транзитивная зависимость `pdf-inspector`
   (`Cargo.lock:2733`), но отсутствует и в `data/parse/license_inventory.yaml`, и в
   `data/parse/grobid_compatibility.yaml`, и вообще в `data/parse/*.yaml`
   (`grep -rn 'lopdf' data/parse/*.yaml` → пусто). Лицензия PDF-парсера, который физически читает
   байты, не записана нигде.
3. **Полоса pdfalto неисполнима.** `license_status: blocked` (GPL-2.0, subprocess-only,
   no distribution) и бинаря нет локально. Следствие: pdfalto-числа из §3.4 — не более чем
   recorded evidence; проверить их заново на этой машине нельзя.

## 6. Ограничения этого spike

- **pdfalto-полоса не воспроизводима локально.** Названо явно; все pdfalto-числа помечены как
  recorded-only. Более того, §4 показывает, что сами recorded-носители между собой расходятся.
- **Нет quality claim'а.** `evaluated_population.status = not_evaluated`; token-coverage gap
  остаётся неразрешённым (a vs b).
- **Мой свежий прогон — `--workers 1`.** Числа токенов/координат детерминированы и совпадают с
  коммитнутыми, но latency/throughput моего прогона несопоставимы с 12-worker коммитнутым
  (`workers: 12`, `queue_capacity: 24`, `max_in_flight: 12`). Load-числа ниже берутся из
  коммитнутого артефакта, а не из моего прогона.
- Visual heads: redaction-контракт bakeoff содержит `visual_structure_heads: false`; отдельный
  носитель `artifacts/parse_compat/native_visual_heads_shadow.json` существует. Качество visual
  heads в этом spike не измерялось — принадлежит S05.

## 7. Прямой ответ: нужен ли второй engine?

**Для P1 (PDF → layout без Java) и P2 (Rust segmentation этого layout) — нет.**

Evidence:

- Маршрут существует, работает на S13 corpus и не зовёт Java (§2.1).
- `pdf_inspector_positioned` даёт полное соответствие координат (`coordinate_valid == token_count`,
  `complete`) на всех 202 страницах (§3.3, §3.4).
- `da_parse::segment_document_full` сегментирует этот layout без subprocess и без HTTP (§2.1).
- Расхождения и пробелы, которые могли бы потребовать второго engine, **названы и адресованы
  вперёд**: token-coverage gap → S04/S05 (§3.5), visual heads → S05, license/provenance → S04 (§5).

**Граница.** «Не нужен» — про механику P1/P2, а не про качество. Второй engine станет оправдан,
только если S04/S05 докажет, что:

- (b) часть тела текста реально теряется (не просто иная гранулярность токенов), **или**
- visual heads/структура не восстанавливаются нативно.

До этих доказательств добавление второго engine было бы решением по умолчанию, а не по evidence —
ровно то, что запрещает slice contract.

Отдельно: **pdfalto не является запасным «вторым engine»** — он GPL-2.0 blocked и не
устанавливается локально, поэтому как выбираемая альтернатива он не существует. GPL-путь может
оставаться только как измеряемая recorded evidence в shadow.

---

## 8. Evidence-команды (все числа воспроизводимы)

| # | Что доказывает | Команда | Результат |
|---|---|---|---|
| E1 | сборка runners | `cargo build -p da-cli --features pdf-layout --example document_segmentation_shadow --example layout_bakeoff` | exit 0, 57.41s |
| E2 | Rust-полосы, свежий прогон | `target/debug/examples/document_segmentation_shadow --report /tmp/s03/document_segmentation_shadow.json --workers 1 --queue-capacity 1` | exit 0, 154.7s |
| E3 | bakeoff, свежий прогон | `target/debug/examples/layout_bakeoff --manifest data/parse/layout_bakeoff.yaml --report /tmp/s03/layout_bakeoff.json --workers 1 --queue-capacity 1 --max-in-flight 1` | exit 0, 619.9s |
| E4 | per-lane числа (коммитнутые) | `python3 -c "import json; …artifacts/parse_compat/layout_bakeoff.json…"` | таблица §3.4 |
| E5 | not_evaluated | `python3 -c "…document_segmentation_shadow.json…"` | §3.6 |
| E6 | provenance-расхождение | те же E4 + §4-команда по segmentation shadow | §4 |
| E7 | лицензии | `grep -n 'pdfalto_runtime\|pdf_inspector' data/parse/license_inventory.yaml`; `grep -n -A3 '^name = "lopdf"' Cargo.lock`; `grep -n '\[patch' Cargo.toml` | §5 |
| E8 | Java-free | `grep -n 'Command::new' crates/da-adapters/src/layout_donors.rs`; `grep -n 'Command::new\|reqwest\|grobid\|java\|http' crates/da-parse/src/native_segmentation.rs` | §2.1 |
| E9 | negative tests | `cargo test -p da-adapters --features pdf-layout --test layout_donor_contract`; `cargo test -p da-parse --test native_segmentation_contract --test native_segmentation_properties --test segmentation_data_contract --test native_segmentation_oracle` | 6 + 8 + 3 + 3 + 4 = 24 passed, 0 failed |
| E10 | коммитнутые носители не изменены | `git status --short` | только `?? doc/review/grobid-absorption-s03-spike.md`; ни один tracked-файл не изменён (`git diff --name-only` пусто) |

### Формат таблицы для сводки slice

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| E1 | `cargo build -p da-cli --features pdf-layout --example document_segmentation_shadow --example layout_bakeoff` | 0 | PASS | 57.4s |
| E2 | `document_segmentation_shadow --report /tmp/s03/… --workers 1 --queue-capacity 1` | 0 | PASS | 154.7s |
| E3 | `layout_bakeoff --manifest … --report /tmp/s03/… --workers 1 --queue-capacity 1 --max-in-flight 1` | 0 | PASS | 619.9s |
| E4 | `python3 … layout_bakeoff.json per-lane` | 0 | PASS | <0.1s |
| E5 | `python3 … document_segmentation_shadow.json evaluated_population` | 0 | PASS | <0.1s |
| E7 | license/provenance greps | 0 | PASS | <0.1s |
| E8 | Java-free greps | 0 | PASS | <0.1s |
| E9 | `cargo test -p da-adapters …layout_donor_contract` + 4 da-parse suites | 0 | PASS | 85.2s + 0.6s |
| E10 | `git status --short` (+ `git diff --name-only`) | 0 | PASS | <0.1s |
