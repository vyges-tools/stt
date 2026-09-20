// SPDX-License-Identifier: Apache-2.0
//! **P10** — `primDijkstra`, the entry point that runs the stages in the reference's order.
//!
//! ```text
//!   pts        <- zip(x, y)                    one node per terminal, id == index
//!   nn         <- get_nearest_neighbors(pts)
//!   driver     <- nodeFromId(driver_index)
//!   buildSpanningTree(node_point, driver, alpha, nn, graph)
//!   steinerize(graph, node_point)
//!   splitDegree4Nodes(graph, node_point)
//!   return makeTree(graph, num_terminals, driver, node_point)
//! ```
//!
//! 🔑 **The order is behaviour, not presentation.** `steinerize` can raise a node above degree
//! three, which is exactly why the split runs after it and not before; `makeTree` sizes its
//! branch array from the node count both of those stages grew.

use super::{
    build_spanning_tree, make_tree, nearest_neighbors, split_degree4_nodes, steinerize, Graph,
};
use crate::tree::Tree;

/// Errors the reference raises through `Logger::error`, which aborts the command.
#[derive(Debug, PartialEq, Eq)]
pub enum PdError {
    /// `STT-0008` — x size != y size.
    MismatchedLengths(usize, usize),
    /// `STT-0009` — invalid request for an empty Steiner tree.
    Empty,
}

impl std::fmt::Display for PdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdError::MismatchedLengths(nx, ny) => write!(f, "x size ({nx}) != y size ({ny})"),
            PdError::Empty => write!(f, "Invalid request for an empty Steiner tree."),
        }
    }
}

/// `pdr::primDijkstra`.
///
/// ⚠️ **`driver_index` is a node ID, not a search key.** The reference does
/// `ListGraph::nodeFromId(driver_index)` on a graph whose nodes were added in terminal order, so
/// it indexes `pts` directly. Looking the driver up by coordinate would pick a different pin
/// whenever two terminals share a location.
pub fn prim_dijkstra(
    x: &[i32],
    y: &[i32],
    driver_index: usize,
    alpha: f32,
) -> Result<Tree, PdError> {
    if x.len() != y.len() {
        return Err(PdError::MismatchedLengths(x.len(), y.len()));
    }
    if x.is_empty() {
        return Err(PdError::Empty);
    }

    let pts: Vec<(i32, i32)> = x.iter().copied().zip(y.iter().copied()).collect();
    let num_terminals = pts.len();

    let nn = nearest_neighbors(&pts);

    let mut g = Graph::with_points(&pts);
    for (u, v) in build_spanning_tree(&pts, driver_index, alpha, &nn) {
        g.add_edge(u, v);
    }

    steinerize(&mut g);
    split_degree4_nodes(&mut g);

    Ok(make_tree(&g, num_terminals, driver_index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::check_tree;

    #[test]
    fn a_single_pin_is_a_self_rooted_tree_of_zero_length() {
        let t = prim_dijkstra(&[7], &[9], 0, 0.3).unwrap();
        assert_eq!(t.deg, 1);
        assert_eq!(t.length, 0);
        assert_eq!(t.branch.len(), 1);
        assert_eq!(t.branch[0].n, 0, "the driver is its own parent");
    }

    #[test]
    fn mismatched_and_empty_inputs_are_the_references_two_errors() {
        assert_eq!(
            prim_dijkstra(&[0, 1], &[0], 0, 0.3).unwrap_err(),
            PdError::MismatchedLengths(2, 1)
        );
        assert_eq!(prim_dijkstra(&[], &[], 0, 0.3).unwrap_err(), PdError::Empty);
    }

    #[test]
    fn two_pins_cost_the_manhattan_distance() {
        let t = prim_dijkstra(&[0, 30], &[0, 40], 0, 0.3).unwrap();
        assert_eq!(t.length, 70);
        assert!(check_tree(&t));
    }

    #[test]
    fn a_square_is_built_and_passes_the_tree_check() {
        let t = prim_dijkstra(&[0, 100, 0, 100], &[0, 0, 100, 100], 0, 0.0).unwrap();
        assert!(check_tree(&t), "{t:?}");
        // alpha = 0 is a pure MST: three edges of 100 over a unit square's corners.
        assert_eq!(t.length, 300);
        assert_eq!(t.deg, 4);
    }

    #[test]
    fn NO_node_exceeds_degree_three_after_the_full_pipeline() {
        // ⚠️ grt depends on this — the reference's own comment says so. A star net is the shape
        // that violates it if `splitDegree4Nodes` is skipped or runs before `steinerize`.
        let xs = [0, 50, -50, 0, 0, 35, -35, 35, -35];
        let ys = [0, 0, 0, 50, -50, 35, 35, -35, -35];
        let t = prim_dijkstra(&xs, &ys, 0, 0.4).unwrap();
        let mut child_count = vec![0usize; t.branch.len()];
        for (i, b) in t.branch.iter().enumerate() {
            if b.n != i {
                child_count[b.n] += 1;
            }
        }
        for (i, &c) in child_count.iter().enumerate() {
            let deg = c + usize::from(t.branch[i].n != i);
            assert!(deg <= 3, "node {i} has degree {deg} — the split did not run to fixpoint");
        }
    }

    #[test]
    fn raising_alpha_never_lengthens_the_path_to_the_farthest_pin() {
        // The PD trade-off: alpha buys depth with wirelength. Total length is non-decreasing in
        // alpha, which is the property the paper claims and the cheapest end-to-end check that
        // the weight really blends path length in.
        let xs = [0, 100, 200, 300, 400, 500];
        let ys = [0, 10, 20, 30, 40, 50];
        let l0 = prim_dijkstra(&xs, &ys, 0, 0.0).unwrap().length;
        let l1 = prim_dijkstra(&xs, &ys, 0, 1.0).unwrap().length;
        assert!(l1 >= l0, "alpha=1 gave {l1} < alpha=0 {l0}");
    }
}