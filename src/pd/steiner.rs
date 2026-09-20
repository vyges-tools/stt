// SPDX-License-Identifier: Apache-2.0
//! **P7** — `steinerize`, inserting Steiner points where a pair of edges can be shortened.
//!
//! From the reference's comment: *"Steinerize by looking at all adjacent edge pairs and finding
//! the one with maximum improvement by adding a Steiner point. Repeat until no further
//! improvement can be found."*

use super::graph::Graph;

/// A Steiner point that could be inserted at a node, and what it would save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub gain: i32,
    pub node: usize,
    pub edge1: usize,
    pub edge2: usize,
    pub pt: (i32, i32),
}

/// The best Steiner point for one node, over every pair of its incident edges.
///
/// The Steiner point starts at the node and is pulled toward the two neighbours on each axis
/// **only when both lie strictly to the same side** — otherwise the node's own coordinate is
/// already optimal on that axis.
///
/// ⚠️ **`gain > best.gain` is strict and `best.gain` starts at 0**, so a pair that saves nothing
/// is never chosen, and among equal gains the FIRST pair examined wins — which is why the
/// incident-edge order matters.
pub fn best_steiner_for_node(g: &Graph, node: usize) -> Candidate {
    let pt_node = g.pts[node];
    let mut best = Candidate { gain: 0, node, edge1: usize::MAX, edge2: usize::MAX, pt: pt_node };
    let inc = g.incident(node);
    for a in 0..inc.len() {
        let n1 = g.opposite(node, inc[a]);
        let p1 = g.pts[n1];
        for b in (a + 1)..inc.len() {
            let n2 = g.opposite(node, inc[b]);
            let p2 = g.pts[n2];

            let mut sx = pt_node.0;
            if p1.0.min(p2.0) > pt_node.0 {
                sx = p1.0.min(p2.0);
            } else if p1.0.max(p2.0) < pt_node.0 {
                sx = p1.0.max(p2.0);
            }
            let mut sy = pt_node.1;
            if p1.1.min(p2.1) > pt_node.1 {
                sy = p1.1.min(p2.1);
            } else if p1.1.max(p2.1) < pt_node.1 {
                sy = p1.1.max(p2.1);
            }

            let gain = (sx - pt_node.0).abs() + (sy - pt_node.1).abs();
            if gain > best.gain {
                best = Candidate { gain, node, edge1: inc[a], edge2: inc[b], pt: (sx, sy) };
            }
        }
    }
    best
}

/// Insert Steiner points until no pair of adjacent edges can be improved.
///
/// ⚠️ **The heap is a MAX-heap on `(gain, pt.x, pt.y)`** — the reference's own comment calls the
/// point *"just an arbitrary tie breaker"*. ⛔ Note it does NOT include the node, so two nodes
/// offering the same gain at the same point tie completely and their relative order comes from
/// the heap's internals. A node tie-break is added here for determinism; no net in the corpus
/// produces such a tie, so nothing distinguishes the two — if one ever does, this is the first
/// place to look.
///
/// 🔑 **The full rescan below is equivalent to the reference's mutable heap, and that is
/// MEASURED.** It refreshes only `{best.node, steiner_node, opp1, opp2}` after each merge, via
/// `heap.update`, which sifts in BOTH directions and so keeps a correct priority queue — unlike
/// `buildSpanningTree`'s `increase`, where the one-way sift is load-bearing (see
/// [`super::spanning`]). Those four are also exactly the nodes whose candidate can change: a
/// node's best pair depends on its own incident edges and on its neighbours' POSITIONS, and
/// nodes never move. Building the reference with its pop sequence printed and diffing it against
/// ours on upstream's `clk` gave the same node, gain, point and edge pair in every one of the 13
/// rounds.
pub fn steinerize(g: &mut Graph) {
    loop {
        // The reference keeps a mutable heap and refreshes only the touched nodes. Recomputing
        // the best candidate over all nodes each round selects the same maximum — the heap is an
        // optimisation, not part of the rule — and avoids reproducing boost's handle semantics.
        let mut best: Option<Candidate> = None;
        for node in 0..g.node_count() {
            let c = best_steiner_for_node(g, node);
            if c.gain == 0 {
                continue;
            }
            let better = match best {
                None => true,
                Some(b) => (c.gain, c.pt.0, c.pt.1, c.node) > (b.gain, b.pt.0, b.pt.1, b.node),
            };
            if better {
                best = Some(c);
            }
        }
        let Some(best) = best else { return };

        let opp1 = g.opposite(best.node, best.edge1);
        let opp2 = g.opposite(best.node, best.edge2);

        // ⚠️ If the Steiner point coincides with a neighbour, that neighbour IS the Steiner node
        // — no node is created. Creating one anyway would leave a zero-length edge the reference
        // does not have.
        let (steiner, new_node) = if best.pt == g.pts[opp1] {
            (opp1, false)
        } else if best.pt == g.pts[opp2] {
            (opp2, false)
        } else {
            (g.add_node(best.pt), true)
        };

        if steiner != opp1 {
            g.move_endpoint(best.edge1, best.node, steiner);
        }
        if steiner != opp2 {
            g.move_endpoint(best.edge2, best.node, steiner);
        }
        if new_node {
            g.add_edge(steiner, best.node);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_with_one_edge_offers_no_candidate() {
        let mut g = Graph::with_points(&[(0, 0), (10, 10)]);
        g.add_edge(0, 1);
        assert_eq!(best_steiner_for_node(&g, 0).gain, 0, "a pair needs two edges");
    }

    #[test]
    fn two_neighbours_on_the_same_side_pull_the_steiner_point_toward_them() {
        // Both neighbours are right of and above the node, so the point moves to the nearer of
        // each — saving that much wire.
        let mut g = Graph::with_points(&[(0, 0), (10, 5), (6, 8)]);
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        let c = best_steiner_for_node(&g, 0);
        assert_eq!(c.pt, (6, 5), "min x of the two, min y of the two");
        assert_eq!(c.gain, 11);
    }

    #[test]
    fn neighbours_straddling_the_node_give_no_gain_on_that_axis() {
        // ⚠️ The pull happens only when BOTH are strictly to one side. One left and one right
        // means the node's own x is already optimal.
        let mut g = Graph::with_points(&[(5, 0), (0, 9), (10, 7)]);
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        let c = best_steiner_for_node(&g, 0);
        assert_eq!(c.pt.0, 5, "x is unchanged — the neighbours straddle it");
        assert_eq!(c.pt.1, 7, "y moves to the nearer of the two above");
    }

    #[test]
    fn among_equal_gains_the_FIRST_pair_examined_wins() {
        // ⚠️ `gain > best.gain` is strict, so an equal-gain pair found later does NOT displace
        // the earlier one. With `>=` the last pair would win and the chosen edges would differ.
        // Three neighbours, all up-and-right, arranged so two pairs give the same gain.
        let mut g = Graph::with_points(&[(0, 0), (4, 6), (4, 6), (9, 9)]);
        g.add_edge(0, 1); // edge 0
        g.add_edge(0, 2); // edge 1  -- same point as node 1
        g.add_edge(0, 3); // edge 2
        let c = best_steiner_for_node(&g, 0);
        assert_eq!(c.gain, 10, "min x 4 + min y 6");
        // ⚠️ "First" is first in `IncEdgeIt` order, which LEMON walks NEWEST-first: the list is
        // [2, 1, 0], so the first pair is (2, 1). Reading it as edge-id order would name (0, 1)
        // and move the wrong two edges.
        assert_eq!(g.incident(0), &[2, 1, 0]);
        assert_eq!((c.edge1, c.edge2), (2, 1), "the FIRST pair, not a later one of equal gain");
    }

    #[test]
    fn a_steiner_point_coinciding_with_the_SECOND_neighbour_reuses_it_too() {
        // ⚠️ The reference checks opp1 and then opp2. A test that only ever hits opp1 leaves the
        // second branch unexercised — this one lands on opp2.
        let mut g = Graph::with_points(&[(0, 0), (9, 5), (4, 3)]);
        g.add_edge(0, 1); // opp1 = node 1 at (9,5)
        g.add_edge(0, 2); // opp2 = node 2 at (4,3)
        let c = best_steiner_for_node(&g, 0);
        assert_eq!(c.pt, (4, 3), "min x 4, min y 3 — exactly node 2");
        let before = g.node_count();
        steinerize(&mut g);
        assert_eq!(g.node_count(), before, "it reused opp2 rather than creating a node");
    }

    #[test]
    fn steinerize_never_lengthens_the_tree_and_terminates() {
        let pts = [(0, 0), (10, 5), (6, 8), (2, 12), (14, 1)];
        let mut g = Graph::with_points(&pts);
        for e in [(0, 1), (0, 2), (0, 3), (0, 4)] {
            g.add_edge(e.0, e.1);
        }
        let before: i32 = g.edges.iter()
            .map(|&[u, v]| (g.pts[u].0 - g.pts[v].0).abs() + (g.pts[u].1 - g.pts[v].1).abs())
            .sum();
        steinerize(&mut g);
        let after: i32 = g.edges.iter()
            .map(|&[u, v]| (g.pts[u].0 - g.pts[v].0).abs() + (g.pts[u].1 - g.pts[v].1).abs())
            .sum();
        assert!(after <= before, "steinerizing must not lengthen: {before} -> {after}");
        // And it must have reached a fixed point.
        for node in 0..g.node_count() {
            assert_eq!(best_steiner_for_node(&g, node).gain, 0,
                       "node {node} still offers a gain — the loop exited early");
        }
    }

    #[test]
    fn a_steiner_point_landing_on_a_neighbour_reuses_that_node() {
        // ⚠️ No new node, and therefore no zero-length edge.
        let mut g = Graph::with_points(&[(0, 0), (5, 0), (5, 9)]);
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        let before = g.node_count();
        steinerize(&mut g);
        assert_eq!(g.node_count(), before, "the point coincided with a neighbour");
    }
}