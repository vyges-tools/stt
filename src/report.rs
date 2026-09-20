// SPDX-License-Identifier: Apache-2.0
//! `reportSteinerTree` — the reference's own printed form of a tree.
//!
//! 🔑 **This is the correlation surface.** Upstream's `stt` regression compares exactly these
//! lines, shipped as `.ok` files, so reproducing the printer byte for byte is what makes those
//! goldens usable as our gate without a re-derivation in between.

use crate::tree::Tree;

/// `findLocationIndex` — the branch index at the driver's COORDINATES.
///
/// ⛔ **By location, not by the index the caller passed in.** The reference's own comment:
/// *"flute mangles the x/y locations and pdrevII moves the driver to 0 so we have to find the
/// driver location index."* The FIRST branch at that point wins, which is why a net with
/// duplicate pins can report a depth measured from a different branch than the caller meant.
pub fn find_location_index(tree: &Tree, x: i32, y: i32) -> Option<usize> {
    (0..tree.branch_count()).find(|&i| tree.branch[i].x == x && tree.branch[i].y == y)
}

/// `findPathDepth` — the longest root-to-leaf distance from the driver branch.
///
/// ⚠️ **The adjacency is UNDIRECTED and built from `branch.n`, skipping self edges.** Walking
/// `branch.n` upward instead would give the depth of one path, not the maximum over all of them.
///
/// ⚠️ **A tree of one branch is depth 0** without building any adjacency — the reference guards
/// `branch_count > 1` before the walk.
pub fn find_path_depth(tree: &Tree, drvr_index: usize) -> i64 {
    let n = tree.branch_count();
    if n <= 1 {
        return 0;
    }
    let mut adj: Vec<Vec<(usize, i64)>> = vec![Vec::new(); n];
    for i in 0..n {
        let neighbor = tree.branch[i].n;
        if neighbor != i {
            let len = tree.branch_length(i);
            adj[neighbor].push((i, len));
            adj[i].push((neighbor, len));
        }
    }
    // Iterative, for the same reason `make_tree` is: the reference recurses and a long chain
    // would reach the Rust stack first.
    let mut max_length = 0i64;
    let mut stack = vec![(drvr_index, drvr_index, 0i64)];
    while let Some((node, from, length)) = stack.pop() {
        max_length = max_length.max(length);
        for &(neighbor, edge_length) in &adj[node] {
            if neighbor != from {
                stack.push((neighbor, node, length + edge_length));
            }
        }
    }
    max_length
}

/// The full `reportSteinerTree` output: the summary line then one line per branch.
///
/// Returns `None` when no branch sits at the driver's location — the reference raises `STT-0007`
/// there and prints nothing.
pub fn report_steiner_tree(tree: &Tree, drvr_x: i32, drvr_y: i32) -> Option<Vec<String>> {
    let drvr_index = find_location_index(tree, drvr_x, drvr_y)?;
    let mut out = vec![format!(
        "Wire length = {} Path depth = {}",
        tree.length,
        find_path_depth(tree, drvr_index)
    )];
    out.extend(tree.print_lines());
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Branch;

    /// `pd1.ok`'s `dup1`, which upstream reports as `Wire length = 30 Path depth = 30`.
    fn dup1() -> Tree {
        Tree {
            deg: 4,
            length: 30,
            branch: vec![
                Branch { x: 0, y: 0, n: 0 },
                Branch { x: 10, y: 10, n: 0 },
                Branch { x: 10, y: 20, n: 3 },
                Branch { x: 10, y: 10, n: 1 },
            ],
        }
    }

    #[test]
    fn dup1_reproduces_upstreams_golden_line_for_line() {
        let got = report_steiner_tree(&dup1(), 0, 0).unwrap();
        assert_eq!(
            got,
            vec![
                "Wire length = 30 Path depth = 30",
                "0 (0 0) neighbor 0 length 0",
                "1 (10 10) neighbor 0 length 20",
                "2 (10 20) neighbor 3 length 10",
                "3 (10 10) neighbor 1 length 0",
            ]
        );
    }

    #[test]
    fn the_depth_is_the_LONGEST_path_not_the_driver_chain() {
        // ⛔ Branch 0 is the root and its own `n`, so walking `branch.n` from the driver gives 0.
        // The real answer, 30, is the walk DOWN to the deepest leaf.
        assert_eq!(find_path_depth(&dup1(), 0), 30);
    }

    #[test]
    fn a_single_branch_tree_is_depth_zero_with_no_adjacency_built() {
        let t = Tree { deg: 1, length: 0, branch: vec![Branch { x: 10, y: 10, n: 0 }] };
        assert_eq!(find_path_depth(&t, 0), 0);
        assert_eq!(
            report_steiner_tree(&t, 10, 10).unwrap(),
            vec!["Wire length = 0 Path depth = 0", "0 (10 10) neighbor 0 length 0"]
        );
    }

    #[test]
    fn the_driver_is_found_by_LOCATION_and_the_FIRST_match_wins() {
        // ⚠️ `dup1` has two branches at (10 10) — 1 and 3. The reference takes the lower index.
        assert_eq!(find_location_index(&dup1(), 10, 10), Some(1));
        assert_eq!(find_location_index(&dup1(), 99, 99), None);
    }

    #[test]
    fn a_driver_location_no_branch_sits_at_is_the_references_STT_0007() {
        assert!(report_steiner_tree(&dup1(), 7, 7).is_none());
    }
}