// SPDX-License-Identifier: Apache-2.0
//! Choosing a solution from the table's group, and assembling the tree from it.
//!
//! This is the back half of `flutes_low_degree`: evaluate every stored solution for the group,
//! keep the cheapest, then lay its topology onto the real coordinates.

use super::index::GroupIndex;
use super::lut::Csoln;
use crate::tree::{Branch, Tree};

/// Evaluate a group's solutions and return the index of the cheapest and its length.
///
/// ```text
/// l[0] = xs[d-1] - xs[0] + ys[d-1] - ys[0];
/// minl = l[0];
/// for (i = 0; rlist->seg[i] > 0; i++)  minl += dd[rlist->seg[i]];
/// l[1] = minl;  j = 2;
/// while (j <= numsoln) {
///     rlist++;
///     sum = l[rlist->parent];
///     for (i = 0;  rlist->seg[i] > 0; i++)  sum += dd[rlist->seg[i]];
///     for (i = 10; rlist->seg[i] > 0; i--)  sum -= dd[rlist->seg[i]];
///     if (sum < minl) { minl = sum; bestrlist = rlist; }
///     l[j++] = sum;
/// }
/// ```
///
/// 🔑 **The evaluation is INCREMENTAL, and that is the whole design.** Solution 1's length is
/// computed from scratch — the bounding box's half-perimeter plus its forward segments. Every
/// later solution starts from `l[parent]`, the already-computed length of another solution in the
/// same group, and adjusts it. `seg`'s two halves are the adjustment: the forward half ADDS gaps,
/// the backward half SUBTRACTS them. Recomputing each solution from scratch would give different
/// answers wherever the table relies on that chain.
///
/// ⚠️ **Solution 1 applies only the forward half**, because it has no parent to subtract from.
///
/// ⚠️ **`sum < minl` is strict**, so the FIRST solution achieving the minimum wins. `<=` would
/// pick the last, and the two disagree whenever a group holds ties — which, in a table of optimal
/// topologies, is common.
pub fn best_solution(solns: &[Csoln], dd: &[i32], bbox_half_perimeter: i32) -> (usize, i32) {
    debug_assert!(!solns.is_empty());
    // `l` is indexed by SOLUTION NUMBER, one-based, with l[0] the bare bounding box — the
    // reference sizes it kMaxPowv + 1 for exactly that off-by-one.
    let mut l = vec![0i32; solns.len() + 2];
    l[0] = bbox_half_perimeter;

    let mut minl = l[0];
    for &seg in solns[0].seg.iter().take_while(|&&s| s > 0) {
        minl += dd[seg as usize];
    }
    let mut best = 0usize;
    l[1] = minl;

    for (idx, r) in solns.iter().enumerate().skip(1) {
        let mut sum = l[r.parent as usize];
        // ADD: the forward half, 0..i.
        for &seg in r.seg.iter().take_while(|&&s| s > 0) {
            sum += dd[seg as usize];
        }
        // SUB: the backward half, from index 10 downward. The two halves are separated by the
        // zeros written when the table was parsed, not by a length.
        let mut i = 10usize;
        while r.seg[i] > 0 {
            sum -= dd[r.seg[i] as usize];
            if i == 0 {
                break;
            }
            i -= 1;
        }
        if sum < minl {
            minl = sum;
            best = idx;
        }
        l[idx + 1] = sum;
    }
    (best, minl)
}

/// Lay a chosen solution's topology onto the real coordinates.
///
/// ⚠️ **The four endpoint branches get their neighbour by a COMPARISON, not by position**, and
/// the horizontal flip reverses which way that comparison runs — `s[1] < s[0]` under a flip where
/// it is `s[0] < s[1]` without one. The bodies are identical; only the test inverts.
///
/// ⚠️ **A Steiner point's x is mirrored under a flip**: `xs[d - 1 - rowcol % 16]` rather than
/// `xs[rowcol % 16]`. Its y is the same either way. That asymmetry is the reflection being
/// undone — the table was read mirrored, so the coordinates must be un-mirrored.
pub fn build_tree(
    d: usize,
    xs: &[i32],
    ys: &[i32],
    s: &[usize],
    soln: &Csoln,
    gi: GroupIndex,
    length: i32,
) -> Tree {
    let mut branch = vec![Branch { x: 0, y: 0, n: 0 }; 2 * d - 2];

    branch[0] = Branch { x: xs[s[0]], y: ys[0], n: 0 };
    branch[1] = Branch { x: xs[s[1]], y: ys[1], n: 0 };
    // ⚠️ `i < d - 2`, so branch d-2 is NOT set here — it is one of the four endpoints below.
    for i in 2..(d - 2) {
        branch[i] = Branch { x: xs[s[i]], y: ys[i], n: soln.neighbor[i] as usize };
    }
    branch[d - 2] = Branch { x: xs[s[d - 2]], y: ys[d - 2], n: 0 };
    branch[d - 1] = Branch { x: xs[s[d - 1]], y: ys[d - 1], n: 0 };

    // The endpoint pairs. Under a flip the comparison is reversed; the assignment is not.
    let (first_swaps, last_swaps) = if gi.hflip {
        (s[1] < s[0], s[d - 1] < s[d - 2])
    } else {
        (s[0] < s[1], s[d - 2] < s[d - 1])
    };
    if first_swaps {
        branch[0].n = soln.neighbor[1] as usize;
        branch[1].n = soln.neighbor[0] as usize;
    } else {
        branch[0].n = soln.neighbor[0] as usize;
        branch[1].n = soln.neighbor[1] as usize;
    }
    if last_swaps {
        branch[d - 2].n = soln.neighbor[d - 1] as usize;
        branch[d - 1].n = soln.neighbor[d - 2] as usize;
    } else {
        branch[d - 2].n = soln.neighbor[d - 2] as usize;
        branch[d - 1].n = soln.neighbor[d - 1] as usize;
    }

    // The Steiner points, read out of the packed rowcol nibbles.
    for i in d..(2 * d - 2) {
        let rc = soln.rowcol[i - d];
        let col = (rc % 16) as usize;
        let row = (rc / 16) as usize;
        branch[i] = Branch {
            x: if gi.hflip { xs[d - 1 - col] } else { xs[col] },
            y: ys[row],
            n: soln.neighbor[i] as usize,
        };
    }

    Tree { deg: d, length: i64::from(length), branch }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flute::lut::Csoln;

    fn soln(parent: u8, add: &[u8], sub: &[u8]) -> Csoln {
        let mut c = Csoln { parent, ..Csoln::default() };
        for (i, &v) in add.iter().enumerate() {
            c.seg[i] = v;
        }
        // The backward half fills from index 10 DOWNWARD, as the parser writes it.
        for (i, &v) in sub.iter().enumerate() {
            c.seg[10 - i] = v;
        }
        c
    }

    #[test]
    fn the_first_solution_is_the_bounding_box_plus_its_forward_segments_only() {
        // dd[1] = 5, dd[2] = 7. Solution 1 adds both; its SUB half must be ignored even if set,
        // because the reference never runs the backward loop for solution 1.
        let dd = [0, 5, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let s0 = soln(0, &[1, 2], &[2]);
        let (best, len) = best_solution(&[s0], &dd, 100);
        assert_eq!(best, 0);
        assert_eq!(len, 112, "100 + 5 + 7, with no subtraction");
    }

    #[test]
    fn later_solutions_start_from_their_parents_length_not_from_scratch() {
        // 🔑 Solution 2's parent is 1, so it begins at l[1] = 112 and then adds dd[2] = 7 and
        // subtracts dd[1] = 5 -> 114. Computed from scratch it would be 100 + 7 - 5 = 102, a
        // different and wrong answer.
        let dd = [0, 5, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let s1 = soln(0, &[1, 2], &[]);
        let s2 = soln(1, &[2], &[1]);
        let (best, len) = best_solution(&[s1, s2], &dd, 100);
        assert_eq!(best, 0, "112 < 114, so the first solution still wins");
        assert_eq!(len, 112);
    }

    #[test]
    fn a_cheaper_later_solution_is_selected() {
        let dd = [0, 5, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let s1 = soln(0, &[1, 2], &[]);           // 100 + 12 = 112
        let s2 = soln(1, &[], &[1, 2]);           // l[1]=112 - 5 - 7 = 100
        let (best, len) = best_solution(&[s1, s2], &dd, 100);
        assert_eq!(best, 1);
        assert_eq!(len, 100);
    }

    #[test]
    fn a_tie_keeps_the_FIRST_solution_because_the_test_is_strict() {
        // ⚠️ `sum < minl`, not `<=`. Two solutions of equal length must resolve to the earlier.
        let dd = [0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let s1 = soln(0, &[1], &[]);   // 100 + 5 = 105
        let s2 = soln(1, &[], &[]);    // l[1] = 105, unchanged -> a tie
        let (best, len) = best_solution(&[s1, s2], &dd, 100);
        assert_eq!(best, 0, "the FIRST of the tied pair");
        assert_eq!(len, 105);
    }

    #[test]
    fn a_steiner_points_x_is_mirrored_under_a_flip_but_its_y_is_not() {
        let xs = [0, 10, 20, 30, 40];
        let ys = [0, 1, 2, 3, 4];
        let s = [0usize, 1, 2, 3, 4];
        let d = 5;
        let mut c = Csoln::default();
        c.rowcol[0] = 0x21; // row = 2, col = 1
        let plain = build_tree(d, &xs, &ys, &s, &c, GroupIndex { k: 0, hflip: false }, 7);
        let flipped = build_tree(d, &xs, &ys, &s, &c, GroupIndex { k: 0, hflip: true }, 7);
        assert_eq!(plain.branch[d].x, xs[1], "col 1");
        assert_eq!(flipped.branch[d].x, xs[d - 1 - 1], "mirrored: d-1-col");
        assert_eq!(plain.branch[d].y, ys[2]);
        assert_eq!(flipped.branch[d].y, ys[2], "y is NOT mirrored");
    }

    #[test]
    fn the_flip_reverses_which_way_the_endpoint_comparison_runs() {
        let xs = [0, 10, 20, 30, 40];
        let ys = [0, 1, 2, 3, 4];
        let d = 5;
        let mut c = Csoln::default();
        c.neighbor[0] = 6;
        c.neighbor[1] = 7;
        // s[0] < s[1], so WITHOUT a flip the pair swaps; WITH one it does not.
        let s = [0usize, 1, 2, 3, 4];
        let plain = build_tree(d, &xs, &ys, &s, &c, GroupIndex { k: 0, hflip: false }, 0);
        let flipped = build_tree(d, &xs, &ys, &s, &c, GroupIndex { k: 0, hflip: true }, 0);
        assert_eq!((plain.branch[0].n, plain.branch[1].n), (7, 6));
        assert_eq!((flipped.branch[0].n, flipped.branch[1].n), (6, 7));
    }

    #[test]
    fn the_tree_has_two_d_minus_two_branches() {
        let xs = [0, 10, 20, 30, 40];
        let ys = [0, 1, 2, 3, 4];
        let s = [0usize, 1, 2, 3, 4];
        let t = build_tree(5, &xs, &ys, &s, &Csoln::default(),
                           GroupIndex { k: 0, hflip: false }, 42);
        assert_eq!(t.branch.len(), 2 * 5 - 2);
        assert_eq!(t.deg, 5);
        assert_eq!(t.length, 42);
    }
}