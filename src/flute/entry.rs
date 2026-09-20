// SPDX-License-Identifier: Apache-2.0
//! `Flute::flute` — the entry point that orders the points and hands off to `flutes`.
//!
//! Its job is entirely preparation: two degenerate cases, then an x-ordering, then a y-ordering
//! that yields the `s[]` permutation the table lookup is indexed by.

use crate::tree::{Branch, Tree};

/// Below this degree the reference uses a hand-written selection sort; at or above it, a
/// `std::stable_sort`.
///
/// ⛔ **The two are not interchangeable, and this is a real behavioural fork.** Selection sort is
/// NOT stable, so points with equal coordinates come out in a different order on either side of
/// this bound. Picking one sort for all degrees would agree with the reference on one side and
/// disagree on the other, and only on nets that have ties.
pub const SORT_CROSSOVER_DEGREE: usize = 200;

/// A point, carrying the index it landed at after the x-ordering.
#[derive(Debug, Clone, Copy, Default)]
struct Pt {
    x: i32,
    y: i32,
    /// `o` in the reference — the position this point took in x-order.
    o: usize,
}

/// The prepared inputs `flutes` consumes: x-ordered xs, y-ordered ys, and `s` mapping y-order
/// back to x-order.
#[derive(Debug, PartialEq, Eq)]
pub struct Prepared {
    pub xs: Vec<i32>,
    pub ys: Vec<i32>,
    pub s: Vec<usize>,
}

/// Order the points exactly as `Flute::flute` does.
pub fn prepare(x: &[i32], y: &[i32]) -> Prepared {
    let d = x.len();
    let mut pt: Vec<Pt> = (0..d).map(|i| Pt { x: x[i], y: y[i], o: 0 }).collect();
    // The reference sorts an array of POINTERS and leaves the points themselves in place; an
    // index permutation is the same thing without the aliasing.
    let mut ptp: Vec<usize> = (0..d).collect();

    // ── sort by x ────────────────────────────────────────────────────────────────────────────
    if d < SORT_CROSSOVER_DEGREE {
        // Selection sort, with `std::swap` — the reference's own idiom here.
        for i in 0..d.saturating_sub(1) {
            let mut minval = pt[ptp[i]].x;
            let mut minidx = i;
            for j in (i + 1)..d {
                // ⚠️ STRICTLY greater: a tie does NOT move the minimum, so the earlier point
                // wins. `>=` would reverse every tie.
                if minval > pt[ptp[j]].x {
                    minval = pt[ptp[j]].x;
                    minidx = j;
                }
            }
            ptp.swap(i, minidx);
        }
    } else {
        // ⚠️ `stable_sort`, so ties keep their original relative order — which the selection
        // sort above does not guarantee.
        ptp.sort_by(|&a, &b| pt[a].x.cmp(&pt[b].x));
    }

    let mut xs = vec![0i32; d];
    for i in 0..d {
        xs[i] = pt[ptp[i]].x;
        pt[ptp[i]].o = i;
    }

    // ── sort by y, extracting ys and s as it goes ────────────────────────────────────────────
    let mut ys = vec![0i32; d];
    let mut s = vec![0usize; d];
    if d < SORT_CROSSOVER_DEGREE {
        for i in 0..d.saturating_sub(1) {
            let mut minval = pt[ptp[i]].y;
            let mut minidx = i;
            for j in (i + 1)..d {
                if minval > pt[ptp[j]].y {
                    minval = pt[ptp[j]].y;
                    minidx = j;
                }
            }
            ys[i] = pt[ptp[minidx]].y;
            s[i] = pt[ptp[minidx]].o;
            // ⚠️ **NOT a swap.** The reference writes `ptp[minidx] = ptp[i]` — a one-way
            // overwrite that discards what was at `minidx` and leaves `ptp[i]` untouched. The
            // effect on the remaining range [i+1, d) matches a swap, and `i` is never revisited,
            // so the two agree here — but the x-sort above really does use `std::swap`, and
            // writing the same idiom in both places would be transcribing a similarity that is
            // not in the source.
            ptp[minidx] = ptp[i];
        }
        if d > 0 {
            ys[d - 1] = pt[ptp[d - 1]].y;
            s[d - 1] = pt[ptp[d - 1]].o;
        }
    } else {
        ptp.sort_by(|&a, &b| pt[a].y.cmp(&pt[b].y));
        for i in 0..d {
            ys[i] = pt[ptp[i]].y;
            s[i] = pt[ptp[i]].o;
        }
    }

    Prepared { xs, ys, s }
}

/// The degenerate trees `flute` returns before any table is consulted.
///
/// ⚠️ **Both branches of the two-pin tree point at index 1**, including branch 1 itself. That is
/// the reference's literal initialiser — a self-reference at the root is how the walk terminates,
/// and "obviously" making branch 1 point at 0 would create a cycle.
pub fn degenerate(x: &[i32], y: &[i32]) -> Option<Tree> {
    match x.len() {
        0 | 1 => Some(Tree { deg: 1, length: 0, branch: Vec::new() }),
        2 => Some(Tree {
            deg: 2,
            length: i64::from((x[0] - x[1]).abs()) + i64::from((y[0] - y[1]).abs()),
            branch: vec![
                Branch { x: x[0], y: y[0], n: 1 },
                Branch { x: x[1], y: y[1], n: 1 },
            ],
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_net_below_two_pins_is_a_degree_one_empty_tree() {
        let t = degenerate(&[5], &[7]).unwrap();
        assert_eq!(t.deg, 1);
        assert_eq!(t.length, 0);
        assert!(t.branch.is_empty(), "no branches at all, not one self-referencing branch");
    }

    #[test]
    fn a_two_pin_tree_has_both_branches_pointing_at_index_one() {
        // ⚠️ Branch 1 points at ITSELF. That is the reference's initialiser verbatim; making it
        // point at 0 would close a cycle and the tree walk would not terminate.
        let t = degenerate(&[0, 10], &[0, 4]).unwrap();
        assert_eq!(t.deg, 2);
        assert_eq!(t.length, 14, "|dx| + |dy|");
        assert_eq!(t.branch[0].n, 1);
        assert_eq!(t.branch[1].n, 1, "self-reference at the root");
    }

    #[test]
    fn preparation_orders_x_and_derives_s_from_the_y_order() {
        // Three points, deliberately out of order in both axes.
        //   index 0: (30, 1)   index 1: (10, 3)   index 2: (20, 2)
        // x-order: 10, 20, 30  -> xs
        // y-order: 1, 2, 3     -> the point at y=1 is x=30, which sits at x-position 2
        let p = prepare(&[30, 10, 20], &[1, 3, 2]);
        assert_eq!(p.xs, vec![10, 20, 30]);
        assert_eq!(p.ys, vec![1, 2, 3]);
        assert_eq!(p.s, vec![2, 1, 0], "s maps y-order back to x-order");
    }

    #[test]
    fn a_tie_in_x_keeps_the_earlier_point_because_the_test_is_strictly_greater() {
        // Two points share x. `minval > ...` is STRICT, so the first one encountered stays put.
        // A `>=` test would swap them and every downstream index would shift.
        let p = prepare(&[10, 10, 5], &[7, 1, 3]);
        assert_eq!(p.xs, vec![5, 10, 10]);
        // The y-order is 1, 3, 7 — i.e. the point (10,1), then (5,3), then (10,7).
        assert_eq!(p.ys, vec![1, 3, 7]);
        // (10,1) was the SECOND of the tied pair in input order and must land at x-position 1,
        // not 2, if the tie kept the earlier point first.
        assert_eq!(p.s, vec![1, 0, 2]);
    }

    #[test]
    fn the_prepared_arrays_are_a_permutation_of_the_input() {
        let x = [4, 9, 1, 7, 3];
        let y = [8, 2, 6, 5, 0];
        let p = prepare(&x, &y);
        let mut sx = x.to_vec();
        sx.sort_unstable();
        let mut sy = y.to_vec();
        sy.sort_unstable();
        assert_eq!(p.xs, sx);
        assert_eq!(p.ys, sy);
        let mut seen = p.s.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..x.len()).collect::<Vec<_>>(), "s is a permutation");
    }
}