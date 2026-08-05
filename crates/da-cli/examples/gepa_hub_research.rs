//! GEPA hub (2507.19457) bibliography research on process-local Samyama (M318–M323).
//!
//! - Parses hub + cited arXiv PDFs via GROBID
//! - Ingests with **kg_embeddings::MockEmbedder** (no FD_API auth)
//! - Reports citation/Reference/CITES/hasPart connectivity
//! - Inventory via `collect_graph_inventory` (registered_node_labels + edge histogram)
//! - Graph `section_nodes` from ontology inventory — never hub TEI section Vec (M320/M323)
//! - Exports snapshot + JSON/MD under artifacts/gepa_hub_research/
//!
//! D127: import_eligible stays false.
//!
//! Usage:
//!   cargo run -p da-cli --example gepa_hub_research
//!   cargo run -p da-cli --example gepa_hub_research -- --ids-file artifacts/gepa_hub_research/ingest_ids.txt

use da_adapters::{GrobidParser, RuleBasedExtractor, SamyamaGraphStore};
use da_application::{IngestUseCase, batch_ingest_pdfs};
use da_ports::extractor::Extractor;
use da_ports::parser::ParserPort;
use kg_embeddings::mock::MockEmbedder;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct ResearchReport {
    hub_id: String,
    papers_requested: usize,
    papers_found_pdf: usize,
    ingest: IngestSummary,
    graph: GraphSummary,
    hub_extract: HubExtractSummary,
    bibliography: BibliographySummary,
    import_eligible: bool,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct IngestSummary {
    ok: usize,
    fail: usize,
    total_citations: usize,
    total_references_written: usize,
    total_cites_resolved: usize,
    total_sections: usize,
    errors: Vec<(String, String)>,
    snapshot_path: Option<String>,
}

#[derive(Serialize)]
struct GraphSummary {
    paper_nodes: usize,
    reference_nodes: usize,
    section_nodes: usize,
    category_nodes: usize,
    citation_nodes: usize,
    entity_nodes: usize,
    source_nodes: usize,
    total_nodes: usize,
    /// Edge histogram by ontology string (e.g. `hasPart`, `CITES`, `inCategory`).
    edge_type_histogram: HashMap<String, usize>,
    cites_edges: usize,
    has_part_edges: usize,
    from_source_edges: usize,
    in_category_edges: usize,
    mentions_edges: usize,
    paper_ids_in_graph: Vec<String>,
}

#[derive(Serialize)]
struct HubExtractSummary {
    entities: usize,
    by_type: HashMap<String, usize>,
    labels_sample: Vec<String>,
}

#[derive(Serialize)]
struct BibliographySummary {
    hub_citations_from_tei: usize,
    hub_citations_with_arxiv: usize,
    hub_citations_with_doi: usize,
    cited_arxiv_ids: Vec<String>,
    cited_arxiv_in_catalog: usize,
    cited_arxiv_ingested_ok: usize,
}

#[derive(serde::Deserialize)]
struct CitationsFile {
    #[allow(dead_code)]
    paper: String,
    entries: Vec<CitationEntryJson>,
    arxiv_ids: Vec<String>,
}

#[derive(serde::Deserialize)]
struct CitationEntryJson {
    arxiv_id: Option<String>,
    #[allow(dead_code)]
    title: Option<String>,
    doi: Option<String>,
}

fn find_pdf(paper_id: &str) -> Option<String> {
    let o = std::process::Command::new("find")
        .args(["data/article_catalog", "-name", &format!("{paper_id}.pdf")])
        .output()
        .ok()?;
    let s = String::from_utf8(o.stdout).ok()?;
    let t = s.trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}

fn load_ids(args: &[String]) -> Vec<String> {
    let mut ids = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--ids-file" && i + 1 < args.len() {
            let text = std::fs::read_to_string(&args[i + 1]).expect("ids-file");
            for line in text.lines() {
                let l = line.trim();
                if !l.is_empty() && !l.starts_with('#') {
                    ids.push(l.to_string());
                }
            }
            i += 2;
        } else if !args[i].starts_with('-') {
            ids.push(args[i].clone());
            i += 1;
        } else {
            i += 1;
        }
    }
    if ids.is_empty() {
        let default = Path::new("artifacts/gepa_hub_research/ingest_ids.txt");
        if default.exists() {
            for line in std::fs::read_to_string(default).unwrap().lines() {
                let l = line.trim();
                if !l.is_empty() {
                    ids.push(l.to_string());
                }
            }
        } else {
            ids.push("2507.19457".into());
        }
    }
    ids
}

fn load_bibliography() -> BibliographySummary {
    let mut bib = BibliographySummary {
        hub_citations_from_tei: 0,
        hub_citations_with_arxiv: 0,
        hub_citations_with_doi: 0,
        cited_arxiv_ids: vec![],
        cited_arxiv_in_catalog: 0,
        cited_arxiv_ingested_ok: 0,
    };
    let path = Path::new("artifacts/gepa_hub_research/citations.json");
    if !path.exists() {
        return bib;
    }
    let cf: CitationsFile = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    bib.hub_citations_from_tei = cf.entries.len();
    bib.hub_citations_with_arxiv = cf.entries.iter().filter(|e| e.arxiv_id.is_some()).count();
    bib.hub_citations_with_doi = cf.entries.iter().filter(|e| e.doi.is_some()).count();
    bib.cited_arxiv_ids = cf.arxiv_ids;
    bib.cited_arxiv_ids.sort();
    bib.cited_arxiv_in_catalog = bib
        .cited_arxiv_ids
        .iter()
        .filter(|a| find_pdf(a).is_some())
        .count();
    bib
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let ids = load_ids(&args);
    let hub_id = "2507.19457".to_string();
    println!("GEPA hub research: {} paper ids (hub={hub_id})", ids.len());

    let parser = GrobidParser::from_env();
    if !parser.is_alive().await {
        anyhow::bail!("GROBID not alive at default endpoint");
    }

    let mut bib = load_bibliography();
    println!(
        "Hub bibliography (from citations.json): {} cites (arxiv={} doi={}) catalog={}/{}",
        bib.hub_citations_from_tei,
        bib.hub_citations_with_arxiv,
        bib.hub_citations_with_doi,
        bib.cited_arxiv_in_catalog,
        bib.cited_arxiv_ids.len()
    );

    let mut pdfs: Vec<(String, String)> = Vec::new();
    for id in &ids {
        if let Some(p) = find_pdf(id) {
            pdfs.push((p, id.clone()));
        } else {
            eprintln!("  PDF missing for {id}");
        }
    }
    println!("PDFs found: {}/{}", pdfs.len(), ids.len());

    // MockEmbedder: deterministic, no FD_API (research path when key is 401).
    let store = SamyamaGraphStore::new();
    let ingest = IngestUseCase::new(
        Box::new(parser),
        Box::new(MockEmbedder::new(1024)),
        Box::new(store),
    );

    let snap = PathBuf::from("artifacts/gepa_hub_research/hub_and_refs_research.sgsnap");
    let batch = batch_ingest_pdfs(&ingest, &pdfs, Some(&snap)).await?;
    println!(
        "Ingest: ok={} fail={} citations={} references_written={} cites_resolved={}",
        batch.ok,
        batch.fail,
        batch.total_citations,
        batch.total_references_written,
        batch.total_cites_resolved
    );
    for (id, e) in batch.errors.iter().take(10) {
        eprintln!("  fail {id}: {e}");
    }

    // Ontology-driven inventory (domain registered labels + edge histogram).
    // Capture *before* hub extract so TEI locals cannot shadow counts (M320).
    use da_application::collect_graph_inventory;
    use da_domain::relation::{bibliographic, structure};

    let inv = collect_graph_inventory(ingest.graph_store.as_ref()).await;
    let paper_node_count = inv.nodes("Paper");
    let reference_node_count = inv.nodes("Reference");
    let section_node_count = inv.nodes("Section");
    let category_node_count = inv.nodes("Category");
    let citation_node_count = inv.nodes("Citation");
    let entity_node_count = inv.nodes("Entity");
    let source_node_count = inv.nodes("Source");
    let total_node_count = inv.total_nodes;

    let papers = ingest.graph_store.get_nodes_by_label("Paper").await;
    let mut paper_ids = Vec::new();
    for nid in &papers {
        if let Some(aid) = ingest
            .graph_store
            .get_node_property_string(*nid, "arxiv_id")
            .await
        {
            paper_ids.push(aid);
        }
    }
    paper_ids.sort();

    let edge_hist: HashMap<String, usize> = inv
        .edge_type_histogram
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    let cites = inv.edges(bibliographic::CITES);
    let has_part = inv.edges(structure::HAS_PART);
    let from_source = inv.edges(structure::FROM_SOURCE);
    let in_category = inv.edges(structure::IN_CATEGORY);
    let mentions = inv.edges(bibliographic::MENTIONS);
    println!(
        "Edge histogram (ontology strings): cites={cites} hasPart={has_part} FROM_SOURCE={from_source} inCategory={in_category} MENTIONS={mentions} full={edge_hist:?}"
    );
    println!(
        "Node inventory: Paper={paper_node_count} Reference={reference_node_count} Section={section_node_count} (ingest total_sections={}) Category={category_node_count} Citation={citation_node_count} Source={source_node_count} (ontology labels total_nodes={total_node_count})",
        batch.total_sections,
    );
    // Reference: references_written counts every hasPart link; unique nodes dedupe by vid.
    if batch.total_references_written > reference_node_count {
        println!(
            "Note: Reference unique nodes ({reference_node_count}) < references_written ({}) — expected vid dedup across papers",
            batch.total_references_written
        );
    }
    if section_node_count != batch.total_sections && batch.total_sections > section_node_count {
        eprintln!(
            "WARN: get_nodes_by_label(Section) undercount: label_index={section_node_count} vs ingest.total_sections={} — investigate Samyama/adapter",
            batch.total_sections
        );
    }
    if !inv.unknown_edge_types().is_empty() {
        eprintln!(
            "WARN: unknown edge types not in edge_contracts: {:?}",
            inv.unknown_edge_types()
        );
    }

    let paper_set: HashSet<_> = paper_ids.iter().cloned().collect();
    bib.cited_arxiv_ingested_ok = bib
        .cited_arxiv_ids
        .iter()
        .filter(|a| paper_set.contains(*a))
        .count();

    // Hub extract — MUST NOT reuse name `sections` (shadows graph Section node IDs).
    let hub_pdf = find_pdf(&hub_id).expect("hub pdf");
    let parser2 = GrobidParser::from_env();
    let parsed = parser2.parse_pdf(&hub_pdf, &hub_id).await?;
    let hub_sections: Vec<(String, String)> = parsed
        .sections
        .iter()
        .map(|s| (s.title.clone(), s.text.clone()))
        .collect();
    let hub_tei_section_count = hub_sections.len();
    let extractor = RuleBasedExtractor::new();
    let extracted = extractor.extract(&hub_sections).await?;
    let mut by_type: HashMap<String, usize> = HashMap::new();
    for e in &extracted {
        *by_type.entry(format!("{:?}", e.entity_type)).or_default() += 1;
    }
    let labels_sample: Vec<String> = extracted.iter().take(20).map(|e| e.label.clone()).collect();
    println!(
        "Hub TEI sections for extract: {hub_tei_section_count} (must not be used as graph section_nodes)"
    );

    let report = ResearchReport {
        hub_id: hub_id.clone(),
        papers_requested: ids.len(),
        papers_found_pdf: pdfs.len(),
        ingest: IngestSummary {
            ok: batch.ok,
            fail: batch.fail,
            total_citations: batch.total_citations,
            total_references_written: batch.total_references_written,
            total_cites_resolved: batch.total_cites_resolved,
            total_sections: batch.total_sections,
            errors: batch.errors,
            snapshot_path: batch.snapshot_path,
        },
        graph: GraphSummary {
            paper_nodes: paper_node_count,
            reference_nodes: reference_node_count,
            section_nodes: section_node_count,
            category_nodes: category_node_count,
            citation_nodes: citation_node_count,
            entity_nodes: entity_node_count,
            source_nodes: source_node_count,
            total_nodes: total_node_count,
            edge_type_histogram: edge_hist.clone(),
            cites_edges: cites,
            has_part_edges: has_part,
            from_source_edges: from_source,
            in_category_edges: in_category,
            mentions_edges: mentions,
            paper_ids_in_graph: paper_ids.clone(),
        },
        hub_extract: HubExtractSummary {
            entities: extracted.len(),
            by_type: by_type.clone(),
            labels_sample: labels_sample.clone(),
        },
        bibliography: bib,
        import_eligible: false,
        notes: vec![
            "Research embedder: kg_embeddings::MockEmbedder (FD_API 401 avoided).".into(),
            "CITES resolves only when cited arxiv_id matches an ingested Paper.".into(),
            "All bibliography entries become Reference nodes (references_written).".into(),
            "Reference unique nodes may be < references_written (vid dedup across papers).".into(),
            "Edge inventory uses ontology wire strings (hasPart not HAS_PART) — M319.".into(),
            "section_nodes is get_nodes_by_label(Section), not hub TEI section count — M320."
                .into(),
            "MENTIONS=0 is expected: ingest does not write Entity MENTIONS edges.".into(),
            "D127 import_eligible=false.".into(),
        ],
    };

    let out_dir = Path::new("artifacts/gepa_hub_research");
    std::fs::create_dir_all(out_dir)?;
    let json_path = out_dir.join("engine_research.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&report)? + "\n")?;
    println!("Wrote {}", json_path.display());

    let md = format!(
        r#"# GEPA hub research — 2507.19457

## Hub bibliography (GROBID TEI / citations.json)

| Metric | Value |
|--------|--------|
| Citations in TEI | {cites} |
| With arXiv id | {arxiv} |
| With DOI | {doi} |
| Cited arXiv in catalog | {cat}/{arxiv_total} |
| Cited arXiv ingested as Paper | {ing} |

## Ingest (process-local, mock embedder)

| Metric | Value |
|--------|--------|
| PDFs found | {pdfs} |
| Ingest OK | {ok} |
| Ingest fail | {fail} |
| Total citations parsed | {tc} |
| Reference nodes written | {rw} |
| CITES resolved to Paper | {cr} |
| Snapshot | `{snap}` |

## Graph inventory (M319 ontology-string honest counts)

| Metric | Value |
|--------|--------|
| Paper nodes | {pn} |
| Reference nodes | {rn} |
| Section nodes | {sec} |
| Category nodes | {catn} |
| Citation nodes | {citn} |
| Entity nodes | {en} |
| Source nodes | {sn} |
| Total nodes | {tn} |
| CITES edges | {ce} |
| hasPart edges | {hp} |
| FROM_SOURCE edges | {fs} |
| inCategory edges | {ic} |
| MENTIONS edges | {me} |
| Edge histogram | `{hist}` |

## Hub extraction (RuleBasedExtractor)

- Entities: **{he}**
- By type: `{bt}`
- Sample labels: {ls}

## Papers in graph

{plist}

## Notes

- Research used **MockEmbedder** (FD_API returned 401 with current key).
- Structural graph (Paper/Reference/CITES/hasPart) is valid for connectivity research.
- Edge inventory matches ontology wire strings (`hasPart`, not Rust name `HAS_PART`).
- MENTIONS=0 expected: ingest does not write Entity MENTIONS (extract-only path).
- Vectors are **not** production TEI embeddings.
- `import_eligible=false` (D127).

## Reproduce

```bash
python3 scripts/download_arxiv_to_catalog.py --ids-file artifacts/gepa_hub_research/missing_arxiv.txt --skip-meta
cargo test -p da-adapters --lib real_tei -- --test-threads=1
cargo test -p da-domain --lib relation -- --test-threads=1
cargo run -p da-cli --example gepa_hub_research
```
"#,
        cites = report.bibliography.hub_citations_from_tei,
        arxiv = report.bibliography.hub_citations_with_arxiv,
        doi = report.bibliography.hub_citations_with_doi,
        cat = report.bibliography.cited_arxiv_in_catalog,
        arxiv_total = report.bibliography.cited_arxiv_ids.len(),
        ing = report.bibliography.cited_arxiv_ingested_ok,
        pdfs = report.papers_found_pdf,
        ok = report.ingest.ok,
        fail = report.ingest.fail,
        tc = report.ingest.total_citations,
        rw = report.ingest.total_references_written,
        cr = report.ingest.total_cites_resolved,
        snap = report.ingest.snapshot_path.as_deref().unwrap_or("none"),
        pn = report.graph.paper_nodes,
        rn = report.graph.reference_nodes,
        sec = report.graph.section_nodes,
        catn = report.graph.category_nodes,
        citn = report.graph.citation_nodes,
        en = report.graph.entity_nodes,
        sn = report.graph.source_nodes,
        tn = report.graph.total_nodes,
        ce = report.graph.cites_edges,
        hp = report.graph.has_part_edges,
        fs = report.graph.from_source_edges,
        ic = report.graph.in_category_edges,
        me = report.graph.mentions_edges,
        hist = format!("{:?}", report.graph.edge_type_histogram),
        he = report.hub_extract.entities,
        bt = format!("{:?}", report.hub_extract.by_type),
        ls = report.hub_extract.labels_sample.join(", "),
        plist = report
            .graph
            .paper_ids_in_graph
            .iter()
            .map(|s| format!("- `{s}`"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let md_path = out_dir.join("REPORT.md");
    std::fs::write(&md_path, md)?;
    println!("Wrote {}", md_path.display());
    println!(
        "\nDONE hub_cites={} refs_written={} papers={} cites_resolved={}",
        report.bibliography.hub_citations_from_tei,
        report.ingest.total_references_written,
        report.graph.paper_nodes,
        report.ingest.total_cites_resolved
    );
    Ok(())
}
