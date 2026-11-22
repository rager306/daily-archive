//! Separate ordered-token diagnostic; does not recapture or reclassify frozen body evidence.
use clap::Parser;
use da_parse::native_exact_alignment::{
    ExactAlignmentBudget, ExactAlignmentOutcome, NATIVE_EXACT_ALIGNMENT_SCHEMA_VERSION,
    align_exact_tokens,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, fs, path::PathBuf, process::ExitCode};

#[derive(Parser)]
struct Args {
    /// UTF-8 inventory: one already-normalized token per line, with no implicit normalization.
    #[arg(long)]
    oracle: PathBuf,
    #[arg(long)]
    side: PathBuf,
    /// Matcher work only; excludes file reading, collision-free interning, U and H.
    #[arg(long)]
    max_work: u64,
    #[arg(long)]
    out: Option<PathBuf>,
}

fn tokens(text: &str) -> Result<Vec<&str>, Box<dyn Error>> {
    let tokens: Vec<_> = text.lines().collect();
    if tokens.iter().any(|token| token.is_empty()) {
        return Err("empty token line: declare a non-empty unit per line".into());
    }
    Ok(tokens)
}

fn intern(
    tokens: &[&str],
    vocabulary: &mut BTreeMap<String, u32>,
) -> Result<Vec<u32>, Box<dyn Error>> {
    tokens
        .iter()
        .map(|token| {
            if let Some(&id) = vocabulary.get(*token) {
                return Ok(id);
            }
            let id =
                u32::try_from(vocabulary.len()).map_err(|_| "too many distinct token units")?;
            vocabulary.insert((*token).to_owned(), id);
            Ok(id)
        })
        .collect()
}

fn count_units(tokens: &[u32]) -> BTreeMap<u32, usize> {
    let mut counts = BTreeMap::new();
    for &token in tokens {
        *counts.entry(token).or_default() += 1;
    }
    counts
}

fn multiset_ceiling(left: &[u32], right: &[u32]) -> usize {
    let left = count_units(left);
    let right = count_units(right);
    left.iter()
        .map(|(token, &count)| count.min(*right.get(token).unwrap_or(&0)))
        .sum()
}

fn greedy_count(left: &[u32], right: &[u32]) -> usize {
    let mut occurrences = BTreeMap::<u32, Vec<usize>>::new();
    for (index, &token) in right.iter().enumerate() {
        occurrences.entry(token).or_default().push(index);
    }
    let mut cursor = 0;
    let mut count = 0;
    for token in left {
        if let Some(indices) = occurrences.get(token) {
            let at = indices.partition_point(|&index| index < cursor);
            if let Some(&index) = indices.get(at) {
                count += 1;
                cursor = index + 1;
            }
        }
    }
    count
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let oracle_bytes = fs::read(&args.oracle)?;
    let side_bytes = fs::read(&args.side)?;
    let oracle_tokens = tokens(std::str::from_utf8(&oracle_bytes)?)?;
    let side_tokens = tokens(std::str::from_utf8(&side_bytes)?)?;
    let mut vocabulary = BTreeMap::new();
    let oracle = intern(&oracle_tokens, &mut vocabulary)?;
    let side = intern(&side_tokens, &mut vocabulary)?;
    let u = multiset_ceiling(&oracle, &side);
    let h = greedy_count(&oracle, &side);
    let outcome = align_exact_tokens(
        &oracle,
        &side,
        ExactAlignmentBudget {
            max_work: args.max_work,
        },
    );
    let l = match &outcome {
        ExactAlignmentOutcome::Complete { pairs, .. } => Some(pairs.len()),
        ExactAlignmentOutcome::NotAssessable { .. } => None,
    };
    if let Some(l) = l
        && (h > l || l > u)
    {
        return Err("invalid ordered-match bounds: expected H <= L <= U".into());
    }
    let report = json!({
        "schema_version": NATIVE_EXACT_ALIGNMENT_SCHEMA_VERSION,
        "claim_scope": "separate_ordered_token_diagnostic_not_physical_occurrence_identity",
        "normalization": "caller_declared_utf8_line_tokens_no_implicit_normalization",
        "matcher_budget_scope": "matcher_only_excludes_read_intern_multiset_greedy",
        "max_work": args.max_work,
        "oracle": {"sha256": format!("{:x}", Sha256::digest(&oracle_bytes)), "units": oracle.len()},
        "side": {"sha256": format!("{:x}", Sha256::digest(&side_bytes)), "units": side.len()},
        "multiset_ceiling_u": u,
        "valid_greedy_h": h,
        "exact_lcs_l": l,
        "unexplained_greedy_gap_u_minus_h": u - h,
        "order_incompatibility_u_minus_l": l.map(|l| u - l),
        "outcome": outcome,
        "frozen_body_evidence_changed": false,
        "import_eligible": false,
        "deployed": false,
        "auto_promote": "forbidden",
        "graph_write": "forbidden"
    });
    let output = serde_json::to_string_pretty(&report)? + "\n";
    if let Some(out) = args.out {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out)?;
        std::io::Write::write_all(&mut file, output.as_bytes())?;
    } else {
        print!("{output}");
    }
    Ok(())
}

fn main() -> ExitCode {
    let _guard = da_cli::logging::init_pipeline_logging("native_exact_alignment");
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("native_exact_alignment: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_exact_order_from_multiset_and_valid_greedy() {
        let left = [0, 1, 2];
        let right = [1, 2, 0];
        assert_eq!(multiset_ceiling(&left, &right), 3);
        assert_eq!(greedy_count(&left, &right), 1);
    }

    #[test]
    fn duplicate_units_keep_their_multiplicity() {
        assert_eq!(multiset_ceiling(&[0, 0, 0, 1], &[0, 1, 1]), 2);
        assert_eq!(greedy_count(&[0, 0, 0, 1], &[0, 1, 1]), 2);
    }

    #[test]
    fn line_contract_does_not_normalize_token_content() {
        assert_eq!(tokens("  Alpha\nβ\n").unwrap(), ["  Alpha", "β"]);
        assert!(tokens("alpha\n\nbeta\n").is_err());
    }
}
