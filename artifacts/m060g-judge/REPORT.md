# M060-gakmo0 REPORT: выбор figure QA judge для M061+

## 0. Резюме M059b pilot: M3 multimodal выбран

M060-gakmo0 завершает диагностический пилот M059b по выбору судьи качества фигур для слоя M058 v2 figure layer. По итогам S01 smoke test и S02 пилота на 30 фигурах выбран `minimax-m3-multimodal-anthropic` через binding `figure-qa-judge-quality`. Решение практическое: M3 multimodal оказался примерно в 3 раза быстрее M2.7-highspeed на полном pilot-run и лучше по двум из трёх ключевых измерений качества.

Вывод для M061: использовать M3 multimodal как production figure QA judge для диагностической оценки качества извлечённых фигур. M2.7-highspeed сохраняется как полезный сравнительный baseline для caption-heavy triage, но не становится основным судьёй.

## 1. Контекст: figure QA judge для M058 v2 figure layer

M058 v2 figure layer дал корпус фигур и метаданных, которые нужно проверять перед расширением ingestion-пути на 2-hop BFS. Риск не в том, что модель вообще отвечает, а в том, что она может неверно оценить полноту изображения, соответствие подписи и структурную верность диаграмм. Поэтому M060g был не production-import milestone, а диагностическим выбором judge-модели.

Safety-контракт остаётся прежним и записывается точными guardrail-фразами: Graph writes are not authorized. Production import is not authorized. Fact promotion is not authorized. External network default is disabled. LLM calls default is disabled. Единственное исключение — scoped diagnostic-only LLM override для M060-gakmo0, где `llm_calls_authorized` установлен в `true` только для оценки figure QA и не разрешает запись в граф или продвижение фактов.

Локальные ссылки и диагностические инструкции должны использовать `127.0.0.1`, а не loopback hostname, чтобы guardrail-сканеры и runtime-документация оставались однозначными.

## 2. S01 smoke test: latency baseline

S01 добавил bindings в `models.yaml` и выполнил smoke test по трём путям вызова:

| Binding / модель | Модальность | Latency baseline |
|---|---:|---:|
| `figure-qa-judge-fast` / M2.7-highspeed | text | 3030 ms |
| M3 text path | text | 1350 ms |
| `figure-qa-judge-quality` / M3 multimodal | image + text | 4730 ms |

Smoke test подтвердил, что оба кандидата доступны, отвечают через Anthropic-compatible endpoint и дают пригодный JSON/текстовый результат для диагностического пайплайна. В S01 M3 multimodal был медленнее text-only M3, но оставался достаточно быстрым для image-aware проверки.

## 3. S02 30 figure pilot

S02 прогнал 30 фигур: 15 `data_plot` и 15 `schema_diagram`. Обе модели прошли 30/30 без failed runs. Сравнение показало разные профили сильных сторон.

| Модель | Caption accuracy | Figure completeness | Structural fidelity | Средняя latency | Outliers | Failed |
|---|---:|---:|---:|---:|---:|---:|
| M2.7-highspeed | 0.7477 | 0.7823 | 0.7467 | 23846 ms | 3 | 0 |
| M3 multimodal | 0.6907 | 0.8757 | 0.8603 | 8549 ms | 7 | 0 |

Победители по измерениям:

- `caption_accuracy`: M2.7-highspeed, 0.7477 против 0.6907.
- `figure_completeness`: M3 multimodal, 0.8757 против 0.7823.
- `structural_fidelity`: M3 multimodal, 0.8603 против 0.7467.

По side-by-side результатам M3 выиграл 23 фигуры, M2.7 выиграл 6, 1 сравнение завершилось tie. Это достаточно сильный сигнал для выбора M3 как основного judge, потому что figure QA критичнее завязан на визуальную полноту и структуру, чем на чистое совпадение caption-текста.

Latency comparison особенно важен для M061: M3 multimodal показал 8549 ms average против 23846 ms у M2.7-highspeed, то есть примерно 3x faster на том же pilot-корпусе. При масштабировании на тысячи фигур это меняет wall-time с непрактичного на управляемый.

Outliers отличаются по природе. M2.7 дал 3 outliers, преимущественно в caption_accuracy. M3 дал 7 outliers, что указывает не на общий провал, а на другой failure mode: модель чаще помечает спорные визуальные случаи. Для M061 это означает, что outlier queue нужно сохранять как diagnostic artifact, а не автоматически продвигать решения.

## 4. S03 decision: M3 multimodal как production judge

S03 принимает M3 multimodal как production figure QA judge для M061+. Production здесь означает production choice for diagnostic judging, а не разрешение на production import. Graph writes remain false, production import remains false, fact promotion remains false, external network remains disabled by default, and LLM calls remain disabled by default outside scoped diagnostics.

Причины решения:

1. M3 multimodal лучше оценивает визуальную полноту фигур.
2. M3 multimodal лучше оценивает структурную верность диаграмм.
3. M3 multimodal примерно в 3 раза быстрее на 30-figure pilot.
4. 30/30 runs passed для обеих моделей, значит выбор основан не на доступности, а на качестве и стоимости времени.
5. Caption advantage M2.7 полезен, но вторичен для задачи проверки figure layer.

## 5. ADR-014 model selection

ADR-014 фиксирует binding decision: `figure-qa-judge-quality` должен указывать на `minimax-m3-multimodal-anthropic` и использоваться M061+ для figure QA diagnostics. ADR также фиксирует non-authorization: выбор judge-модели не разрешает graph writes, production import, fact promotion или постоянные LLM-вызовы вне явно заданного диагностического scope.

ADR-014 не supersedes ADR-013. ADR-013 остаётся manifest-driven ingest contract, а ADR-014 добавляет модельный выбор для диагностической проверки качества фигур перед будущими ingest-шагами.

## 6. M061 scope: 2-hop BFS with M3 judge integration

Рекомендуемый M061 scope: 2-hop BFS acquisition + parsing + M3 judge integration. Рабочая оценка: 8–10 часов инженерной работы на настройку acquisition, parsing, manifest/replay alignment и включение M3 judge в диагностический контур.

Оценка runtime: 2000–5000 фигур × 8.5 s на фигуру = примерно 5–12 часов wall time при последовательной обработке. Если нужно снизить стоимость и риск первого запуска, альтернативный scope — 10% sample: 200–500 фигур, примерно 30–70 минут модельного времени плюс orchestration overhead.

M061 должен сохранять diagnostic artifacts: per-figure scores, raw model response, parsed scores, latency, outlier flags, prompt version, binding id и safety override audit. Хостовые локальные ссылки — только `127.0.0.1`.

## 7. Lessons: multimodal wins for visual completeness, text-only wins for caption content

Главный урок: multimodal judge выигрывает там, где оценка требует видеть фигуру, а не только читать surrounding text. Для `figure_completeness` и `structural_fidelity` M3 дал более высокий mean score и больше side-by-side побед. Это согласуется с природой задачи: полнота и структура — визуальные свойства.

Второй урок: text-heavy модель может лучше оценивать подпись. M2.7-highspeed выиграл `caption_accuracy`, и это нельзя игнорировать. В M061 стоит сохранять caption-specific diagnostics и рассматривать M2.7 как fallback или audit comparison для спорных caption cases, но не как основной judge.

Третий урок: outliers — это не failure, если они явно сохранены. У M3 больше outliers, но это может быть полезной чувствительностью к неоднозначным визуальным случаям. M061 должен делать outlier queue first-class artifact, а не скрывать её в summary.

## 8. Next milestones: M061, M062, M063

- M061: 2-hop BFS с M3 judge integration, diagnostic-only LLM override, graph writes false, production import false, fact promotion false.
- M062: calibration pass по outlier queue, включая caption-heavy disagreements и thresholds для acceptance/review.
- M063: production-readiness decision после M061/M062 evidence; только после этого можно обсуждать изменение authorization boundary.

До M063 никакой автоматический импорт фактов из judge output не разрешён. Judge output — evidence for review, not source of truth. Любая будущая промоция в граф требует отдельного ADR/requirement update и явного safety gate.
