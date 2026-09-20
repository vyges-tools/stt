// SPDX-License-Identifier: Apache-2.0
//! **F4** — the gate that decides whether a Prim-Dijkstra tree is used at all.
//!
//! ⛔ **This is not a diagnostic.** The reference's `makeSteinerTree` runs Prim-Dijkstra when
//! `alpha > 0`, and then:
//!
//! ```text
//! if (checkTree(tree)) { return tree; }
//! // Fall back to flute if PD fails.
//! ```
//!
//! A tree that fails here is DISCARDED and FLUTE builds the net instead. Getting this predicate
//! wrong does not produce a wrong tree at this stage — it silently routes the net through the
//! other builder, and the difference shows up as a completely different topology somewhere with
//! nothing to point at.

use crate::tree::{Rect, Tree};

/// Degree above which the check is skipped outright.
///
/// ⚠️ The reference's reason, verbatim: *"Such high fanout nets are going to get buffered so we
/// don't need to worry about them."* It is a deliberate escape, not a performance guard, so the
/// threshold is part of the behaviour.
pub const MAX_CHECKED_DEGREE: usize = 100;

/// Does this tree pass the reference's self-overlap check?
///
/// Two branch segments fail the check when **both are non-degenerate, they intersect, and they do
/// not merely share a corner**. Anything else passes.
pub fn check_tree(tree: &Tree) -> bool {
    if tree.deg > MAX_CHECKED_DEGREE {
        return true;
    }

    // One rectangle per branch, from the branch to its neighbour. Built for EVERY branch first,
    // in index order, because the pairwise loop below indexes into this array by branch index.
    let rects: Vec<Rect> = (0..tree.branch_count())
        .map(|i| {
            let b = tree.branch[i];
            let n = tree.branch[b.n];
            Rect::new(b.x, b.y, n.x, n.y)
        })
        .collect();

    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let (r1, r2) = (&rects[i], &rects[j]);
            if !r1.is_point() && !r2.is_point() && r1.intersects(r2) && !r1.shares_corner(r2) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Branch;

    fn tree(deg: usize, pts: &[(i32, i32, usize)]) -> Tree {
        Tree {
            deg,
            length: 0,
            branch: pts.iter().map(|&(x, y, n)| Branch { x, y, n }).collect(),
        }
    }

    /// Upstream's own case, replicated: `src/stt/test/check.tcl`, net `cross1`, alpha 0.3.
    ///
    /// 🔑 **Real reference data, not a shape I invented.** The case is titled *"Nets with cross
    /// over/overlap"* and turns on `set_debug_level STT check 1` to *"squack when segment overlap
    /// is detected"* — it exists precisely to drive this predicate. The tree below is its
    /// `check.ok` golden verbatim: 11 terminals, 4 Steiner points, wire length 37, path depth 27.
    ///
    /// ⚠️ The golden carries **no** `check failed` line, so the tree it records is one this
    /// predicate ACCEPTS. If `check_tree` rejected it, the reference would have discarded it and
    /// returned FLUTE's tree instead, and the golden would show a different topology.
    fn cross1() -> Tree {
        Tree {
            deg: 11,
            length: 37,
            branch: [
                (72, 61, 0), (86, 50, 12), (84, 51, 14), (89, 51, 14), (86, 53, 5),
                (86, 58, 13), (85, 59, 13), (80, 53, 8), (79, 56, 11), (81, 58, 11),
                (79, 60, 0), (79, 58, 10), (86, 51, 4), (85, 58, 9), (86, 51, 12),
            ].iter().map(|&(x, y, n)| Branch { x, y, n }).collect(),
        }
    }

    #[test]
    fn upstreams_own_cross1_tree_is_accepted() {
        let t = cross1();
        assert_eq!(t.branch_count(), 15, "11 terminals + 4 Steiner points");
        assert!(check_tree(&t), "the golden records a tree the reference RETURNED, so it passed");
    }

    #[test]
    fn upstreams_cross1_wirelength_matches_the_golden() {
        // `Wire length = 37` in check.ok. Summing each branch's own length double-counts nothing
        // here because every branch contributes its own edge to its neighbour exactly once.
        let t = cross1();
        let total: i64 = (0..t.branch_count()).map(|i| t.branch_length(i)).sum();
        assert_eq!(total, 37, "wire length as the golden reports it");
    }

    #[test]
    fn a_degree_above_one_hundred_skips_the_check_entirely() {
        // ⚠️ Upstream's reason, verbatim: "Such high fanout nets are going to get buffered so we
        // don't need to worry about them." It is a deliberate escape, so the threshold is
        // behaviour: a tree that WOULD fail must still pass above it.
        let overlapping = [(0, 0, 1), (10, 0, 0), (5, -5, 3), (5, 5, 2)];
        assert!(!check_tree(&tree(4, &overlapping)), "it really does fail when checked");
        assert!(check_tree(&tree(101, &overlapping)), "and is waved through above 100");
        assert!(!check_tree(&tree(100, &overlapping)), "the bound is EXCLUSIVE: 100 is checked");
    }

    #[test]
    fn a_horizontal_branch_is_not_zero_area_despite_the_name() {
        // ⛔ `rectAreaZero` is `xMin == xMax && yMin == yMax` — a POINT. Reading the NAME instead
        // of the body would treat every axis-aligned branch as zero-area and exclude it from the
        // check, which is all of them, and the gate would never fail.
        use crate::tree::Rect;
        assert!(!Rect::new(0, 0, 10, 0).is_point(), "horizontal: NOT a point");
        assert!(!Rect::new(0, 0, 0, 10).is_point(), "vertical: NOT a point");
        assert!(Rect::new(7, 7, 7, 7).is_point(), "both collapsed: a point");
    }

    #[test]
    fn two_branches_crossing_fail_but_two_meeting_at_a_corner_pass() {
        // A cross: (0,0)-(10,0) horizontal and (5,-5)-(5,5) vertical intersect away from any
        // corner, so the tree is rejected and the net falls through to FLUTE.
        let crossing = tree(4, &[(0, 0, 1), (10, 0, 0), (5, -5, 3), (5, 5, 2)]);
        assert!(!check_tree(&crossing));

        // An L: the two segments share the corner (10, 0), which is explicitly allowed.
        let elbow = tree(4, &[(0, 0, 1), (10, 0, 0), (10, 0, 3), (10, 10, 2)]);
        assert!(check_tree(&elbow), "sharing a corner is not an overlap");
    }

    #[test]
    fn a_point_branch_never_triggers_the_check() {
        // Zero-length branches are ignored by "picking one end as the representative" — a
        // degenerate branch sitting on top of a real one must not reject the tree.
        let with_point = tree(3, &[(0, 0, 1), (10, 0, 0), (5, 0, 2)]);
        assert!(check_tree(&with_point), "a point on a segment is not an overlap");
    }

    #[test]
    fn the_rectangle_normalises_its_corners() {
        // 🔑 `odb::Rect::init` is `std::tie(xlo_, xhi_) = std::minmax(x1, x2)` on both axes, so
        // the same segment described either way is the same rectangle.
        //
        // ⚠️ **Asserted on `Rect` directly, and deliberately NOT through `check_tree`.** A first
        // version of this test built two trees wound opposite ways and compared verdicts — and it
        // passed with normalisation REMOVED, so it tested nothing. The reason is structural: in
        // these trees a branch pair names each other, so every segment appears TWICE, once wound
        // each way, and the correctly-wound copy always carries the verdict. The property is real
        // but `check_tree` cannot expose it.
        use crate::tree::Rect;
        assert_eq!(Rect::new(10, 5, 0, 0), Rect::new(0, 0, 10, 5));
        assert_eq!(Rect::new(0, 5, 10, 0), Rect::new(0, 0, 10, 5));
        let r = Rect::new(10, 5, 0, 0);
        assert!(r.xlo <= r.xhi && r.ylo <= r.yhi, "corners come out ordered");
    }
}