//! Independent tiny oracle: enumerate increasing index subsets on both sides.
//! It does not use the matcher's frontier, reconstruction, or a DP recurrence.

use da_parse::{
    ExactAlignmentBudget, ExactAlignmentIncompleteReason, ExactAlignmentOutcome,
    ExactAlignmentWorkKind, ExactTokenPair, align_exact_tokens,
};

fn increasing_subsets(len: usize, size: usize) -> Vec<Vec<usize>> {
    fn visit(
        len: usize,
        size: usize,
        next: usize,
        current: &mut Vec<usize>,
        output: &mut Vec<Vec<usize>>,
    ) {
        if current.len() == size {
            output.push(current.clone());
            return;
        }
        for index in next..len {
            current.push(index);
            visit(len, size, index + 1, current, output);
            current.pop();
        }
    }
    let mut output = Vec::new();
    visit(len, size, 0, &mut Vec::new(), &mut output);
    output
}

fn exhaustive_index_pair_oracle(oracle: &[u32], side: &[u32]) -> Vec<ExactTokenPair> {
    for size in (0..=oracle.len().min(side.len())).rev() {
        for left in increasing_subsets(oracle.len(), size) {
            for right in increasing_subsets(side.len(), size) {
                if left.iter().zip(&right).all(|(&a, &b)| oracle[a] == side[b]) {
                    return left
                        .into_iter()
                        .zip(right)
                        .map(|(oracle_index, side_index)| ExactTokenPair {
                            oracle_index,
                            side_index,
                        })
                        .collect();
                }
            }
        }
    }
    unreachable!("the two empty subsets always form a legal matching")
}

fn complete(oracle: &[u32], side: &[u32]) -> Vec<ExactTokenPair> {
    let outcome = align_exact_tokens(oracle, side, ExactAlignmentBudget { max_work: u64::MAX });
    let ExactAlignmentOutcome::Complete { pairs, work } = outcome else {
        panic!("tiny input must complete: {outcome:?}");
    };
    assert_eq!(
        work.total,
        work.frontier_steps
            + work.equality_comparisons
            + work.scratch_cells
            + work.reconstruction_steps
    );
    assert_witness(oracle, side, &pairs);
    pairs
}

fn assert_witness(oracle: &[u32], side: &[u32], pairs: &[ExactTokenPair]) {
    for pair in pairs {
        assert!(pair.oracle_index < oracle.len());
        assert!(pair.side_index < side.len());
        assert_eq!(oracle[pair.oracle_index], side[pair.side_index]);
    }
    assert!(pairs.windows(2).all(|pair| {
        pair[0].oracle_index < pair[1].oracle_index && pair[0].side_index < pair[1].side_index
    }));
}

fn sequences(alphabet: u32, max_len: usize) -> Vec<Vec<u32>> {
    let mut result = vec![Vec::new()];
    let mut layer = vec![Vec::new()];
    for _ in 0..max_len {
        let mut next = Vec::new();
        for prefix in &layer {
            for token in 0..alphabet {
                let mut sequence = prefix.clone();
                sequence.push(token);
                next.push(sequence);
            }
        }
        result.extend(next.clone());
        layer = next;
    }
    result
}

#[test]
fn every_small_binary_sequence_agrees_with_exhaustive_index_pair_enumeration() {
    let inputs = sequences(2, 5);
    for oracle in &inputs {
        for side in &inputs {
            let witness = complete(oracle, side);
            let independent = exhaustive_index_pair_oracle(oracle, side);
            assert_eq!(witness.len(), independent.len(), "{oracle:?} / {side:?}");
        }
    }
}

#[test]
fn small_ternary_permutations_and_duplicates_agree_with_exhaustive_oracle() {
    let inputs = sequences(3, 3);
    for oracle in &inputs {
        for side in &inputs {
            assert_eq!(
                complete(oracle, side).len(),
                exhaustive_index_pair_oracle(oracle, side).len(),
                "{oracle:?} / {side:?}"
            );
        }
    }
}

#[test]
fn a_greedy_counterexample_has_two_exact_bindings_not_one_or_three() {
    // Oracle [a,b,c], side [b,c,a]: H=1, L=2, U=3. Only L is the
    // matcher's result; neither U nor the frozen greedy result is a body verdict.
    let pairs = complete(&[0, 1, 2], &[1, 2, 0]);
    assert_eq!(
        pairs,
        vec![
            ExactTokenPair {
                oracle_index: 1,
                side_index: 0,
            },
            ExactTokenPair {
                oracle_index: 2,
                side_index: 1,
            },
        ]
    );
    assert_eq!(complete(&[0, 0], &[0]).len(), 1);
    assert_eq!(complete(&[0, 1, 0, 1], &[1, 0, 1, 0]).len(), 3);
    assert_eq!(complete(&[0, 1, 2], &[2, 1, 0]).len(), 1);
}

#[test]
fn exact_only_matching_never_invents_a_split_merge_or_substring_pair() {
    // IDs denote different declared whole units: x, y, xy, he, the.
    assert_eq!(complete(&[0, 1], &[0, 2]).len(), 1);
    assert!(complete(&[3], &[4]).is_empty());
    assert!(complete(&[0, 1], &[2]).is_empty());
}

#[test]
fn a_zero_budget_is_incomplete_for_nonempty_inputs_but_empty_sides_are_exact() {
    let zero = ExactAlignmentBudget { max_work: 0 };
    let outcome = align_exact_tokens(&[1], &[1], zero);
    assert_eq!(outcome.lcs_length(), None);
    assert_eq!(outcome.work().total, 0);
    assert!(matches!(
        outcome,
        ExactAlignmentOutcome::NotAssessable {
            reason: ExactAlignmentIncompleteReason::WorkBudgetExhausted { .. },
            ..
        }
    ));
    for (oracle, side) in [(&[][..], &[1][..]), (&[1][..], &[][..]), (&[][..], &[][..])] {
        let outcome = align_exact_tokens(oracle, side, zero);
        assert_eq!(outcome.lcs_length(), Some(0));
        assert_eq!(outcome.work().total, 0);
    }
}

#[test]
fn every_budget_below_completion_withholds_l_and_partial_pairs() {
    let oracle = [0, 1, 2, 3, 1, 4, 0];
    let side = [2, 3, 0, 1, 4, 1, 0];
    let full = align_exact_tokens(&oracle, &side, ExactAlignmentBudget { max_work: u64::MAX });
    assert!(full.lcs_length().is_some());
    let required = full.work().total;
    for max_work in 0..required {
        let outcome = align_exact_tokens(&oracle, &side, ExactAlignmentBudget { max_work });
        assert_eq!(outcome.lcs_length(), None, "budget={max_work}");
        assert!(outcome.work().total <= max_work);
        assert!(matches!(
            outcome,
            ExactAlignmentOutcome::NotAssessable {
                reason: ExactAlignmentIncompleteReason::WorkBudgetExhausted { .. },
                ..
            }
        ));
    }
    assert_eq!(
        align_exact_tokens(&oracle, &side, ExactAlignmentBudget { max_work: required }),
        full
    );
}

#[test]
fn long_snakes_and_final_reconstruction_equalities_are_charged() {
    let input = vec![7; 1_000];
    let outcome = align_exact_tokens(&input, &input, ExactAlignmentBudget { max_work: u64::MAX });
    assert_eq!(outcome.lcs_length(), Some(input.len()));
    // One prefix comparison and one independent validation comparison per pair.
    assert_eq!(outcome.work().equality_comparisons, 2_000);
    let full = outcome.work().total;
    let almost = align_exact_tokens(&input, &input, ExactAlignmentBudget { max_work: full - 1 });
    assert_eq!(almost.lcs_length(), None);
    assert!(matches!(
        almost,
        ExactAlignmentOutcome::NotAssessable {
            reason: ExactAlignmentIncompleteReason::WorkBudgetExhausted {
                next_work: ExactAlignmentWorkKind::EqualityComparison,
            },
            ..
        }
    ));
}

#[test]
fn huge_dissimilar_inputs_stop_under_the_explicit_caller_budget() {
    let oracle = vec![0; 100_000];
    let side = vec![1; 100_000];
    // Test workload, not an accepted production threshold. Enough to admit the
    // linear scratch, but nowhere near a quadratic dissimilar search.
    let budget = ExactAlignmentBudget {
        max_work: 1_000_000,
    };
    let outcome = align_exact_tokens(&oracle, &side, budget);
    assert_eq!(outcome.lcs_length(), None);
    assert!(outcome.work().total <= budget.max_work);
    assert!(outcome.work().scratch_cells > 0);
    assert!(outcome.work().frontier_steps > 0);
    assert!(outcome.work().equality_comparisons > 0);
    assert!(matches!(
        outcome,
        ExactAlignmentOutcome::NotAssessable {
            reason: ExactAlignmentIncompleteReason::WorkBudgetExhausted { .. },
            ..
        }
    ));
}

#[test]
fn huge_dissimilar_inputs_refuse_scratch_before_allocating_above_budget() {
    let oracle = vec![0; 100_000];
    let side = vec![1; 100_000];
    let outcome = align_exact_tokens(&oracle, &side, ExactAlignmentBudget { max_work: 100 });
    assert_eq!(outcome.lcs_length(), None);
    assert_eq!(outcome.work().scratch_cells, 0);
    assert!(matches!(
        outcome,
        ExactAlignmentOutcome::NotAssessable {
            reason: ExactAlignmentIncompleteReason::WorkBudgetExhausted {
                next_work: ExactAlignmentWorkKind::ScratchCell,
            },
            ..
        }
    ));
}

#[test]
fn completed_length_is_symmetric_and_a_shared_fresh_suffix_adds_one() {
    for oracle in sequences(3, 2) {
        for side in sequences(3, 2) {
            let length = complete(&oracle, &side).len();
            assert_eq!(length, complete(&side, &oracle).len());
            let mut extended_oracle = oracle.clone();
            let mut extended_side = side.clone();
            extended_oracle.push(99);
            extended_side.push(99);
            assert_eq!(complete(&extended_oracle, &extended_side).len(), length + 1);
        }
    }
}

#[test]
fn outcomes_are_deterministic_and_roundtrip_without_token_text() {
    let first = align_exact_tokens(
        &[0, 1, 0, 1, 2],
        &[1, 0, 1, 0, 2],
        ExactAlignmentBudget { max_work: u64::MAX },
    );
    let second = align_exact_tokens(
        &[0, 1, 0, 1, 2],
        &[1, 0, 1, 0, 2],
        ExactAlignmentBudget { max_work: u64::MAX },
    );
    assert_eq!(first, second);
    for outcome in [
        first,
        align_exact_tokens(&[0], &[1], ExactAlignmentBudget { max_work: 0 }),
    ] {
        let encoded = serde_json::to_string(&outcome).unwrap();
        let decoded: ExactAlignmentOutcome = serde_json::from_str(&encoded).unwrap();
        assert_eq!(outcome, decoded);
    }
}
