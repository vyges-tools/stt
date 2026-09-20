// SPDX-License-Identifier: Apache-2.0
//! **P4** — the nearest-neighbour relation Prim-Dijkstra searches over.
//!
//! From the reference's own comment, quoting *"Prim-Dijkstra Revisited"* §4:
//!
//! > *"We say that vi is a neighbor of vj if the smallest bounding box containing vi and vj
//! > contains no other nodes."*
//!
//! ⚠️ And its correction of the original code's comments: *"This has nothing to do with the
//! Guibas & Stolfi method despite its mention in the paper... GS is not a good choice as it
//! excludes edges that may be optimal for high alpha values."* So this is deliberately NOT a
//! Delaunay-style neighbourhood, and substituting one would change which trees are reachable.

/// Each point's neighbours, in the order the sweep discovers them.
///
/// ⛔ **Order is part of the output, not an artefact.** The lists are consumed in sequence by the
/// spanning-tree search, so sorting them — or building them with a different sweep direction —
/// changes which edge wins a tie.
pub fn nearest_neighbors(pts: &[(i32, i32)]) -> Vec<Vec<usize>> {
    let n = pts.len();
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];

    // Per-node bounds on the closest x seen so far in each quadrant. A node beyond one of these
    // would have another node inside its bounding box, so it is not a nearest neighbour.
    //
    // ⚠️ **The two pairs start at OPPOSITE extremes**: the right-hand quadrants tighten downward
    // from `i32::MAX`, the left-hand ones upward from `i32::MIN`. Initialising all four the same
    // way silently admits or rejects every first candidate.
    let mut ur = vec![i32::MAX; n];
    let mut lr = vec![i32::MAX; n];
    let mut ul = vec![i32::MIN; n];
    let mut ll = vec![i32::MIN; n];

    // ⚠️ Sorted by **(y, x) as a pair**, and STABLE — the sweep depends on processing in order of
    // increasing y, and the x component decides ties rather than leaving them to input order.
    let mut sorted: Vec<usize> = (0..n).collect();
    sorted.sort_by_key(|&i| (pts[i].1, pts[i].0));

    for idx in 0..n {
        let pt_idx = sorted[idx];
        let pt_x = pts[pt_idx].0;

        // Update the UPPER quadrants of every point already swept (i.e. at or below this y).
        // ⚠️ FORWARD through the swept prefix.
        for i in 0..idx {
            let below_idx = sorted[i];
            let below_x = pts[below_idx].0;
            if below_x <= pt_x && pt_x < ur[below_idx] {
                // ⚠️ `<=` on the left, `<` on the right: a point directly above counts as
                // upper-RIGHT, not upper-left, and the bound is exclusive.
                neighbors[below_idx].push(pt_idx);
                ur[below_idx] = pt_x;
            } else if ul[below_idx] < pt_x && pt_x < below_x {
                neighbors[below_idx].push(pt_idx);
                ul[below_idx] = pt_x;
            }
        }

        // Set this point's LOWER quadrants from the same prefix.
        // ⚠️ BACKWARD — nearest-in-y first, so the bound tightens from the closest outward. The
        // two loops walk the same range in opposite directions and that is not interchangeable.
        for i in (0..idx).rev() {
            let below_idx = sorted[i];
            let below_x = pts[below_idx].0;
            if pt_x <= below_x && below_x < lr[pt_idx] {
                neighbors[pt_idx].push(below_idx);
                lr[pt_idx] = below_x;
            } else if ll[pt_idx] < below_x && below_x < pt_x {
                neighbors[pt_idx].push(below_idx);
                ll[pt_idx] = below_x;
            }
        }
    }
    neighbors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_point_has_no_neighbours() {
        assert_eq!(nearest_neighbors(&[(0, 0)]), vec![Vec::<usize>::new()]);
    }

    #[test]
    fn two_points_are_neighbours_of_each_other() {
        // The lower point gains the upper one; the upper gains the lower.
        let nn = nearest_neighbors(&[(0, 0), (5, 5)]);
        assert_eq!(nn[0], vec![1]);
        assert_eq!(nn[1], vec![0]);
    }

    #[test]
    fn a_point_inside_the_bounding_box_blocks_the_pair() {
        // 🔑 The defining property: (0,0) and (10,10) are NOT neighbours once (5,5) sits in the
        // box between them — the sweep tightens `ur[0]` to 5 before it ever sees x = 10.
        let nn = nearest_neighbors(&[(0, 0), (10, 10), (5, 5)]);
        assert!(!nn[0].contains(&1), "(0,0) and (10,10) are separated by (5,5)");
        assert!(nn[0].contains(&2), "but (5,5) is a neighbour");
    }

    #[test]
    fn the_relation_is_symmetric_in_what_it_records() {
        // Every pair recorded upward from the lower point is also recorded downward from the
        // upper one, so each edge appears in both lists.
        let pts = [(0, 0), (7, 3), (2, 8), (9, 9), (4, 5)];
        let nn = nearest_neighbors(&pts);
        for (i, list) in nn.iter().enumerate() {
            for &j in list {
                assert!(nn[j].contains(&i), "edge {i}-{j} recorded one way only");
            }
        }
    }

    #[test]
    fn a_point_directly_above_counts_as_upper_RIGHT() {
        // ⚠️ `below_x <= pt_x` is inclusive on the left, so equal x goes to the right quadrant.
        // Flipping that boundary moves the edge into `ul` and changes which bound tightens.
        let nn = nearest_neighbors(&[(5, 0), (5, 10)]);
        assert_eq!(nn[0], vec![1], "the point directly above is recorded once");
        assert_eq!(nn[1], vec![0]);
    }

    #[test]
    fn ties_in_y_are_broken_by_x_and_the_LISTS_prove_it() {
        // ⚠️ The sort key is the PAIR (y, x). Rust's sort is stable, so a y-only key would leave
        // equal-y points in INPUT order and sweep them differently.
        //
        // A first version of this test asserted only membership and passed under BOTH keys — it
        // discriminated nothing. The neighbour LISTS, order included, are what differ, so they
        // are what is pinned.
        let pts = [(9, 4), (1, 4), (5, 4), (3, 9), (7, 1)];
        assert_eq!(
            nearest_neighbors(&pts),
            vec![vec![2, 4, 3], vec![4, 2, 3], vec![1, 4, 0, 3], vec![0, 2, 1], vec![1, 2, 0]],
        );
    }

    #[test]
    fn a_grid_gives_every_point_at_least_one_neighbour() {
        let mut pts = Vec::new();
        for i in 0..4 {
            for j in 0..4 {
                pts.push((i * 10, j * 10));
            }
        }
        let nn = nearest_neighbors(&pts);
        assert!(nn.iter().all(|l| !l.is_empty()), "no point is isolated");
    }
}