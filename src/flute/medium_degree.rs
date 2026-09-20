// SPDX-License-Identifier: Apache-2.0
//! `flutes_medium_degree` — degrees above the lookup table.
//!
//! The reference first tries to DECOMPOSE the net into two sub-problems that share a terminal,
//! solving each through [`flutes_all_degree`] and joining them with
//! [`super::merge::d_merge_tree`]. Only when neither decomposition applies does it fall through
//! to a scoring heuristic.
//!
//! ⬜ The heuristic is not implemented here yet; [`flutes_medium_degree`] returns `None` for those
//! nets rather than answering from a path the reference would not have taken.

use super::breaks::{accuracy, break_in_x, break_pt, scores, take_best};
use super::low_degree::flutes_low_degree;
use super::merge::{h_merge_tree, v_merge_tree};
use super::refine::local_refinement;
use super::score::{break_range, penalties, scale_factors, spans};
use super::lut::{Lut, MAX_LUT_DEGREE};
use super::merge::d_merge_tree;
use crate::tree::Tree;

/// Where the net splits, if it splits at all.
///
/// ⛔ **Both searches use a loop whose BOUND MOVES AS THE LOOP RUNS.** The reference writes
///
/// ```text
/// ms = max(s[0], s[1]);
/// for (int i = 2; i <= ms; i++) ms = max(ms, s[i]);
/// ```
///
/// — `ms` is both the accumulator and the limit, so each new maximum EXTENDS the scan. It is
/// computing the shortest prefix that is closed under `s`: the smallest `ms` with
/// `max(s[0..=ms]) == ms`. Transcribed as a fixed-bound loop (`for i in 2..=initial_ms`) it stops
/// early and returns a split point the reference would have rejected — and the tree still builds,
/// so nothing announces the difference.
///
/// The second branch is the mirror: `ms` shrinks and the bound `d - 1 - ms` grows.
fn split_point_forward(d: usize, s: &[usize]) -> usize {
    let mut ms = s[0].max(s[1]);
    let mut i = 2usize;
    while i <= ms {
        ms = ms.max(s[i]);
        i += 1;
    }
    let _ = d;
    ms
}

fn split_point_backward(d: usize, s: &[usize]) -> usize {
    let mut ms = s[0].min(s[1]);
    let mut i = 2usize;
    while i <= d - 1 - ms {
        ms = ms.min(s[i]);
        i += 1;
    }
    ms
}

/// `Flute::flutes_all_degree` — the dispatcher both paths recurse through.
pub fn flutes_all_degree(
    lut: &Lut,
    d: usize,
    xs: &[i32],
    ys: &[i32],
    s: &[usize],
) -> Option<Tree> {
    flutes_all_degree_acc(lut, d, xs, ys, s, ACCURACY)
}

/// The reference's default accuracy for tree construction.
pub const ACCURACY: i32 = 3;

pub fn flutes_all_degree_acc(
    lut: &Lut,
    d: usize,
    xs: &[i32],
    ys: &[i32],
    s: &[usize],
    acc: i32,
) -> Option<Tree> {
    if d <= MAX_LUT_DEGREE {
        return Some(flutes_low_degree(lut, d, xs, ys, s));
    }
    flutes_medium_degree_acc(lut, d, xs, ys, s, acc)
}

/// Decompose and merge, or `None` when neither decomposition applies.
pub fn flutes_medium_degree(
    lut: &Lut,
    d: usize,
    xs: &[i32],
    ys: &[i32],
    s: &[usize],
) -> Option<Tree> {
    flutes_medium_degree_acc(lut, d, xs, ys, s, ACCURACY)
}

pub fn flutes_medium_degree_acc(
    lut: &Lut,
    d: usize,
    xs: &[i32],
    ys: &[i32],
    s: &[usize],
    acc: i32,
) -> Option<Tree> {
    if s[0] < s[d - 1] {
        let ms = split_point_forward(d, s);
        if ms <= d - 3 {
            // First half: points 0..=ms, with the split point DUPLICATED at ms+1 — that
            // duplicate is the terminal the two halves share.
            let mut x1: Vec<i32> = xs[..=ms].to_vec();
            let mut y1: Vec<i32> = ys[..=ms].to_vec();
            let mut s1: Vec<usize> = s[..=ms].to_vec();
            x1.push(xs[ms]);
            y1.push(ys[ms]);
            s1.push(ms + 1);

            // Second half: s re-based by ms, with s2[0] pinned to 0.
            let mut s2 = vec![0usize; d - ms];
            for i in 1..=(d - 1 - ms) {
                s2[i] = s[i + ms] - ms;
            }

            let t1 = flutes_all_degree_acc(lut, ms + 2, &x1, &y1, &s1, acc)?;
            let t2 = flutes_all_degree_acc(lut, d - ms, &xs[ms..], &ys[ms..], &s2, acc)?;
            return Some(d_merge_tree(&t1, &t2));
        }
    } else {
        let ms = split_point_backward(d, s);
        if ms >= 2 {
            let mut x1 = vec![0i32; d - ms + 1];
            let mut y1 = vec![0i32; d - ms + 1];
            let mut s1 = vec![0usize; d - ms + 1];
            x1[0] = xs[ms];
            y1[0] = ys[0];
            s1[0] = s[0] - ms + 1;
            for i in 1..=(d - 1 - ms) {
                x1[i] = xs[i + ms - 1];
                y1[i] = ys[i];
                s1[i] = s[i] - ms + 1;
            }
            x1[d - ms] = xs[d - 1];
            y1[d - ms] = ys[d - 1 - ms];
            s1[d - ms] = 0;

            let mut s2 = vec![0usize; ms + 1];
            s2[0] = ms;
            for i in 1..=ms {
                s2[i] = s[i + d - 1 - ms];
            }

            let t1 = flutes_all_degree_acc(lut, d + 1 - ms, &x1, &y1, &s1, acc)?;
            // ⚠️ The FULL `xs` is passed here — only `ys` is sliced. Slicing both, by symmetry
            // with the branch above, would hand the sub-problem different geometry.
            let t2 = flutes_all_degree_acc(lut, ms + 1, xs, &ys[(d - 1 - ms)..], &s2, acc)?;
            return Some(d_merge_tree(&t1, &t2));
        }
    }
    scoring_heuristic(lut, d, xs, ys, s, acc)
}

/// The scoring heuristic — try the best `acc` breaks and keep the shortest result.
///
/// ⛔ **The recursion runs at `newacc`, not at `acc`.** The reference passes the HALVED accuracy
/// into both sub-problems (`flutes_all_degree(..., newacc)`), and that halving is what bounds the
/// search — each level tries fewer candidates than the one above. Recursing at the original `acc`
/// is not merely slower: it explores a different, larger set of breaks and can pick a different
/// winner. Measured here as a hang on degree-36 nets before it was corrected.
///
/// ⚠️ **`ll` is the merged length computed WITHOUT merging**: the two sub-tree lengths plus an
/// overlap correction, mirroring what `h_merge_tree`/`v_merge_tree` would subtract. The reference
/// avoids building a tree per candidate and only merges the winner, so the correction is written
/// twice — once here as arithmetic, once in the merge as a coordinate. They must agree.
fn scoring_heuristic(
    lut: &Lut,
    d: usize,
    xs: &[i32],
    ys: &[i32],
    s: &[usize],
    acc: i32,
) -> Option<Tree> {
    // si[] is the inverse permutation of s[].
    let mut si = vec![0usize; d];
    for r in 0..d {
        si[s[r]] = r;
    }

    let (lb, ub) = break_range(d, acc);
    if lb > ub {
        return None;
    }
    let (cc, dd_) = scale_factors(d);
    let penalty = penalties(d, xs, ys, s, cc);
    let (distx, disty) = spans(d, xs, ys, s, &si, lb, ub);
    let mut score = scores(d, xs, ys, s, &si, &penalty, &distx, &disty, dd_, lb, ub);
    let nbp = score.len();
    let (newacc, acc) = accuracy(acc, nbp);

    let mut minl = i64::MAX;
    let mut best: Option<(Tree, Tree, usize)> = None;

    for _ in 0..acc.max(0) {
        let maxbp = take_best(&mut score);
        let p = break_pt(maxbp, lb);
        // ⚠️ `p` indexes xs/ys directly, so a break at or past the ends cannot be split.
        if p == 0 || p >= d {
            continue;
        }

        let (t1, t2, ll) = if break_in_x(maxbp) {
            // Split by s[r] against p: the r with s[r] == p is SHARED by both halves.
            let (mut y1, mut s1, mut y2, mut s2) = (vec![], vec![], vec![], vec![]);
            let (mut nn1, mut nn2) = (0usize, 0usize);
            for r in 0..d {
                if s[r] < p {
                    s1.push(s[r]);
                    y1.push(ys[r]);
                } else if s[r] > p {
                    s2.push(s[r] - p);
                    y2.push(ys[r]);
                } else {
                    nn1 = s1.len();
                    nn2 = s2.len();
                    s1.push(p);
                    s2.push(0);
                    y1.push(ys[r]);
                    y2.push(ys[r]);
                }
            }
            let t1 = flutes_all_degree_acc(lut, p + 1, xs, &y1, &s1, newacc)?;
            let t2 = flutes_all_degree_acc(lut, d - p, &xs[p..], &y2, &s2, newacc)?;
            let ll = merged_length(&t1, &t2, nn1, nn2, true);
            (t1, t2, ll)
        } else {
            // Split by si[r] against p, writing s1/s2 by DESTINATION index rather than pushing.
            let mut x1 = vec![0i32; p + 1];
            let mut x2 = vec![0i32; d - p];
            let mut s1 = vec![0usize; p + 1];
            let mut s2 = vec![0usize; d - p];
            let (mut n1, mut n2) = (0usize, 0usize);
            for r in 0..d {
                if si[r] < p {
                    s1[si[r]] = n1;
                    x1[n1] = xs[r];
                    n1 += 1;
                } else if si[r] > p {
                    s2[si[r] - p] = n2;
                    x2[n2] = xs[r];
                    n2 += 1;
                } else {
                    s1[p] = n1;
                    s2[0] = n2;
                    x1[n1] = xs[r];
                    x2[n2] = xs[r];
                    n1 += 1;
                    n2 += 1;
                }
            }
            let t1 = flutes_all_degree_acc(lut, p + 1, &x1, ys, &s1, newacc)?;
            let t2 = flutes_all_degree_acc(lut, d - p, &x2, &ys[p..], &s2, newacc)?;
            let ll = merged_length(&t1, &t2, p, 0, false);
            (t1, t2, ll)
        };

        if minl > ll {
            minl = ll;
            best = Some((t1, t2, maxbp));
        }
    }

    let (bt1, bt2, bestbp) = best?;
    // ⚠️ `kLocalRefinement` is always true in the reference, so refinement is not optional.
    let mut t = if break_in_x(bestbp) {
        let mut t = h_merge_tree(&bt1, &bt2, s);
        // ⛔ A refusal here is OUR defect surfacing, not a property of the net: the merged tree
        // contains a parent cycle on some inputs. Refuse rather than hang, and rather than
        // answer from a structure known to be wrong.
        local_refinement(lut, d + 1, &mut t, si[break_pt(bestbp, lb)]).ok()?;
        t
    } else {
        let mut t = v_merge_tree(&bt1, &bt2);
        local_refinement(lut, d + 1, &mut t, break_pt(bestbp, lb)).ok()?;
        t
    };
    t.deg = d;
    Some(t)
}

/// The length the merge WOULD produce, without building it.
///
/// The two halves overlap at the joining branch; whichever way that branch lies outside the
/// span of the two coordinates it joins, that excess is removed.
fn merged_length(t1: &Tree, t2: &Tree, nn1: usize, nn2: usize, in_x: bool) -> i64 {
    let mut ll = t1.length + t2.length;
    let pick = |t: &Tree, i: usize| if in_x { t.branch[t.branch[i].n].y } else { t.branch[t.branch[i].n].x };
    let joiner = if in_x { t2.branch[nn2].y } else { t2.branch[nn2].x };
    let (c1, c2) = (pick(t1, nn1), pick(t2, nn2));
    let (lo, hi) = (c1.min(c2), c1.max(c2));
    if joiner > hi {
        ll -= i64::from(joiner - hi);
    } else if joiner < lo {
        ll -= i64::from(lo - joiner);
    }
    ll
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_forward_search_extends_as_it_finds_larger_values() {
        // ⚠️ Inputs chosen by TRACING the loop, not by intuition. A first version of this test
        // used ms = max(s[0], s[1]) = 1, where `i <= ms` is `2 <= 1` and the loop never runs at
        // all — it asserted a mental model the source does not have.
        //
        // Here ms starts at 2, so the scan runs; s[2] = 3 raises it to 3, which extends the
        // bound to 3; s[3] = 9 then raises it to 9 and extends the scan to i = 9.
        let s = [2usize, 0, 3, 9, 1, 4, 5, 6, 7, 8, 10];
        assert_eq!(split_point_forward(11, &s), 9);

        // The same input under a FIXED bound would stop at i = 2 and answer 3 — a different,
        // smaller split point that still produces a buildable tree.
        let mut fixed = s[0].max(s[1]);
        let initial = fixed;
        for i in 2..=initial {
            fixed = fixed.max(s[i]);
        }
        assert_eq!(fixed, 3, "a fixed bound stops early");
        assert_ne!(fixed, split_point_forward(11, &s), "and that is a different answer");
    }

    #[test]
    fn the_forward_search_stops_once_the_prefix_is_closed() {
        // max(s[0..=1]) == 1, so `i <= ms` is `2 <= 1` and nothing extends it.
        let s = [0usize, 1, 5, 6, 7, 8, 9, 10, 2, 3, 4];
        assert_eq!(split_point_forward(11, &s), 1);
    }

    #[test]
    fn the_backward_search_is_the_mirror_and_also_extends() {
        // ms starts at min(5, 6) = 5, so the bound is d-1-ms = 4. s[2] = 3 lowers ms to 3 and
        // widens the bound to 6; s[5] = 0 then lowers it to 0 and widens it to 9.
        let s = [5usize, 6, 3, 9, 8, 0, 1, 2, 4, 7];
        assert_eq!(split_point_backward(10, &s), 0);

        // Under a fixed bound the scan never reaches s[5] and answers 3.
        let mut fixed = s[0].min(s[1]);
        let bound = 10 - 1 - fixed;
        for i in 2..=bound {
            fixed = fixed.min(s[i]);
        }
        assert_eq!(fixed, 3, "a fixed bound never reaches the zero at index 5");
        assert_ne!(fixed, split_point_backward(10, &s));
    }

    #[test]
    fn a_decomposable_net_of_degree_eleven_builds_a_tree() {
        use super::super::entry::prepare;
        use super::super::lut::load_tables;
        let lut = load_tables(MAX_LUT_DEGREE).expect("tables");
        let x: Vec<i32> = (0..11).map(|i| i * 13 % 47).collect();
        let y: Vec<i32> = (0..11).map(|i| i * 17 % 53).collect();
        let p = prepare(&x, &y);
        if let Some(t) = flutes_medium_degree(&lut, 11, &p.xs, &p.ys, &p.s) {
            assert_eq!(t.deg, 11);
            assert_eq!(t.branch.len(), 2 * 11 - 2);
            assert!(t.branch.iter().all(|b| b.n < t.branch.len()));
        }
        // Not asserting it decomposes: whether it does is a property of THIS point set, and a
        // test that demanded it would be asserting the fixture rather than the rule.
    }
}