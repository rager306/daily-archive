//! Independent, exact-only alignment experiment over declared token identities.
//!
//! This does not replace the frozen body locator, issue a P1/P2 verdict, or tile
//! token boundaries. Inputs are already normalized units, interned together into
//! collision-free `u32` identities. A shared ID must mean equal units on both
//! sides; hashing strings into IDs without collision checking is not sufficient.
//!
//! A complete result is one maximum-cardinality monotone matching (an LCS).
//! Repeated units can have several optimal witnesses; deterministic selection
//! does not prove which physical occurrence a pair represents. The multiset
//! overlap U and any valid greedy count H are separate caller diagnostics:
//! H <= L <= U. L is available only on `Complete`.
//!
//! Reconstruction uses Myers' bidirectional frontier split with an explicit
//! stack, not a stored quadratic trace. Live auxiliary space is O(n + m), plus
//! the returned O(L) pairs. Worst-case work remains quadratic. There are no
//! heuristic anchors, approximate fallbacks, or implicit budgets.
//!
//! The caller's deterministic work budget covers frontier initialization and
//! updates, every token equality (including prefix/suffix scans and final
//! witness validation), and reconstruction stack/pair operations. These are
//! abstract work units, not CPU instructions, a byte limit, or a wall deadline.
//! Vector growth is amortized over charged reconstruction operations. Input
//! acquisition, normalization, collision-free interning, U/H, and report IO are
//! outside this function and must be accounted for separately by the experiment.

use serde::{Deserialize, Serialize};

/// Identity of this independent experiment, not the frozen body-text contract.
pub const NATIVE_EXACT_ALIGNMENT_SCHEMA_VERSION: &str = "da-parse.native-exact-alignment/v1";

/// No default or product threshold: the experiment caller chooses its budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactAlignmentBudget {
    pub max_work: u64,
}

/// Operations charged before they execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactAlignmentWorkKind {
    FrontierStep,
    EqualityComparison,
    ScratchCell,
    ReconstructionStep,
}

/// Successful charges only; `total` is the sum of the four component counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactAlignmentWork {
    pub total: u64,
    pub frontier_steps: u64,
    pub equality_comparisons: u64,
    /// Initialized frontier cells; both search directions are charged.
    pub scratch_cells: u64,
    /// Stack pushes/pops, pair appends, and final pair-validation steps.
    pub reconstruction_steps: u64,
}

/// A conserved exact binding: both indices strictly increase across the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactTokenPair {
    pub oracle_index: usize,
    pub side_index: usize,
}

/// An incomplete experiment is not an approximate optimum or a body verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactAlignmentIncompleteReason {
    WorkBudgetExhausted { next_work: ExactAlignmentWorkKind },
    InputTooLarge,
    AllocationFailed,
    InvalidReconstruction,
}

/// No L or partial witness is published when any search/reconstruction work fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactAlignmentOutcome {
    Complete {
        pairs: Vec<ExactTokenPair>,
        work: ExactAlignmentWork,
    },
    NotAssessable {
        reason: ExactAlignmentIncompleteReason,
        work: ExactAlignmentWork,
    },
}

impl ExactAlignmentOutcome {
    /// The exact LCS length only after search, reconstruction and validation finish.
    pub fn lcs_length(&self) -> Option<usize> {
        match self {
            Self::Complete { pairs, .. } => Some(pairs.len()),
            Self::NotAssessable { .. } => None,
        }
    }

    pub fn work(&self) -> &ExactAlignmentWork {
        match self {
            Self::Complete { work, .. } | Self::NotAssessable { work, .. } => work,
        }
    }
}

/// Align two sequences of shared, collision-free normalized token IDs.
///
/// Empty-side comparisons have an exact empty witness without search work, even
/// with a zero budget. Nonempty inputs can stop before the next charged operation;
/// that returns `NotAssessable`, never a truncated `Complete` result.
pub fn align_exact_tokens(
    oracle: &[u32],
    side: &[u32],
    budget: ExactAlignmentBudget,
) -> ExactAlignmentOutcome {
    let mut meter = WorkMeter {
        limit: budget.max_work,
        work: ExactAlignmentWork::default(),
    };
    match reconstruct(oracle, side, &mut meter) {
        Ok(pairs) => ExactAlignmentOutcome::Complete {
            pairs,
            work: meter.work,
        },
        Err(reason) => ExactAlignmentOutcome::NotAssessable {
            reason,
            work: meter.work,
        },
    }
}

type AlignmentResult<T> = Result<T, ExactAlignmentIncompleteReason>;

struct WorkMeter {
    limit: u64,
    work: ExactAlignmentWork,
}

impl WorkMeter {
    fn charge(&mut self, kind: ExactAlignmentWorkKind, amount: u64) -> AlignmentResult<()> {
        if amount > self.limit - self.work.total {
            return Err(ExactAlignmentIncompleteReason::WorkBudgetExhausted { next_work: kind });
        }
        self.work.total += amount;
        match kind {
            ExactAlignmentWorkKind::FrontierStep => self.work.frontier_steps += amount,
            ExactAlignmentWorkKind::EqualityComparison => self.work.equality_comparisons += amount,
            ExactAlignmentWorkKind::ScratchCell => self.work.scratch_cells += amount,
            ExactAlignmentWorkKind::ReconstructionStep => self.work.reconstruction_steps += amount,
        }
        Ok(())
    }

    fn equal(&mut self, left: u32, right: u32) -> AlignmentResult<bool> {
        self.charge(ExactAlignmentWorkKind::EqualityComparison, 1)?;
        Ok(left == right)
    }
}

#[derive(Clone, Copy)]
struct Range {
    oracle_start: usize,
    oracle_end: usize,
    side_start: usize,
    side_end: usize,
}

#[derive(Clone, Copy)]
enum Pending {
    Range(Range),
    EqualRun {
        oracle_start: usize,
        side_start: usize,
        len: usize,
    },
}

fn push_pending(
    stack: &mut Vec<Pending>,
    pending: Pending,
    meter: &mut WorkMeter,
) -> AlignmentResult<()> {
    meter.charge(ExactAlignmentWorkKind::ReconstructionStep, 1)?;
    stack
        .try_reserve(1)
        .map_err(|_| ExactAlignmentIncompleteReason::AllocationFailed)?;
    stack.push(pending);
    Ok(())
}

fn append_pair(
    pairs: &mut Vec<ExactTokenPair>,
    oracle_index: usize,
    side_index: usize,
    meter: &mut WorkMeter,
) -> AlignmentResult<()> {
    meter.charge(ExactAlignmentWorkKind::ReconstructionStep, 1)?;
    pairs
        .try_reserve(1)
        .map_err(|_| ExactAlignmentIncompleteReason::AllocationFailed)?;
    pairs.push(ExactTokenPair {
        oracle_index,
        side_index,
    });
    Ok(())
}

fn reconstruct(
    oracle: &[u32],
    side: &[u32],
    meter: &mut WorkMeter,
) -> AlignmentResult<Vec<ExactTokenPair>> {
    let mut pairs = Vec::new();
    if oracle.is_empty() || side.is_empty() {
        return Ok(pairs);
    }
    let mut stack = Vec::new();
    push_pending(
        &mut stack,
        Pending::Range(Range {
            oracle_start: 0,
            oracle_end: oracle.len(),
            side_start: 0,
            side_end: side.len(),
        }),
        meter,
    )?;
    while !stack.is_empty() {
        meter.charge(ExactAlignmentWorkKind::ReconstructionStep, 1)?;
        let pending = stack.pop().expect("nonempty reconstruction stack");
        let mut range = match pending {
            Pending::EqualRun {
                oracle_start,
                side_start,
                len,
            } => {
                for offset in 0..len {
                    append_pair(
                        &mut pairs,
                        oracle_start + offset,
                        side_start + offset,
                        meter,
                    )?;
                }
                continue;
            }
            Pending::Range(range) => range,
        };
        while range.oracle_start < range.oracle_end && range.side_start < range.side_end {
            if !meter.equal(oracle[range.oracle_start], side[range.side_start])? {
                break;
            }
            append_pair(&mut pairs, range.oracle_start, range.side_start, meter)?;
            range.oracle_start += 1;
            range.side_start += 1;
        }
        let mut suffix = 0;
        while range.oracle_start < range.oracle_end && range.side_start < range.side_end {
            if !meter.equal(oracle[range.oracle_end - 1], side[range.side_end - 1])? {
                break;
            }
            range.oracle_end -= 1;
            range.side_end -= 1;
            suffix += 1;
        }
        if suffix > 0 {
            push_pending(
                &mut stack,
                Pending::EqualRun {
                    oracle_start: range.oracle_end,
                    side_start: range.side_end,
                    len: suffix,
                },
                meter,
            )?;
        }
        let a = &oracle[range.oracle_start..range.oracle_end];
        let b = &side[range.side_start..range.side_end];
        if a.is_empty() || b.is_empty() {
            continue;
        }
        if a.len() == 1 {
            for (index, token) in b.iter().enumerate() {
                if meter.equal(a[0], *token)? {
                    append_pair(
                        &mut pairs,
                        range.oracle_start,
                        range.side_start + index,
                        meter,
                    )?;
                    break;
                }
            }
        } else if b.len() == 1 {
            for (index, token) in a.iter().enumerate() {
                if meter.equal(*token, b[0])? {
                    append_pair(
                        &mut pairs,
                        range.oracle_start + index,
                        range.side_start,
                        meter,
                    )?;
                    break;
                }
            }
        } else {
            let (oracle_split, side_split) = bisect(a, b, meter)?;
            if (oracle_split == 0 && side_split == 0)
                || (oracle_split == a.len() && side_split == b.len())
            {
                return Err(ExactAlignmentIncompleteReason::InvalidReconstruction);
            }
            let oracle_mid = range.oracle_start + oracle_split;
            let side_mid = range.side_start + side_split;
            // LIFO: finish the left range, then the right range, then its suffix.
            push_pending(
                &mut stack,
                Pending::Range(Range {
                    oracle_start: oracle_mid,
                    oracle_end: range.oracle_end,
                    side_start: side_mid,
                    side_end: range.side_end,
                }),
                meter,
            )?;
            push_pending(
                &mut stack,
                Pending::Range(Range {
                    oracle_start: range.oracle_start,
                    oracle_end: oracle_mid,
                    side_start: range.side_start,
                    side_end: side_mid,
                }),
                meter,
            )?;
        }
    }
    let mut previous: Option<ExactTokenPair> = None;
    for pair in &pairs {
        meter.charge(ExactAlignmentWorkKind::ReconstructionStep, 1)?;
        if previous.is_some_and(|prior| {
            prior.oracle_index >= pair.oracle_index || prior.side_index >= pair.side_index
        }) || !meter.equal(oracle[pair.oracle_index], side[pair.side_index])?
        {
            return Err(ExactAlignmentIncompleteReason::InvalidReconstruction);
        }
        previous = Some(*pair);
    }
    Ok(pairs)
}

/// Find an exact shortest-path split. Frontier vectors die before reconstruction
/// descends into either child, so they are never retained per edit distance.
fn bisect(oracle: &[u32], side: &[u32], meter: &mut WorkMeter) -> AlignmentResult<(usize, usize)> {
    let n =
        isize::try_from(oracle.len()).map_err(|_| ExactAlignmentIncompleteReason::InputTooLarge)?;
    let m =
        isize::try_from(side.len()).map_err(|_| ExactAlignmentIncompleteReason::InputTooLarge)?;
    let max_d = n
        .checked_add(m)
        .and_then(|total| total.checked_add(1))
        .map(|total| total / 2)
        .ok_or(ExactAlignmentIncompleteReason::InputTooLarge)?;
    let width = max_d
        .checked_mul(2)
        .and_then(|width| width.checked_add(3))
        .and_then(|width| usize::try_from(width).ok())
        .ok_or(ExactAlignmentIncompleteReason::InputTooLarge)?;
    let scratch_cells = width
        .checked_mul(2)
        .and_then(|cells| u64::try_from(cells).ok())
        .ok_or(ExactAlignmentIncompleteReason::InputTooLarge)?;
    meter.charge(ExactAlignmentWorkKind::ScratchCell, scratch_cells)?;
    let mut forward = frontier(width)?;
    let mut reverse = frontier(width)?;
    let offset = max_d + 1;
    let delta = n - m;
    let odd_delta = delta % 2 != 0;
    for d in 0..=max_d {
        for k in (-d..=d).step_by(2) {
            let x = advance(oracle, side, false, &mut forward, offset, d, k, meter)?;
            let reverse_k = delta - k;
            if odd_delta && d > 0 && reverse_k.abs() < d && x >= 0 {
                let reverse_x = reverse[(offset + reverse_k) as usize];
                if reverse_x >= 0 && x >= n - reverse_x {
                    return Ok((x as usize, (x - k) as usize));
                }
            }
        }
        for k in (-d..=d).step_by(2) {
            let x = advance(oracle, side, true, &mut reverse, offset, d, k, meter)?;
            let forward_k = delta - k;
            if !odd_delta && forward_k.abs() <= d && x >= 0 {
                let forward_x = forward[(offset + forward_k) as usize];
                if forward_x >= 0 && forward_x >= n - x {
                    return Ok((forward_x as usize, (forward_x - forward_k) as usize));
                }
            }
        }
    }
    Err(ExactAlignmentIncompleteReason::InvalidReconstruction)
}

fn frontier(width: usize) -> AlignmentResult<Vec<isize>> {
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(width)
        .map_err(|_| ExactAlignmentIncompleteReason::AllocationFailed)?;
    cells.resize(width, -1);
    Ok(cells)
}

#[allow(clippy::too_many_arguments)]
fn advance(
    oracle: &[u32],
    side: &[u32],
    reversed: bool,
    frontier: &mut [isize],
    offset: isize,
    d: isize,
    k: isize,
    meter: &mut WorkMeter,
) -> AlignmentResult<isize> {
    meter.charge(ExactAlignmentWorkKind::FrontierStep, 1)?;
    let n = oracle.len() as isize;
    let m = side.len() as isize;
    let index = (offset + k) as usize;
    let mut x = if d == 0 {
        0
    } else {
        let insertion = frontier[index + 1];
        let deletion = frontier[index - 1];
        let insertion = if insertion >= 0 && insertion - (k + 1) < m {
            insertion
        } else {
            -1
        };
        let deletion = if deletion >= 0 && deletion < n {
            deletion + 1
        } else {
            -1
        };
        insertion.max(deletion)
    };
    if x >= 0 {
        let mut y = x - k;
        while x < n && y < m {
            let oracle_index = if reversed { n - x - 1 } else { x } as usize;
            let side_index = if reversed { m - y - 1 } else { y } as usize;
            if !meter.equal(oracle[oracle_index], side[side_index])? {
                break;
            }
            x += 1;
            y += 1;
        }
    }
    frontier[index] = x;
    Ok(x)
}

#[cfg(test)]
mod coverage_smoke {
    use super::*;

    #[test]
    fn empty_sides_have_an_exact_empty_witness() {
        let outcome = align_exact_tokens(&[], &[], ExactAlignmentBudget { max_work: 0 });
        assert!(matches!(outcome, ExactAlignmentOutcome::Complete { .. }));
        assert_eq!(outcome.lcs_length(), Some(0));
    }

    #[test]
    fn identical_tokens_align_completely_without_budget_pressure() {
        let outcome = align_exact_tokens(
            &[7, 8, 9],
            &[7, 8, 9],
            ExactAlignmentBudget { max_work: 1000 },
        );
        match &outcome {
            ExactAlignmentOutcome::Complete { pairs, .. } => {
                assert_eq!(pairs.len(), 3);
                assert!(
                    pairs
                        .iter()
                        .enumerate()
                        .all(|(i, pair)| { pair.oracle_index == i && pair.side_index == i })
                );
            }
            ExactAlignmentOutcome::NotAssessable { .. } => {
                panic!("identical tokens must align completely")
            }
        }
    }
}
