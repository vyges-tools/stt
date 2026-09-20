// SPDX-License-Identifier: Apache-2.0
//! **P8** and **P9** — enforce max degree three, then convert the graph to a FLUTE-style tree.

use super::graph::Graph;
use crate::tree::{Branch, Tree};

/// **P8** — split any node of degree four or more.
///
/// From the reference's comment: *"Flute only returns nodes with maximum degree of three. grt
/// assumes this property so we enforce it here. Any degree four node is split into two nodes with
/// a zero length edge between them. A very high degree node could be split more than once."*
///
/// ⚠️ **A queue, not a single pass**, and the new node is pushed BACK onto it — a node of degree
/// eight needs splitting repeatedly, and each split leaves the new node holding the surplus.
///
/// ⚠️ **The first TWO incident edges stay**; everything from the third onward moves. The counter
/// is pre-incremented (`if (++edge_cnt >= 3)`), so the test fires on the third edge, and the new
/// zero-length edge added afterwards brings the original back to exactly three.
///
/// ⛔ **The queue is seeded in DESCENDING node id.** The reference seeds it with `ListGraph::
/// NodeIt`, and LEMON's `addNode` prepends to the node list (`nodes[n].next = first_node;
/// first_node = n;`), so `NodeIt` walks the HIGHEST id first. That decides which node is split
/// first and therefore which new node gets which id — on a nine-pin star the reference numbers
/// the splits 11, 12, 13 for nodes 10, 9, 0, and ascending order numbers them 11, 12, 13 for
/// nodes 0, 9, 10 instead. Same tree, different branch indices, and `printTree` compares indices.
pub fn split_degree4_nodes(g: &mut Graph) {
    let mut queue: std::collections::VecDeque<usize> = (0..g.node_count()).rev().collect();
    while let Some(node) = queue.pop_front() {
        // ⚠️ Snapshotted BEFORE the mutations below, exactly as the reference advances its
        // `IncEdgeIt` past the edge it is about to move — walking a list you are prepending to
        // would revisit the edges you just relocated.
        let inc: Vec<usize> = g.incident(node).to_vec();
        if inc.len() <= 3 {
            continue;
        }
        let new_node = g.add_node(g.pts[node]);
        for (i, &edge) in inc.iter().enumerate() {
            // `++edge_cnt >= 3` on a 1-based counter is the 3rd edge onward, i.e. index >= 2.
            if i >= 2 {
                g.move_endpoint(edge, node, new_node);
            }
        }
        g.add_edge(node, new_node);
        queue.push_back(new_node);
    }
}

/// **P9** — walk the graph from the driver into a FLUTE-style `Tree`.
///
/// ⚠️ **The driver is its own parent**, which is what the reference means by *"Flute-style needs a
/// self edge on the driver node"*. That self-reference is how a tree walk terminates, and it is
/// the same convention the two- and three-pin FLUTE trees use.
///
/// ⚠️ **`branch` is sized by `maxNodeId() + 1`, not by degree** — Steiner nodes added during
/// steinerizing and splitting are numbered past the terminals, and the array must span all of
/// them. Sizing it `2*deg - 2` would be right for FLUTE and wrong here.
///
/// ⚠️ **Length accumulates during the walk**, including the driver's own zero-length self edge,
/// so it is the sum over edges exactly once — each node contributes its edge to its parent.
pub fn make_tree(g: &Graph, num_terminals: usize, driver: usize) -> Tree {
    let mut tree = Tree {
        deg: num_terminals,
        length: 0,
        branch: vec![Branch { x: 0, y: 0, n: 0 }; g.node_count()],
    };
    // An explicit stack rather than recursion: the reference recurses, and a 250-pin net with a
    // deep chain would risk the Rust stack where C++'s happens to survive.
    let mut stack = vec![(driver, driver)];
    let mut seen = vec![false; g.node_count()];
    while let Some((node, parent)) = stack.pop() {
        if seen[node] {
            continue;
        }
        seen[node] = true;
        let (x, y) = g.pts[node];
        let (px, py) = g.pts[parent];
        tree.branch[node] = Branch { x, y, n: parent };
        tree.length += i64::from((x - px).abs()) + i64::from((y - py).abs());
        // ⚠️ Pushed in REVERSE so they pop in incident-edge order, matching the reference's
        // depth-first recursion over `IncEdgeIt`.
        let inc = g.incident(node);
        for &edge in inc.iter().rev() {
            let child = g.opposite(node, edge);
            if child != parent && !seen[child] {
                stack.push((child, node));
            }
        }
    }
    tree
}

#[cfg(test)]
mod tests {
    use super::*;

    fn star(n: usize) -> Graph {
        let mut g = Graph::with_points(&[(0, 0)]);
        for i in 1..=n {
            g.add_node((i as i32 * 10, i as i32 * 3));
            g.add_edge(0, i);
        }
        g
    }

    #[test]
    fn a_node_of_degree_three_is_left_alone() {
        let mut g = star(3);
        let before = g.node_count();
        split_degree4_nodes(&mut g);
        assert_eq!(g.node_count(), before, "degree 3 is already legal");
    }

    #[test]
    fn a_degree_four_node_is_split_once_and_both_halves_end_at_three() {
        let mut g = star(4);
        split_degree4_nodes(&mut g);
        assert_eq!(g.node_count(), 6, "one new node");
        for node in 0..g.node_count() {
            assert!(g.incident(node).len() <= 3, "node {node} still exceeds degree 3");
        }
    }

    #[test]
    fn a_very_high_degree_node_is_split_REPEATEDLY() {
        // ⚠️ The new node is pushed BACK onto the queue, so a degree-8 node splits more than
        // once. A single pass would leave the surplus node over degree three.
        let mut g = star(8);
        split_degree4_nodes(&mut g);
        for node in 0..g.node_count() {
            assert!(g.incident(node).len() <= 3,
                    "node {node} has degree {} — the queue did not re-process it",
                    g.incident(node).len());
        }
    }

    #[test]
    fn the_queue_is_seeded_HIGHEST_NODE_FIRST_which_decides_the_new_nodes_ids() {
        // ⛔ LEMON's `addNode` prepends, so `NodeIt` walks the highest id first. Two nodes that
        // both need splitting therefore get their new ids in DESCENDING order of the node split,
        // and `printTree` compares branch indices — so this is observable, not cosmetic.
        //
        // 🔑 Measured on a nine-pin star at alpha 0.4 against the unmodified reference: it
        // numbers the splits of nodes 10, 9 and 0 as 11, 12, 13. Ascending order numbers the
        // splits of 0, 9 and 10 as 11, 12, 13 instead — the same tree, four branches renumbered.
        let mut g = Graph::with_points(&[(0, 0), (100, 0)]);
        for (node, x) in [(0usize, 0), (1usize, 100)] {
            for k in 0..4 {
                let n = g.add_node((x + 10 * (k + 1), 10 * (k + 1)));
                g.add_edge(node, n);
            }
        }
        let before = g.node_count();
        split_degree4_nodes(&mut g);
        assert_eq!(g.node_count(), before + 2, "both hubs split once");
        // Node 1 is the higher id, so it is processed first and takes the LOWER new id.
        assert_eq!(g.pts[before], g.pts[1], "the first new node is node 1's split");
        assert_eq!(g.pts[before + 1], g.pts[0], "the second is node 0's");
    }

    #[test]
    fn the_split_node_sits_at_the_same_point_so_the_new_edge_is_zero_length() {
        let mut g = star(5);
        let origin = g.pts[0];
        split_degree4_nodes(&mut g);
        let extra: Vec<usize> = (1..g.node_count()).filter(|&i| g.pts[i] == origin).collect();
        assert!(!extra.is_empty(), "the split node shares the original's coordinates");
    }

    #[test]
    fn make_tree_roots_at_the_driver_and_measures_every_edge_once() {
        // A simple path 0-1-2 with the driver at 0.
        let mut g = Graph::with_points(&[(0, 0), (10, 0), (10, 5)]);
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let t = make_tree(&g, 3, 0);
        assert_eq!(t.branch[0].n, 0, "the driver is its own parent");
        assert_eq!(t.branch[1].n, 0);
        assert_eq!(t.branch[2].n, 1);
        assert_eq!(t.length, 15, "10 + 5, each edge counted once");
        assert_eq!(t.deg, 3);
    }

    #[test]
    fn make_tree_spans_every_node_including_steiner_ones() {
        // ⚠️ The branch array is sized by NODE COUNT, not by 2*deg-2 — Steiner nodes added by
        // steinerizing and splitting are numbered past the terminals.
        // ⚠️ THREE terminals and TWO Steiner nodes: `2*deg - 2` is 4 and would silently fit a
        // one-Steiner graph, so the control on this rule needs a graph it cannot fit.
        let mut g = Graph::with_points(&[(0, 0), (10, 0), (0, 10)]);
        let s0 = g.add_node((0, 0));
        let s1 = g.add_node((0, 5));
        g.add_edge(0, s0);
        g.add_edge(s0, 1);
        g.add_edge(s0, s1);
        g.add_edge(s1, 2);
        let t = make_tree(&g, 3, 0);
        assert_eq!(t.branch.len(), 5, "three terminals plus TWO Steiner nodes");
        assert_eq!(t.deg, 3, "but the DEGREE is the terminal count");
        let roots = (0..t.branch.len()).filter(|&i| t.branch[i].n == i).count();
        assert_eq!(roots, 1);
    }
}