# M069 S01 T01 Schema Source Notes

## Purpose

Collect primary-source evidence for Agents-K1 schema details relevant to daily-archive. This is not an implementation plan and does not enable graph writes.

## Sources checked

| Source | Status | Local evidence |
|---|---|---|
| arXiv HTML `https://arxiv.org/html/2606.13669v1` | reachable | `source-extracts/*.txt` |
| Appendix D marker `Disaggregated Knowledge Graph Schema` | verified in arXiv HTML | `source-extracts/disaggregated-knowledge-graph-schema-occ2.txt`, `source-extracts/appendix-d-schema-compact.txt` |
| Appendix A marker `Citation Context Classification Schema` | marker found in ToC and extraction snippets | `source-extracts/citation-context-classification*.txt` |
| Appendix B marker `Proofs and Constructive Details for Section 4.5` | marker found in ToC and extraction snippets | `source-extracts/proofs-and-constructive-details*.txt` if extracted by script |
| Section 7 metrics marker `Contain-Acc`, `GPT-Acc` | verified in arXiv HTML | `source-extracts/contain-acc-occ2.txt` |

## Verified schema facts from Appendix D extract

The Appendix D extract is a JSON-like example for "Knowledge Graph Entity and Relation Extraction". It validates that Agents-K1's schema is more than a simple `(subject, predicate, object)` triple list.

### Module A: Meta or factual entities

Verified fields from the extract:

- `A_Meta_Factual_Entities`
- `Paper`
  - `title`
  - `pub_year`
  - `type`
  - `language`
- `Authors`
  - `name`
  - `ordering`
  - `corresponding_flag`

Observed example: the paper `Adam: A Method for Stochastic Optimization` with authors Jimmy Ba and Diederik P. Kingma.

### Module B: Textually mentioned entities

Verified fields from the extract:

- `B_Textually_Mentioned_Entities`
- `Tasks`
  - `name`
  - `type`
  - `input_modality`
  - `output_modality`
  - `constraints`
  - `aliases`
- `Methods`
  - `name`
  - `proposed_or_cited`
  - `components`
  - `training_objectives`
  - `inference_strategies`
  - `aliases`

Observed examples: task `Stochastic Optimization`, method `Adam`, cited methods such as `AdaGrad`.

### Module C: Implicit or abstracted entities

Verified from prose around Figure 6 and Appendix D:

- Module C extracts concepts not necessarily exact keyword matches.
- Example: for the Chain-of-Thought paper, it deduces `involved_task = Symbolic Reasoning`.
- This is an abstraction synthesized from the paper's contribution, not direct string matching.

The compact extraction indicates Module C exists, but the local extract is not yet clean enough to claim a full property list for all Module C entities.

### Module D: Citation relationships

Verified from prose around Figure 6:

- Module D maps explicit citation relationships as `Paper -> Paper`.
- It preserves document-level citation structure.
- Appendix A is named `Citation Context Classification Schema`, but the detailed taxonomy still needs a tighter extraction pass before implementation.

### Module E: Knowledge relations between content entities

Verified from prose around Figure 6:

- Module E captures content-level dependencies as `Content -> Content`.
- Example relation: CoT concept `implements` `few-shot prompting`.
- This confirms that Agents-K1 separates paper-level citations from semantic content-level relations.

## Verified relation groups from saved summary

The saved `PAPER_SUMMARY.md` lists relation groups that should be treated as initially verified by prior source analysis, but they need source-line confirmation before implementation:

- Controlled: `BUILDS_ON`, `USES_COMPONENT`, `ALTERNATIVE_TO`, `SOLVES`, `APPLIED_TO`, `TARGETS`
- Causal: `CAUSES`, `ENABLES`, `INHIBITS`, `MODULATES`, `CORRELATED_WITH`
- Internal composition: `USES_TECHNIQUE`, `CONSISTS_OF`, `IMPLEMENTS`, `COMBINES`, `REQUIRES`
- Methodological comparison: `DERIVED_FROM`, `DIFFERS_FROM`, `HAS_LIMITATION`, `ADDRESSES_PROBLEM`, `MOTIVATED_BY`, `HAS_PROPERTY`, `SUBSET_OF`
- Citation: `CITES`, `SUPPORTS`, `CONTRASTS`, `EXTENDS`

## Unverified or partially verified items

These are important but not safe to implement yet:

- Full Appendix D property schema for Module C, D, and E.
- Exact required vs optional fields.
- Stable identifier generation rules.
- Hyperedge or n-ary relation representation.
- Confidence fields and extractor provenance fields.
- Whether Figure/Table/Equation nodes are first-class in Appendix D or handled through semantic anchors.
- Whether schema is enforced in code or only described in paper examples.
- License/access implications of reusing GraphAnything schema or Scholar-KG artifacts.

## Daily-archive implication

M069 should not copy Agents-K1 directly. The verified actionable takeaway is that daily-archive should separate:

1. paper-level metadata and citations,
2. textually mentioned tasks/methods/datasets/metrics,
3. implicit abstractions such as motivations, hypotheses, mechanisms, limitations,
4. citation-context semantics,
5. content-level relations.

This points to a FalkorDB schema design task, not immediate graph writes.
