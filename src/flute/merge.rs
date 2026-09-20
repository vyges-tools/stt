// SPDX-License-Identifier: Apache-2.0
//! Merging two sub-trees back into one.
//!
//! `flutes_medium_degree` splits a net that decomposes cleanly, solves both halves, and stitches
//! the results. The stitching is pure index arithmetic and every offset matters: a branch's `n`
//! is an index into its OWN tree, so both halves must be renumbered into the combined one.

use crate::tree::{Branch, Tree};

/// `Flute::d_merge_tree` — join two sub-trees that share a terminal.
///
/// ```text
/// d       = t1.deg + t2.deg - 2;
/// offset1 = t2.deg - 2;
/// offset2 = 2 * t1.deg - 4;
/// ```
///
/// ⚠️ **The two halves are renumbered by DIFFERENT offsets**, and neither is the other's length.
/// `offset1` shifts t1's indices past t2's terminals; `offset2` shifts t2's past t1's branches.
/// Using one offset for both, or deriving either from `branch.len()`, silently rewires the tree —
/// every index would still be in range, so nothing would crash and the geometry would be wrong.
///
/// ⚠️ **The combined degree is `t1.deg + t2.deg - 2`, not the sum**: the two halves SHARE a
/// terminal, which is what makes the split valid in the first place.
pub fn d_merge_tree(t1: &Tree, t2: &Tree) -> Tree {
    let d = t1.deg + t2.deg - 2;
    let offset1 = t2.deg - 2;
    let offset2 = 2 * t1.deg - 4;
    let mut branch = vec![Branch { x: 0, y: 0, n: 0 }; 2 * d - 2];

    // t1's terminals, shifted past t2's.
    for i in 0..=(t1.deg - 2) {
        let b = t1.branch[i];
        branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset1 };
    }
    // t2's terminals, from its index 2 onward — index 0 and 1 are the shared ones.
    //
    // ⛔ **`i + 2 - t1.deg`, not `i - t1.deg + 2`.** The reference writes the latter and it is
    // correct there because the arithmetic is `int`: at the first iteration `i == t1.deg - 1`,
    // so `i - t1.deg` is -1 and `+ 2` brings it back to 1. In `usize` that intermediate wraps
    // to `usize::MAX`, and `+ 2` wraps back to the same answer — so a release build is
    // ACCIDENTALLY CORRECT and a debug build panics on the overflow check.
    //
    // 🔑 Reassociated so no intermediate is negative. `i >= t1.deg - 1` gives `i + 2 - t1.deg >= 1`.
    // See the C++-to-Rust numeric notes: transcribing an `int` expression into `usize` changes
    // the intermediates even when it does not change the result.
    for i in (t1.deg - 1)..=(d - 1) {
        let b = t2.branch[i + 2 - t1.deg];
        branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset2 };
    }
    // t1's Steiner points.
    for i in d..=(d + t1.deg - 3) {
        let b = t1.branch[i - offset1];
        branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset1 };
    }
    // t2's Steiner points.
    for i in (d + t1.deg - 2)..=(2 * d - 3) {
        let b = t2.branch[i - offset2];
        branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset2 };
    }

    // ⛔ **The copy loops are not the whole function here either.** Without this reversal t1's
    // own root survives as a SECOND self-loop and the merged tree has two components — the same
    // defect that made `v_merge_tree` hang, in the same family of functions, found the same way.
    //
    // ⚠️ Unlike the other two merges there is NO extra branch: `d_merge` fills every slot, so the
    // reversal seeds `prev` from t2's root (`t2.branch[0].n + offset2`) rather than from a new
    // node. That difference is why copying the tail across would also have been wrong.
    let mut prev = t2.branch[0].n + offset2;
    let mut curr = t1.branch[t1.deg - 1].n + offset1;
    let mut next = branch[curr].n;
    let mut guard = 0usize;
    while curr != next {
        branch[curr].n = prev;
        prev = curr;
        curr = next;
        next = branch[curr].n;
        guard += 1;
        if guard > 4 * branch.len() {
            break;
        }
    }
    branch[curr].n = prev;

    Tree { deg: d, length: t1.length + t2.length, branch }
}



/// `Flute::v_merge_tree` — join two halves split in the y direction.
///
/// ⛔ **Structurally identical to [`d_merge_tree`] with EVERY offset one larger.** `deg` is
/// `t1.deg + t2.deg - 1` rather than `- 2`, `offset1` is `t2.deg - 1` rather than `- 2`, and
/// `offset2` is `2*t1.deg - 3` rather than `- 4`. Deriving one of these from the other by
/// "adjusting" is how they get transposed, and every index stays in range when they are.
pub fn v_merge_tree(t1: &Tree, t2: &Tree) -> Tree {
    let deg = t1.deg + t2.deg - 1;
    let offset1 = t2.deg - 1;
    let offset2 = 2 * t1.deg - 3;
    let mut branch = vec![Branch { x: 0, y: 0, n: 0 }; 2 * deg - 2];

    for i in 0..=(t1.deg - 2) {
        let b = t1.branch[i];
        branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset1 };
    }
    // ⛔ `i + 1 - t1.deg`, not `i - t1.deg + 1` — same reassociation as `d_merge_tree`'s. The
    // reference's `int` lets the intermediate go to -1; `usize` wraps, and only a debug build
    // says so.
    for i in (t1.deg - 1)..=(deg - 1) {
        let b = t2.branch[i + 1 - t1.deg];
        branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset2 };
    }
    for i in deg..=(deg + t1.deg - 3) {
        let b = t1.branch[i - offset1];
        branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset1 };
    }
    for i in (deg + t1.deg - 2)..=(2 * deg - 4) {
        let b = t2.branch[i - offset2];
        branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset2 };
    }

    // ⛔ **The four copy loops are NOT the whole function.** They leave the last slot,
    // `2*deg - 3`, unwritten and the two halves as SEPARATE COMPONENTS — each sub-tree keeps its
    // own self-referencing root. Stopping here produced a tree with two roots, and
    // `local_refinement`'s unbounded pin-walk then spun forever on the orphaned one.
    //
    // The tail below is what joins them, and it mirrors `h_merge_tree`'s with one axis swapped:
    // the extra branch's **x** is clamped here where h_merge clamps its **y**.
    let mut length = t1.length + t2.length;
    let extra = 2 * deg - 3;
    let coord1 = t1.branch[t1.branch[t1.deg - 1].n].x;
    let coord2 = t2.branch[t2.branch[0].n].x;
    let joiner_x = t2.branch[0].x;
    let (lo, hi) = (coord1.min(coord2), coord1.max(coord2));
    branch[extra].x = if joiner_x > hi {
        length -= i64::from(joiner_x - hi);
        hi
    } else if joiner_x < lo {
        length -= i64::from(lo - joiner_x);
        lo
    } else {
        joiner_x
    };
    branch[extra].y = t2.branch[0].y;
    // ⚠️ The relink is at `t1.deg - 1` — the SHARED terminal — not at a recorded join index.
    branch[extra].n = branch[t1.deg - 1].n;
    branch[t1.deg - 1].n = extra;

    let mut prev = extra;
    let mut curr = t1.branch[t1.deg - 1].n + offset1;
    let mut next = branch[curr].n;
    let mut guard = 0usize;
    while curr != next {
        branch[curr].n = prev;
        prev = curr;
        curr = next;
        next = branch[curr].n;
        guard += 1;
        if guard > 4 * branch.len() {
            break;
        }
    }
    branch[curr].n = prev;

    Tree { deg, length, branch }
}

/// `Flute::h_merge_tree` — join two halves split in the x direction.
///
/// Three things happen here that no other merge does:
///
/// 1. **The halves are INTERLEAVED by `s`**, not concatenated: each output position takes from
///    t1 or t2 according to `s[i] < p`, `> p` or `== p`, where `p = t1.deg - 1`. The `== p` case
///    takes from **t2** and records where the join happened.
/// 2. **An EXTRA branch is appended** at `2*deg - 3` — the last slot — whose y is CLAMPED between
///    the two joining coordinates, and the tree's length is reduced by exactly the amount clamped
///    away.
/// 3. ⛔ **A parent-pointer path is REVERSED**, re-rooting the tree at that new node. The walk
///    stops at the self-referencing root (`curr == next`) and then points it back at `prev`.
///    Without the reversal the tree still has valid indices and is wrongly connected.
pub fn h_merge_tree(t1: &Tree, t2: &Tree, s: &[usize]) -> Tree {
    let deg = t1.deg + t2.deg - 1;
    let offset1 = t2.deg - 1;
    let offset2 = 2 * t1.deg - 3;
    let mut branch = vec![Branch { x: 0, y: 0, n: 0 }; 2 * deg - 2];

    let p = t1.deg - 1;
    let (mut n1, mut n2) = (0usize, 0usize);
    let (mut nn1, mut nn2, mut ii) = (0usize, 0usize, 0usize);
    for i in 0..deg {
        if s[i] < p {
            let b = t1.branch[n1];
            branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset1 };
            n1 += 1;
        } else if s[i] > p {
            let b = t2.branch[n2];
            branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset2 };
            n2 += 1;
        } else {
            // ⚠️ The shared position takes from **t2**, and BOTH counters advance.
            let b = t2.branch[n2];
            branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset2 };
            nn1 = n1;
            nn2 = n2;
            ii = i;
            n1 += 1;
            n2 += 1;
        }
    }
    for i in deg..=(deg + t1.deg - 3) {
        let b = t1.branch[i - offset1];
        branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset1 };
    }
    for i in (deg + t1.deg - 2)..=(2 * deg - 4) {
        let b = t2.branch[i - offset2];
        branch[i] = Branch { x: b.x, y: b.y, n: b.n + offset2 };
    }

    let mut length = t1.length + t2.length;
    let extra = 2 * deg - 3;
    let coord1 = t1.branch[t1.branch[nn1].n].y;
    let coord2 = t2.branch[t2.branch[nn2].n].y;
    let joiner_y = t2.branch[nn2].y;
    let (lo, hi) = (coord1.min(coord2), coord1.max(coord2));
    branch[extra].y = if joiner_y > hi {
        length -= i64::from(joiner_y - hi);
        hi
    } else if joiner_y < lo {
        length -= i64::from(lo - joiner_y);
        lo
    } else {
        joiner_y
    };
    branch[extra].x = t2.branch[nn2].x;
    branch[extra].n = branch[ii].n;
    branch[ii].n = extra;

    // ⛔ Re-root: walk from t1's join point to the root, reversing every parent pointer.
    let mut prev = extra;
    let mut curr = t1.branch[nn1].n + offset1;
    let mut next = branch[curr].n;
    let mut guard = 0usize;
    while curr != next {
        branch[curr].n = prev;
        prev = curr;
        curr = next;
        next = branch[curr].n;
        guard += 1;
        if guard > 4 * branch.len() {
            break;
        }
    }
    branch[curr].n = prev;

    Tree { deg, length, branch }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(deg: usize, pts: &[(i32, i32, usize)]) -> Tree {
        Tree {
            deg,
            length: 10,
            branch: pts.iter().map(|&(x, y, n)| Branch { x, y, n }).collect(),
        }
    }

    #[test]
    fn the_merged_degree_subtracts_the_shared_terminal() {
        // ⚠️ NOT t1.deg + t2.deg. The halves share a terminal, which is what makes the split
        // legal — summing the degrees would claim a pin that does not exist.
        let t1 = tree(4, &[(0, 0, 4), (1, 1, 4), (2, 2, 5), (3, 3, 5), (1, 1, 5), (2, 2, 5)]);
        let t2 = tree(4, &[(4, 4, 4), (5, 5, 4), (6, 6, 5), (7, 7, 5), (5, 5, 5), (6, 6, 5)]);
        let m = d_merge_tree(&t1, &t2);
        assert_eq!(m.deg, 6, "4 + 4 - 2");
        assert_eq!(m.branch.len(), 2 * 6 - 2);
        assert_eq!(m.length, 20, "lengths add");
    }

    #[test]
    fn the_two_halves_are_renumbered_by_different_offsets() {
        // 🔑 offset1 = t2.deg - 2 = 2 and offset2 = 2*t1.deg - 4 = 4 here. A single offset, or
        // one derived from branch.len(), would leave every index in range and the tree rewired.
        let t1 = tree(4, &[(0, 0, 4), (1, 1, 4), (2, 2, 5), (3, 3, 5), (1, 1, 5), (2, 2, 5)]);
        let t2 = tree(4, &[(4, 4, 4), (5, 5, 4), (6, 6, 5), (7, 7, 5), (5, 5, 5), (6, 6, 5)]);
        let m = d_merge_tree(&t1, &t2);
        // t1's branch 0 had n = 4; shifted by offset1 = 2 it becomes 6.
        assert_eq!(m.branch[0].n, 4 + 2);
        // ⚠️ The index mapping is `t2.branch[i + 2 - t1.deg]`, so combined index 3 takes t2's
        // branch 1 — NOT branch 2. Getting that wrong was the first version of this assertion,
        // and it is the same off-by-two the offsets themselves invite.
        assert_eq!(m.branch[3].n, t2.branch[3 + 2 - t1.deg].n + 4);
        assert_eq!(m.branch[3].x, t2.branch[1].x, "and the coordinates come from branch 1");
    }

    #[test]
    fn v_merge_offsets_are_each_ONE_LARGER_than_d_merge() {
        // ⛔ The two differ only by a 1 in every offset, and both leave indices in range when
        // transposed. Asserting the relationship directly is the only way to catch a copy.
        let t1 = tree(4, &[(0, 0, 4), (1, 1, 4), (2, 2, 5), (3, 3, 5), (1, 1, 5), (2, 2, 5)]);
        let t2 = tree(4, &[(4, 4, 4), (5, 5, 4), (6, 6, 5), (7, 7, 5), (5, 5, 5), (6, 6, 5)]);
        let dm = d_merge_tree(&t1, &t2);
        let vm = v_merge_tree(&t1, &t2);
        assert_eq!(dm.deg + 1, vm.deg, "v_merge shares one fewer terminal");
        assert_eq!(vm.branch.len(), 2 * vm.deg - 2);
        assert!(vm.branch.iter().all(|b| b.n < vm.branch.len()));
    }

    #[test]
    fn h_merge_appends_an_extra_branch_and_clamps_its_y() {
        // The extra sits in the LAST slot, 2*deg-3, and its y is clamped between the two
        // joining coordinates; the length drops by exactly what the clamp removed.
        let t1 = tree(3, &[(0, 0, 3), (5, 50, 3), (10, 10, 3), (5, 10, 3)]);
        let t2 = tree(3, &[(10, 10, 3), (15, 15, 3), (20, 20, 3), (15, 15, 3)]);
        let s = [0usize, 1, 2, 3, 4];
        let m = h_merge_tree(&t1, &t2, &s);
        let extra = 2 * m.deg - 3;
        assert_eq!(m.branch.len(), 2 * m.deg - 2);
        assert_eq!(m.branch[extra].x, t2.branch[0].x, "x comes from the joiner, unclamped");
        assert!(m.length <= t1.length + t2.length, "the clamp can only shorten");
    }

    #[test]
    fn h_merge_reroots_so_exactly_one_branch_is_its_own_parent() {
        // 🔑 The path reversal re-roots the tree. A correctly rooted tree has EXACTLY ONE
        // self-referencing branch; skipping the reversal leaves indices valid and the tree
        // wrongly connected, which no range check would notice.
        let t1 = tree(3, &[(0, 0, 3), (5, 50, 3), (10, 10, 3), (5, 10, 3)]);
        let t2 = tree(3, &[(10, 10, 3), (15, 15, 3), (20, 20, 3), (15, 15, 3)]);
        let s = [0usize, 1, 2, 3, 4];
        let m = h_merge_tree(&t1, &t2, &s);
        let roots = (0..m.branch.len()).filter(|&i| m.branch[i].n == i).count();
        assert_eq!(roots, 1, "exactly one self-referencing root after re-rooting");
        assert!(m.branch.iter().all(|b| b.n < m.branch.len()));
    }

    #[test]
    fn every_merge_leaves_exactly_ONE_self_referencing_root() {
        // ⛔ The invariant that all three merges must hold, and the one that caught two
        // truncated transcriptions: each sub-tree arrives with its OWN root, and the tail of
        // each merge is what joins them. Without it the result has TWO self-loops — two
        // components — with every index still in range, so nothing crashes and a later walk
        // spins forever.
        let t1 = tree(4, &[(0, 0, 4), (1, 1, 4), (2, 2, 5), (3, 3, 5), (1, 1, 5), (5, 5, 5)]);
        let t2 = tree(4, &[(4, 4, 4), (5, 5, 4), (6, 6, 5), (7, 7, 5), (5, 5, 5), (9, 9, 5)]);
        let s = [0usize, 1, 2, 3, 4, 5, 6, 7];
        for (name, m) in [
            ("d_merge", d_merge_tree(&t1, &t2)),
            ("v_merge", v_merge_tree(&t1, &t2)),
            ("h_merge", h_merge_tree(&t1, &t2, &s)),
        ] {
            let roots: Vec<usize> =
                (0..m.branch.len()).filter(|&i| m.branch[i].n == i).collect();
            assert_eq!(roots.len(), 1, "{name}: expected one root, found {roots:?}");
            assert!(m.branch.iter().all(|b| b.n < m.branch.len()), "{name}: indices in range");
        }
    }

    #[test]
    fn every_neighbour_index_of_a_merged_tree_is_in_range() {
        // The property that a wrong offset would break without crashing.
        let t1 = tree(5, &[(0, 0, 6), (1, 1, 6), (2, 2, 7), (3, 3, 7), (4, 4, 7),
                           (1, 1, 7), (2, 2, 7), (3, 3, 7)]);
        let t2 = tree(4, &[(9, 9, 4), (8, 8, 4), (7, 7, 5), (6, 6, 5), (8, 8, 5), (7, 7, 5)]);
        let m = d_merge_tree(&t1, &t2);
        assert!(m.branch.iter().all(|b| b.n < m.branch.len()),
                "every neighbour must address a branch that exists");
    }
}