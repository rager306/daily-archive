# Thirty-paper corpus availability report

## Summary

M006 selected a 30-paper deviation-scan corpus to test whether M005's 10-paper conclusions hold on a broader local sample. The corpus includes all 10 M005 gold-corpus papers for direct baseline overlap and 20 deterministic expansion papers discovered from local research/cache evidence.

The first deviation is source availability, not chunk behavior: all 30 selected papers have local research workspaces and paper metadata, but only 10 have available Markdown source artifacts and only 2 have cached PDFs. The 20 expansion papers are therefore not yet ready for a meaningful Markdown/chunking dry run unless M006 adds a source acquisition/conversion step or reselects expansion papers from a source-complete subset.

## Counts

| Metric | Count |
|---|---:|
| Total selected papers | 30 |
| M005 overlap | 10 |
| Expansion papers | 20 |
| Research workspace present | 30 |
| Paper metadata present | 30 |
| Available Markdown | 10 |
| Cached PDF | 2 |
| Blocked missing Markdown | 20 |
| Missing PDF | 28 |

## M005 overlap

The full M005 10-paper corpus is included as baseline overlap. This preserves direct comparison against M005 benchmark and negative import-boundary evidence.

## Expansion finding

The deterministic 20-paper expansion is metadata/workspace-rich but source-poor. That means a naive S02 chunking run over all 30 would mostly rediscover missing-source blockers instead of measuring chunking/import-model deviations.

This is still useful: it shows that broader validation cannot assume local paper metadata implies full-text readiness. Source acquisition/conversion must be part of any real 30-paper or larger scan.

## Recommendation for S02

Do not claim a full 30-paper chunking/import scan yet. The next slice should either:

1. add a bounded source acquisition/conversion bridge for the 20 missing-Markdown expansion papers, then run the 30-paper evidence path; or
2. intentionally run a partial 30-paper availability/deviation scan that reports 20 source blockers and only computes chunking metrics for the 10 Markdown-ready papers.

Option 1 is better if the goal is truly to discover chunking/import-model deviations on 30 papers. Option 2 is faster but mostly measures source availability, not chunk quality.

## Safety boundary

This S01 audit did not perform KG import, production LadybugDB writes, embedding generation, vector generation, or raw text/chunk text serialization in machine artifacts.

## Evidence

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`
- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json`
- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-diagnostics.jsonl`
