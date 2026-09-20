// SPDX-License-Identifier: Apache-2.0
//! `flutes_low_degree` — the whole low-degree path, assembled from its parts.
//!
//! For `2 <= d <= kMaxLutDegree` the answer comes straight out of the lookup table. The stages
//! are the reference's, in the reference's order.

use super::index::{gaps, group_index};
use super::lut::Lut;
use super::solve::{best_solution, build_tree};
use crate::tree::{Branch, Tree};

/// Build the tree for a prepared, low-degree net.
///
/// ⚠️ `xs` and `ys` are already SORTED by [`super::entry::prepare`], which is why the lengths
/// below are plain differences with no `abs()` — `xs[d-1] >= xs[0]` by construction.
pub fn flutes_low_degree(lut: &Lut, d: usize, xs: &[i32], ys: &[i32], s: &[usize]) -> Tree {
    if d == 2 {
        return Tree {
            deg: 2,
            length: i64::from(xs[1] - xs[0]) + i64::from(ys[1] - ys[0]),
            branch: vec![
                Branch { x: xs[s[0]], y: ys[0], n: 1 },
                Branch { x: xs[s[1]], y: ys[1], n: 1 },
            ],
        };
    }
    if d == 3 {
        // ⚠️ FOUR branches for three terminals: the fourth is a Steiner point at
        // `(xs[1], ys[1])`, and **every branch including that one points at index 3** — the
        // Steiner point self-references, which is how the walk terminates.
        return Tree {
            deg: 3,
            length: i64::from(xs[2] - xs[0]) + i64::from(ys[2] - ys[0]),
            branch: vec![
                Branch { x: xs[s[0]], y: ys[0], n: 3 },
                Branch { x: xs[s[1]], y: ys[1], n: 3 },
                Branch { x: xs[s[2]], y: ys[2], n: 3 },
                Branch { x: xs[1], y: ys[1], n: 3 },
            ],
        };
    }

    let gi = group_index(d, s);
    let dd = gaps(d, xs, ys, gi.hflip);
    let group = lut.lut[d][gi.k]
        .as_ref()
        .expect("every group of every degree is populated; the parser asserts it");
    let bbox = xs[d - 1] - xs[0] + ys[d - 1] - ys[0];
    let (best, length) = best_solution(group, &dd, bbox);
    build_tree(d, xs, ys, s, &group[best], gi, length)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::check_tree;
    use crate::flute::entry::prepare;
    use crate::flute::lut::{load_tables, MAX_LUT_DEGREE};

    fn lut() -> Lut {
        load_tables(MAX_LUT_DEGREE).expect("the shipped tables parse")
    }

    #[test]
    fn a_three_pin_tree_has_a_self_referencing_steiner_point() {
        let l = lut();
        let p = prepare(&[0, 10, 5], &[0, 4, 9]);
        let t = flutes_low_degree(&l, 3, &p.xs, &p.ys, &p.s);
        assert_eq!(t.branch.len(), 4, "3 terminals + 1 Steiner point");
        assert!(t.branch.iter().all(|b| b.n == 3), "all four point at index 3");
        assert_eq!(t.length, 19, "(10-0) + (9-0)");
    }

    #[test]
    fn every_degree_from_four_to_nine_builds_a_tree_of_the_right_shape() {
        // ⭐ The table, the index, the flip, the incremental evaluation and the assembly, all
        // exercised together against the real 9.4 MB of data.
        let l = lut();
        for d in 4..=MAX_LUT_DEGREE {
            // A deterministic spread of points that is not degenerate in either axis.
            let x: Vec<i32> = (0..d).map(|i| (i as i32 * 37) % 101).collect();
            let y: Vec<i32> = (0..d).map(|i| (i as i32 * 53) % 97).collect();
            let p = prepare(&x, &y);
            let t = flutes_low_degree(&l, d, &p.xs, &p.ys, &p.s);
            assert_eq!(t.deg, d);
            assert_eq!(t.branch.len(), 2 * d - 2, "degree {d}: 2d-2 branches");
            assert!(t.branch.iter().all(|b| b.n < t.branch.len()),
                    "degree {d}: every neighbour index is in range");
        }
    }

    #[test]
    fn a_built_tree_is_at_least_the_bounding_box_half_perimeter() {
        // A Steiner tree cannot be shorter than the bounding box's half-perimeter. This is a
        // property of the ANSWER rather than of the table, so it catches an index or an
        // evaluation that silently returns the wrong group.
        let l = lut();
        for d in 4..=MAX_LUT_DEGREE {
            let x: Vec<i32> = (0..d).map(|i| (i as i32 * 29) % 71).collect();
            let y: Vec<i32> = (0..d).map(|i| (i as i32 * 41) % 83).collect();
            let p = prepare(&x, &y);
            let t = flutes_low_degree(&l, d, &p.xs, &p.ys, &p.s);
            let bbox = i64::from(p.xs[d - 1] - p.xs[0]) + i64::from(p.ys[d - 1] - p.ys[0]);
            assert!(t.length >= bbox,
                    "degree {d}: length {} is below the bounding box {bbox}", t.length);
        }
    }

    #[test]
    fn built_trees_pass_the_engines_own_self_overlap_check() {
        // 🔑 Ties the two halves of this engine together: FLUTE's output must satisfy the
        // predicate that gates Prim-Dijkstra's. A tree that failed here would mean one of the
        // two is wrong, and the reference never returns such a tree from the table.
        let l = lut();
        for d in 4..=MAX_LUT_DEGREE {
            for seed in 0..8i32 {
                let x: Vec<i32> = (0..d).map(|i| (i as i32 * (7 + seed) + seed) % 64).collect();
                let y: Vec<i32> = (0..d).map(|i| (i as i32 * (11 + seed) + 3) % 58).collect();
                let p = prepare(&x, &y);
                let t = flutes_low_degree(&l, d, &p.xs, &p.ys, &p.s);
                assert!(check_tree(&t), "degree {d} seed {seed}: FLUTE tree failed check_tree");
            }
        }
    }
}