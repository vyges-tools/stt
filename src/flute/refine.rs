// SPDX-License-Identifier: Apache-2.0
//! `Flute::local_refinement` — re-solve the neighbourhood around a break.
//!
//! After two halves are merged, the join is often not optimal. This re-roots the tree at the
//! break, collects the pins that hang directly off that root, and if there are between 4 and 9
//! of them re-solves just those through the lookup table and splices the answer back in.
//!
//! ⚠️ `kLocalRefinement` is **always true** in the reference, so this is not optional: a merged
//! tree that skips it is a different tree.

use super::low_degree::flutes_low_degree;
use super::lut::{Lut, MAX_LUT_DEGREE};
use crate::tree::Tree;

/// Marks a Steiner node already claimed while walking to the root.
const CLAIMED: i64 = i64::MAX;

/// A parent chain that does not terminate — the merged tree contains a cycle.
///
/// ⛔ **Named rather than `()`** so a caller cannot read the refusal as "nothing interesting".
/// This is OUR defect surfacing, not a property of the net, and the distinction is the whole
/// reason the refusal exists instead of a tree built on a structure we know is wrong.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Cycle;

/// Returns `Err(Cycle)` when the tree handed in is not walkable.
///
/// ⛔ **The reference has no such guard**, because its merged trees are acyclic by construction.
/// Ours are not yet: `h_merge_tree`'s re-rooting leaves a cycle on some inputs, and the
/// reference's unbounded `while (steiner_pin[curr] < 0) curr = branch[curr].n;` then spins
/// forever. Detecting it and refusing is strictly better than hanging, and strictly better than
/// returning a tree built from a structure we know is wrong — but it IS a defect of ours to fix,
/// not a divergence to keep.
pub fn local_refinement(lut: &Lut, deg: usize, t: &mut Tree, p: usize) -> Result<(), Cycle> {
    let root = t.branch[p].n;

    // ── re-root at `root`, reversing the parent pointers on the way ─────────────────────────
    // ⚠️ The SAME reversal shape as `h_merge_tree`'s, but it starts from `branch[root].n` rather
    // than from a join index, and it finishes by making `root` its own parent.
    let mut prev = root;
    let mut curr = t.branch[prev].n;
    let mut next = t.branch[curr].n;
    let mut guard = 0usize;
    while curr != next {
        t.branch[curr].n = prev;
        prev = curr;
        curr = next;
        next = t.branch[curr].n;
        guard += 1;
        if guard > 4 * t.branch.len() { return Err(Cycle); }
    }
    t.branch[curr].n = prev;
    t.branch[root].n = root;

    let d = t.deg;
    let degree = deg + 1;

    // ── which Steiner nodes sit exactly on a pin ────────────────────────────────────────────
    // ⚠️ `steiner_pin` is -1 for unclaimed, a pin index when a Steiner node coincides with that
    // pin, and INT_MAX once claimed by the walk below. Three states in one array.
    let mut steiner_pin = vec![-1i64; 2 * degree];
    for i in 0..d {
        let nx = t.branch[i].n;
        if t.branch[i].x == t.branch[nx].x && t.branch[i].y == t.branch[nx].y {
            steiner_pin[nx] = i as i64;
        }
    }
    steiner_pin[root] = p as i64;

    // ── the pins hanging directly off the root ──────────────────────────────────────────────
    let mut index = vec![0usize; 2 * degree];
    let mut x = vec![0i32; degree];
    let mut dd = 0usize;
    for i in 0..d {
        let mut curr = t.branch[i].n;
        // ⚠️ A pin whose own Steiner node coincides with it steps once more before the walk —
        // otherwise it would terminate immediately on itself.
        if steiner_pin[curr] == i as i64 {
            curr = t.branch[curr].n;
        }
        let mut g = 0usize;
        while steiner_pin[curr] < 0 {
            curr = t.branch[curr].n;
            g += 1;
                if g > 4 * t.branch.len() { return Err(Cycle); }
        }
        if curr == root {
            x[dd] = t.branch[i].x;
            // A pin with a coincident Steiner node contributes the STEINER node, unless that
            // node is the root itself.
            index[dd] = if steiner_pin[t.branch[i].n] == i as i64 && t.branch[i].n != root {
                t.branch[i].n
            } else {
                i
            };
            dd += 1;
        }
    }

    // ⛔ The refinement only fires for a neighbourhood the LOOKUP TABLE can solve. Outside
    // 4..=9 the merged tree stands as it is.
    if !(4..=MAX_LUT_DEGREE).contains(&dd) {
        return Ok(());
    }

    // Steiner nodes between those pins and the root, appended after the pins.
    let mut ii = dd;
    for i in 0..dd {
        let mut curr = t.branch[index[i]].n;
        let mut g = 0usize;
        while steiner_pin[curr] < 0 {
            index[ii] = curr;
            ii += 1;
            steiner_pin[curr] = CLAIMED;
            curr = t.branch[curr].n;
            g += 1;
            if g > 4 * t.branch.len() { return Err(Cycle); }
        }
    }
    index[ii] = root;

    // ── re-solve that neighbourhood ─────────────────────────────────────────────────────────
    // ⚠️ `ss[]` is built by COUNTING, not by sorting: strictly-less before `ii`, less-or-EQUAL
    // after it. That asymmetry is what breaks ties consistently, and a plain sort would order
    // equal x values differently.
    let mut ss = vec![0usize; degree];
    let mut xs = vec![0i32; degree];
    let mut ys = vec![0i32; degree];
    for i in 0..dd {
        ss[i] = 0;
        for j in 0..i {
            if x[j] < x[i] {
                ss[i] += 1;
            }
        }
        for j in (i + 1)..dd {
            if x[j] <= x[i] {
                ss[i] += 1;
            }
        }
        xs[ss[i]] = x[i];
        ys[i] = t.branch[index[i]].y;
    }

    let tt = flutes_low_degree(lut, dd, &xs, &ys, &ss);

    // ── splice it back, adjusting the length ────────────────────────────────────────────────
    // ⚠️ The new length is added FIRST and the old edges subtracted after, over `2*dd-3`
    // branches — one short of the sub-tree's full branch count.
    t.length += tt.length;
    for i in 0..(2 * dd - 3) {
        let a = index[i];
        let b = t.branch[a].n;
        t.length -= i64::from((t.branch[a].x - t.branch[b].x).abs())
            + i64::from((t.branch[a].y - t.branch[b].y).abs());
    }
    // The first `dd` are pins: only their parent changes. The rest are Steiner nodes and take
    // their coordinates from the sub-tree too.
    for i in 0..dd {
        t.branch[index[i]].n = index[tt.branch[i].n];
    }
    for i in dd..=(2 * dd - 3) {
        t.branch[index[i]].x = tt.branch[i].x;
        t.branch[index[i]].y = tt.branch[i].y;
        t.branch[index[i]].n = index[tt.branch[i].n];
    }
    Ok(())
}

/// `Flute::wirelength` — the length of a tree as the reference measures it.
pub fn wirelength(t: &Tree) -> i64 {
    (0..(2 * t.deg - 2))
        .map(|i| {
            let b = t.branch[i];
            let n = t.branch[b.n];
            i64::from((b.x - n.x).abs()) + i64::from((b.y - n.y).abs())
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flute::entry::prepare;
    use crate::flute::low_degree::flutes_low_degree;
    use crate::flute::lut::load_tables;

    #[test]
    fn wirelength_sums_every_branch_to_its_neighbour() {
        let lut = load_tables(MAX_LUT_DEGREE).unwrap();
        let p = prepare(&[0, 10, 5, 7], &[0, 4, 9, 2]);
        let t = flutes_low_degree(&lut, 4, &p.xs, &p.ys, &p.s);
        assert_eq!(wirelength(&t), t.length, "the reported length is the measured one");
    }

    #[test]
    fn refinement_leaves_a_tree_it_cannot_solve_untouched() {
        // ⛔ Outside 4..=9 pins on the root the merged tree stands. A degree-4 tree has too few
        // to refine, so nothing may change — including the length.
        let lut = load_tables(MAX_LUT_DEGREE).unwrap();
        let p = prepare(&[0, 10, 5, 7], &[0, 4, 9, 2]);
        let mut t = flutes_low_degree(&lut, 4, &p.xs, &p.ys, &p.s);
        let before = t.clone();
        let _ = local_refinement(&lut, 4, &mut t, 1);
        assert_eq!(t.length, before.length, "an unrefinable tree keeps its length");
    }

    #[test]
    fn refinement_keeps_the_tree_well_formed_and_never_lengthens_it() {
        // 🔑 The property that matters: refinement is an optimisation, so it may shorten a tree
        // but must never lengthen one, and the result must still be a tree.
        let lut = load_tables(MAX_LUT_DEGREE).unwrap();
        for seed in 0..12i32 {
            let d = 9usize;
            let x: Vec<i32> = (0..d).map(|i| (i as i32 * (13 + seed)) % 97).collect();
            let y: Vec<i32> = (0..d).map(|i| (i as i32 * (7 + seed) + 5) % 89).collect();
            let p = prepare(&x, &y);
            let mut t = flutes_low_degree(&lut, d, &p.xs, &p.ys, &p.s);
            let before = t.length;
            if local_refinement(&lut, d, &mut t, 2).is_err() { continue; }
            assert!(t.branch.iter().all(|b| b.n < t.branch.len()),
                    "seed {seed}: every neighbour index stays in range");
            assert!(t.length <= before, "seed {seed}: refinement must not lengthen");
            let roots = (0..t.branch.len()).filter(|&i| t.branch[i].n == i).count();
            assert_eq!(roots, 1, "seed {seed}: still exactly one root");
        }
    }
}