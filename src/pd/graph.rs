// SPDX-License-Identifier: Apache-2.0
//! A small mutable graph with the operations `pd.cpp` performs on LEMON's `ListGraph`.
//!
//! Only what the algorithm uses: add a node, add an edge, move one endpoint of an existing edge
//! (`changeU`/`changeV`), and walk a node's incident edges.
//!
//! ⚠️ **Node ids are insertion order**, because `primDijkstra` does
//! `ListGraph::nodeFromId(driver_index)` — the driver is identified by POSITION, so terminal `i`
//! must be node `i` or the driver silently becomes a different pin.

/// A graph whose incidence lists reproduce `ListGraph`'s, because `IncEdgeIt` order is behaviour.
///
/// ⛔ **LEMON keeps a per-node singly-walked list and PREPENDS to it**, so `IncEdgeIt` yields a
/// node's edges in REVERSE order of attachment — and "attachment", not "creation", is the word
/// that matters. `list_graph.h`:
///
/// ```text
/// addEdge(u, v):   arcs[n].next_out = nodes[v.id].first_out;  nodes[v.id].first_out = n;
///                  arcs[n|1].next_out = nodes[u.id].first_out; nodes[u.id].first_out = n|1;
/// changeU(e, n):   ... unlink from the old node ...
///                  arcs[(2e)|1].next_out = nodes[n.id].first_out;
///                  nodes[n.id].first_out = (2e)|1;
/// ```
///
/// 🔑 So a MOVED edge jumps to the FRONT of its new node's list. Scanning edge ids in reverse
/// reproduces this only until the first `changeU`; after that the two diverge, and the order
/// decides which edge pair `best_steiner_for_node` picks and which edges `splitDegree4Nodes`
/// relocates. Reverse-id order scored 144/145 on `pd_gcd` where insertion order scored 139 —
/// both are approximations of this list, and only the list is the rule.
#[derive(Debug, Clone)]
pub struct Graph {
    pub pts: Vec<(i32, i32)>,
    /// `[u, v]` per edge. Endpoints are mutated in place, never removed.
    pub edges: Vec<[usize; 2]>,
    /// `adj[node]` in `IncEdgeIt` order — front first, i.e. most recently attached first.
    adj: Vec<Vec<usize>>,
}

impl Graph {
    pub fn with_points(pts: &[(i32, i32)]) -> Self {
        Graph { pts: pts.to_vec(), edges: Vec::new(), adj: vec![Vec::new(); pts.len()] }
    }

    pub fn add_node(&mut self, p: (i32, i32)) -> usize {
        self.pts.push(p);
        self.adj.push(Vec::new());
        self.pts.len() - 1
    }

    pub fn add_edge(&mut self, u: usize, v: usize) -> usize {
        let e = self.edges.len();
        self.edges.push([u, v]);
        self.adj[u].insert(0, e);
        self.adj[v].insert(0, e);
        e
    }

    pub fn node_count(&self) -> usize {
        self.pts.len()
    }

    /// The edges incident on `node`, in `IncEdgeIt` order.
    pub fn incident(&self, node: usize) -> &[usize] {
        &self.adj[node]
    }

    /// The other endpoint of `edge` from `node`.
    pub fn opposite(&self, node: usize, edge: usize) -> usize {
        let [u, v] = self.edges[edge];
        if u == node { v } else { u }
    }

    /// Move whichever endpoint of `edge` currently equals `from` to `to`.
    ///
    /// The reference distinguishes `changeU` and `changeV` by testing `graph.u(e) == node`; the
    /// effect is the same and expressing it once avoids getting the pair the wrong way round.
    ///
    /// ⚠️ **The edge is PREPENDED to `to`'s list**, not appended — see the type's note.
    pub fn move_endpoint(&mut self, edge: usize, from: usize, to: usize) {
        if self.edges[edge][0] == from {
            self.edges[edge][0] = to;
        } else {
            self.edges[edge][1] = to;
        }
        self.adj[from].retain(|&e| e != edge);
        self.adj[to].insert(0, edge);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incident_order_is_REVERSE_attachment_not_edge_id() {
        let mut g = Graph::with_points(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
        g.add_edge(0, 1); // edge 0
        g.add_edge(0, 2); // edge 1
        g.add_edge(0, 3); // edge 2
        assert_eq!(g.incident(0), &[2, 1, 0], "LEMON prepends, so the newest is first");
    }

    #[test]
    fn a_MOVED_edge_jumps_to_the_front_of_its_new_nodes_list() {
        // ⛔ This is where reverse-edge-id order and the real list part company: edge 0 is the
        // OLDEST, and after the move it is nevertheless FIRST on node 3.
        let mut g = Graph::with_points(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
        g.add_edge(0, 1); // edge 0
        g.add_edge(3, 2); // edge 1
        g.add_edge(3, 1); // edge 2
        assert_eq!(g.incident(3), &[2, 1]);
        g.move_endpoint(0, 0, 3);
        assert_eq!(g.incident(3), &[0, 2, 1], "the moved edge is prepended, not sorted by id");
        assert!(g.incident(0).is_empty(), "and unlinked from the node it left");
    }

    #[test]
    fn a_self_loop_lands_on_the_list_twice_as_lemon_puts_it_there_twice() {
        let mut g = Graph::with_points(&[(0, 0)]);
        g.add_edge(0, 0);
        assert_eq!(g.incident(0), &[0, 0], "both arcs of the edge are on the node's list");
    }
}