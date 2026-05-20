# DSPy optimizer applicability catalog

## Summary

- Optimizers/classes inventoried: `19`
- Rating counts: `{'not-applicable-now': 8, 'blocked': 3, 'future-only': 6, 'possible-dev': 2}`
- No optimizer was run.
- Positive KG import remains blocked.

## Ratings

### AvatarOptimizer

- Source: `dspy/teleprompt/avatar_optimizer.py`
- Algorithm family: `support_or_internal_class`
- Applicability: `not-applicable-now`
- Rationale: Internal/support class or not directly applicable as a project optimizer.

### BetterTogether

- Source: `dspy/teleprompt/bettertogether.py`
- Algorithm family: `multi_strategy_optimizer_orchestrating_multiple_optimizers`
- Applicability: `blocked`
- Rationale: Too broad/high-risk for current project; would combine optimizers before individual safety is proven.

### BootstrapFewShot

- Source: `dspy/teleprompt/bootstrap.py`
- Algorithm family: `bootstrap_demos_from_teacher_student_traces`
- Applicability: `future-only`
- Rationale: Potentially useful after a trusted metric/devset exists, but it runs programs to create traces and can leak raw content if not redacted.

### BootstrapFinetune

- Source: `dspy/teleprompt/bootstrap_finetune.py`
- Algorithm family: `finetuning_from_bootstrapped_data`
- Applicability: `blocked`
- Rationale: Finetuning is out of scope before trusted labeled data, data policy, and model governance.

### COPRO

- Source: `dspy/teleprompt/copro_optimizer.py`
- Algorithm family: `instruction_prompt_optimization`
- Applicability: `future-only`
- Rationale: Could tune instructions but needs metric/devset and risks prompt overfit; no current use without span metrics.

### Ensemble

- Source: `dspy/teleprompt/ensemble.py`
- Algorithm family: `ensemble_program_outputs`
- Applicability: `not-applicable-now`
- Rationale: Might combine multiple safe programs later, but no validated candidate programs exist yet.

### GEPAFeedbackMetric

- Source: `dspy/teleprompt/gepa/gepa.py`
- Algorithm family: `support_or_internal_class`
- Applicability: `not-applicable-now`
- Rationale: Internal/support class or not directly applicable as a project optimizer.

### DspyGEPAResult

- Source: `dspy/teleprompt/gepa/gepa.py`
- Algorithm family: `support_or_internal_class`
- Applicability: `not-applicable-now`
- Rationale: Internal/support class or not directly applicable as a project optimizer.

### GEPA

- Source: `dspy/teleprompt/gepa/gepa.py`
- Algorithm family: `reflective_prompt_evolution`
- Applicability: `blocked`
- Rationale: Experimental/trace-heavy reflective optimizer; inappropriate until redaction/tracing policy and metrics are mature.

### GRPO

- Source: `dspy/teleprompt/grpo.py`
- Algorithm family: `support_or_internal_class`
- Applicability: `not-applicable-now`
- Rationale: Internal/support class or not directly applicable as a project optimizer.

### InferRules

- Source: `dspy/teleprompt/infer_rules.py`
- Algorithm family: `support_or_internal_class`
- Applicability: `not-applicable-now`
- Rationale: Internal/support class or not directly applicable as a project optimizer.

### KNNFewShot

- Source: `dspy/teleprompt/knn_fewshot.py`
- Algorithm family: `retrieval_based_demo_selection`
- Applicability: `possible-dev`
- Rationale: Most plausible early option after labeled examples exist; can select examples without compiling prompts, but still must avoid raw text leakage.

### MIPROv2

- Source: `dspy/teleprompt/mipro_optimizer_v2.py`
- Algorithm family: `bayesian_optimization_of_instructions_and_demos`
- Applicability: `future-only`
- Rationale: Potentially relevant for extraction prompts after strong metrics; high LM-call/cost/trace risk; not first optimizer.

### BootstrapFewShotWithRandomSearch

- Source: `dspy/teleprompt/random_search.py`
- Algorithm family: `random_search_over_bootstrapped_demos`
- Applicability: `future-only`
- Rationale: Could optimize demo selection but cost/trace risk is higher; needs budget caps and redacted devset.

### SignatureOptimizer

- Source: `dspy/teleprompt/signature_opt.py`
- Algorithm family: `support_or_internal_class`
- Applicability: `not-applicable-now`
- Rationale: Internal/support class or not directly applicable as a project optimizer.

### SIMBA

- Source: `dspy/teleprompt/simba.py`
- Algorithm family: `stochastic_instruction_demo_optimization`
- Applicability: `future-only`
- Rationale: Could be explored later for prompt/demo improvement; requires devset, metrics, budget caps.

### Teleprompter

- Source: `dspy/teleprompt/teleprompt.py`
- Algorithm family: `support_or_internal_class`
- Applicability: `not-applicable-now`
- Rationale: Internal/support class or not directly applicable as a project optimizer.

### BootstrapFewShotWithOptuna

- Source: `dspy/teleprompt/teleprompt_optuna.py`
- Algorithm family: `optuna_search_over_bootstrapped_demos`
- Applicability: `future-only`
- Rationale: Search-heavy; only after deterministic metrics and isolated dev environment.

### LabeledFewShot

- Source: `dspy/teleprompt/vanilla.py`
- Algorithm family: `demo_selection_from_labeled_examples`
- Applicability: `possible-dev`
- Rationale: Could select fixed labeled demonstrations after chunk-span/candidate locator fixtures exist; low optimizer risk but still needs devset/metrics.
