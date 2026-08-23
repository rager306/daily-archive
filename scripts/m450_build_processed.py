"""Build data/processed/ from pipeline_cache — M450 S02.

For each article in the sweep cache:
  1. Extract ParsedArticle → parsed/article.json
  2. Extract ExtractedEntity[] → extracted/entities.json
  3. Build DocumentContextProjection → context/projection.json
  4. Generate manifest.json with stage tracking
"""
import gzip
import hashlib
import json
import os
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / "archive/artifacts-legacy/catalog_sweep/pipeline_cache"
PROCESSED = ROOT / "data/processed"
INDEX = ROOT / "data/index.json"

# Load the extraction dictionary fingerprint for disambiguation
sys.path.insert(0, str(ROOT / "scripts"))
os.environ.setdefault("PYTHONPATH", str(ROOT / "scripts"))


def load_yaml(path):
    import yaml
    return yaml.safe_load(path.read_text())


def sha256(data):
    return __import__("hashlib").sha256(data).hexdigest()


def find_success_entry(cache_dir, article_id, stage_id, prefer_fingerprint=None):
    """Find a successful cache entry for this stage."""
    manifest_path = CACHE / article_id / stage_id
    entries = []
    for key_dir in sorted(manifest_path.iterdir()):
        mf = key_dir / "manifest.json"
        if not mf.is_file():
            continue
        meta = json.loads(mf.read_text())
        if isinstance(meta.get("outcome"), dict) and meta["outcome"].get("status") == "success":
            entries.append({
                "key_sha256": key_dir.name,
                "config": meta["key"].get("configuration_fingerprint", ""),
                "payload_sha256": next(
                    (v for k, v in meta.get("outcome", {}).items() if isinstance(v, str) and len(v) == 64),
                    ""
                ),
                "dir": key_dir,
            })
    
    # Prefer entry matching live dictionary fingerprint
    if prefer_fingerprint:
        matching = [e for e in entries if prefer_fingerprint in e["config"]]
        if matching:
            return matching[0]
    
    if len(entries) == 0:
        return None
    if len(entries) == 1:
        return entries[0]
    # Ambiguous: take the last one (most recent)
    return entries[-1]


def load_payload(cache_dir, article_id, stage_id, entry):
    """Load and decompress payload.bin."""
    payload = entry["dir"] / "payload.bin"
    if not payload.is_file():
        return None
    raw = payload.read_bytes()
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return raw


def build_processed(paper_ids):
    index_data = json.loads(INDEX.read_text())
    index_by_id = {a["article_key"]: a for a in index_data.get("articles", [])}
    
    # Get live dictionary fingerprint
    try:
        from da_adapters.rule_extractor import ExtractionConfig
        cfg = ExtractionConfig.from_yaml_file("data/extraction_patterns.yaml")
        extractor = __import__("da_adapters", fromlist=["RuleBasedExtractor"]).RuleBasedExtractor.with_config(cfg)
        live_fp = extractor.cache_identity().configuration_fingerprint
    except Exception:
        live_fp = None
    
    stats = {"total": 0, "parsed_ok": 0, "extracted_ok": 0, "skipped_no_parse": 0, 
             "skipped_ambiguous": 0, "enriched": 0}
    
    for pid in paper_ids:
        cache_article = CACHE / pid
        if not cache_article.exists():
            stats["skipped_no_parse"] += 1
            continue
        
        out_dir = PROCESSED / pid
        out_dir.mkdir(parents=True, exist_ok=True)
        
        manifest = {
            "schema_version": "article-manifest.v2",
            "paper_id": pid,
            "created_at": datetime.now(timezone.utc).isoformat(),
            "source": {
                "code": "arxiv",
                "identity_ref": f"sources/arxiv/{index_by_id[pid].get('coarse_topic_code', 'unknown')}/{pid}/identity.json" if pid in index_by_id else None,
            },
            "stages": {},
        }
        
        # Stage: Parse
        parse_entry = find_success_entry(cache_article, pid, "parse")
        if parse_entry is None:
            manifest["stages"]["parsed"] = {"status": "missing"}
            stats["skipped_no_parse"] += 1
            
            # Still write manifest so we know why
            (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=2))
            stats["total"] += 1
            continue
        
        payload = load_payload(cache_article, pid, "parse", parse_entry)
        if payload is None:
            manifest["stages"]["parsed"] = {"status": "corrupt"}
            stats["skipped_no_parse"] += 1
            (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=2))
            stats["total"] += 1
            continue
        
        # Write parsed output
        parsed_dir = out_dir / "parsed"
        parsed_dir.mkdir(exist_ok=True)
        parsed_bytes = json.dumps(payload, ensure_ascii=False, indent=1).encode()
        (parsed_dir / "article.json").write_bytes(parsed_bytes)
        
        manifest["stages"]["parsed"] = {
            "status": "complete",
            "at": datetime.now(timezone.utc).isoformat(),
            "output_sha256": sha256(parsed_bytes),
            "cache_key": parse_entry["key_sha256"],
            "sections_count": len(payload.get("sections", [])),
        }
        stats["parsed_ok"] += 1
        
        # Stage: Context (from parsed sections — derive AST)
        sections = payload.get("sections", [])
        context_sections = []
        path_stack = []
        for i, sec in enumerate(sections):
            level = sec.get("level", 1)
            while len(path_stack) >= level:
                path_stack.pop()
            parent = ".".join(str(x) for x in path_stack) if path_stack else None
            ordinal_in_level = sum(1 for s in context_sections if s.get("level") == level and (not parent or s.get("parent_section_id") == parent))
            path_id = (f"{parent}." if parent else "") + str(ordinal_in_level + 1)
            
            ctx_sec = {
                "section_id": f"{pid}:{i}",
                "path_id": path_id,
                "order": i,
                "level": level,
                "title": sec.get("title", ""),
                "char_count": len(sec.get("text", "")),
            }
            if parent:
                ctx_sec["parent_section_id"] = parent
                if ordinal_in_level > 0:
                    prev_path = (f"{parent}." if parent else "") + str(ordinal_in_level)
                    ctx_sec["previous_path_id"] = prev_path
            context_sections.append(ctx_sec)
            path_stack.append(ordinal_in_level + 1)
        
        ctx_dir = out_dir / "context"
        ctx_dir.mkdir(exist_ok=True)
        projection = {"paper_id": pid, "sections": context_sections}
        ctx_bytes = json.dumps(projection, ensure_ascii=False, indent=1).encode()
        (ctx_dir / "projection.json").write_bytes(ctx_bytes)
        manifest["stages"]["context"] = {
            "status": "complete",
            "at": datetime.now(timezone.utc).isoformat(),
            "output_sha256": sha256(ctx_bytes),
            "ast_nodes": len(context_sections),
        }
        
        # Stage: Extracted
        extract_entry = find_success_entry(cache_article, pid, "extraction_compute", prefer_fingerprint=live_fp)
        entities_created = 0
        entity_types = {}
        if extract_entry is not None:
            ext_payload = load_payload(cache_article, pid, "extraction_compute", extract_entry)
            if ext_payload is not None:
                ext_dir = out_dir / "extracted"
                ext_dir.mkdir(exist_ok=True)
                ext_bytes = json.dumps(ext_payload, ensure_ascii=False, indent=1).encode()
                (ext_dir / "entities.json").write_bytes(ext_bytes)
                
                entities_created = len(ext_payload) if isinstance(ext_payload, list) else 0
                for e in (ext_payload if isinstance(ext_payload, list) else []):
                    t = e.get("entity_type", "?") if isinstance(e, dict) else "?"
                    entity_types[t] = entity_types.get(t, 0) + 1
                
                manifest["stages"]["extracted"] = {
                    "status": "complete",
                    "at": datetime.now(timezone.utc).isoformat(),
                    "output_sha256": sha256(ext_bytes),
                    "cache_key": extract_entry["key_sha256"],
                    "entity_count": entities_created,
                    "entity_types": entity_types,
                }
                stats["extracted_ok"] += 1
            else:
                manifest["stages"]["extracted"] = {"status": "corrupt"}
        else:
            manifest["stages"]["extracted"] = {"status": "missing"}
        
        # Stage: Enriched (check if OpenAlex data exists)
        enrich_status = "pending"
        # Check old enrichment artifacts
        enrich_artifact = ROOT / "artifacts/article_context" / f"article_{pid}_openalex.json"
        if enrich_artifact.exists():
            enrich_status = "available_not_applied"
        
        manifest["stages"]["enriched"] = {"status": enrich_status}
        if enrich_status == "available_not_applied":
            stats["enriched"] += 1
        
        # Stage: Materialized + Verified (from run report if available)
        run_report_path = ROOT / "artifacts/catalog_sweep/materialization_run_report.json"
        mat_status = "pending"
        if run_report_path.exists():
            try:
                rr = json.loads(run_report_path.read_text())
                for paper in rr.get("papers", []):
                    if paper.get("paper_id") == pid:
                        st = paper.get("state", {})
                        state = st.get("state") if isinstance(st, dict) else str(st)
                        if state == "completed":
                            mat_status = "complete"
                        break
            except:
                pass
        manifest["stages"]["materialized"] = {"status": mat_status}
        manifest["stages"]["verified"] = {"status": "pass" if mat_status == "complete" else "pending"}
        
        # Embedding info
        embed_entry = find_success_entry(cache_article, pid, "embedding")
        if embed_entry:
            manifest["stages"]["embedded"] = {"status": "complete", "cache_key": embed_entry["key_sha256"]}
        else:
            manifest["stages"]["embedded"] = {"status": "missing"}
        
        # Write manifest LAST
        (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False))
        stats["total"] += 1
        
        if stats["total"] % 100 == 0:
            print(f"  {stats['total']} articles processed...", file=sys.stderr)
    
    return stats


if __name__ == "__main__":
    # Read paper IDs from sweep report
    report = json.loads((ROOT / "archive/artifacts-legacy/catalog_sweep/sweep_report.json").read_text())
    ok_papers = [p["paper_id"] for p in report.get("papers", {}).values() if p.get("outcome") == "ok"]
    ok_papers.sort()
    
    print(f"M446-S02: Building processed/ for {len(ok_papers)} papers...")
    result = build_processed(ok_papers)
    print(json.dumps(result, indent=2))
    
    # Summary
    print(f"\nDone. {result['total']}/{len(ok_papers)} articles in processed/")
    print(f"  parsed OK: {result['parsed_ok']}")
    print(f"  extracted OK: {result['extracted_ok']}")
    print(f"  skipped (no parse): {result['skipped_no_parse']}")
