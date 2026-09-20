// SPDX-License-Identifier: Apache-2.0
//! **P6** — `buildSpanningTree`, the Prim-Dijkstra search itself.
//!
//! A best-first search from the driver, where an edge's cost blends its own length with the path
//! length already accumulated to its parent. `alpha = 0` gives a minimum spanning tree; larger
//! alpha trades wirelength for shallower paths.

/// A heap key ordered exactly as the reference's comparator orders it.
///
/// ```text
/// std::tie(lhs.weight, lhs.parent, lhs.node) > std::tie(rhs.weight, rhs.parent, rhs.node)
/// ```
///
/// 🔑 **`node` is unique per entry, so this is a TOTAL order** — no two live entries can compare
/// equal. Pop order is therefore decided entirely by the comparator and not by the heap's
/// internal sift order, which is what lets an ordered set reproduce a `d_ary_heap` exactly.
/// Had the tie-break stopped at `weight`, the two would diverge on ties and no ordered structure
/// could match.
///
/// ⚠️ `parent` is `INVALID` for the root; the reference compares LEMON handles, whose invalid id
/// sorts below every real one, so `-1` reproduces it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Key {
    weight: f32,
    parent: i64,
    node: usize,
}

impl Eq for Key {}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // ⚠️ `total_cmp`, not `partial_cmp`: an ordering that can return None would silently
        // break the set. Weights here are finite and non-negative, but the guarantee should not
        // rest on that.
        self.weight
            .total_cmp(&other.weight)
            .then(self.parent.cmp(&other.parent))
            .then(self.node.cmp(&other.node))
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}


/// `boost::heap::d_ary_heap<SearchEdge, arity<2>, mutable_<true>, compare<CmpEdge>>`, as an array.
///
/// ⛔ **An ordered set is NOT a substitute here, and that is measured, not assumed.** The search
/// re-parents a node on an EQUAL weight (`neighbor_weight <= (*handle).weight`) and then calls
/// `heap.increase(handle)`. Boost documents `increase` as *"the new value is expected to be
/// greater than the current one ... otherwise the behavior is undefined"*, and it is implemented
/// as `siftup` alone. An equal-weight re-parent to a HIGHER parent id makes the tuple larger,
/// i.e. the heap key SMALLER, so the element should sink — and `siftup` cannot sink it. It keeps
/// its old position and pops early.
///
/// 🔑 On `clk` in upstream's own `gcd.nets` this is the whole difference: nodes 4 and 20 both sit
/// at weight 64.8 with parent 22, the comparator says node 4 pops first, and the reference pops
/// node 20 — because node 20's entry was re-parented 5 → 22 at equal weight and left where it
/// was. Reproducing the heap reproduces the tree; reproducing only the comparator does not.
///
/// So this is a transcription of boost's array layout and sift order, not a priority queue:
///
/// ```text
/// push:      q.push_back(v); siftup(size - 1)
/// pop:       swap(q.front(), q.back()); q.pop_back(); siftdown(0)
/// increase:  siftup(index)                       // sift UP only — the bug above lives here
/// siftup:    while i: p = (i-1)/D; if cmp(q[p], q[i]) swap else stop
/// siftdown:  while !leaf: c = top_child(i); if !cmp(q[c], q[i]) swap else stop
/// top_child: std::max_element over the <= D children — the FIRST maximum on a tie
/// ```
struct BoostHeap {
    q: Vec<Key>,
    /// `handles[node]` — the element's index in `q`, or `NONE` when it is not in the heap.
    handles: Vec<usize>,
}

impl BoostHeap {
    const D: usize = 2;
    const NONE: usize = usize::MAX;

    fn new(n: usize) -> Self {
        BoostHeap { q: Vec::new(), handles: vec![Self::NONE; n] }
    }

    /// `super_t::operator()(a, b)` — heap-less-than. ⚠️ `CmpEdge` REVERSES the tuple comparison
    /// to turn boost's max-heap into a min-heap, so "a is heap-less-than b" means a's tuple is
    /// GREATER.
    fn cmp(a: &Key, b: &Key) -> bool {
        (a.weight, a.parent, a.node).partial_cmp(&(b.weight, b.parent, b.node))
            == Some(std::cmp::Ordering::Greater)
    }

    fn is_empty(&self) -> bool {
        self.q.is_empty()
    }

    fn top(&self) -> Key {
        self.q[0]
    }

    fn contains(&self, node: usize) -> bool {
        self.handles[node] != Self::NONE
    }

    fn peek(&self, node: usize) -> Key {
        self.q[self.handles[node]]
    }

    fn swap_at(&mut self, i: usize, j: usize) {
        self.handles[self.q[i].node] = j;
        self.handles[self.q[j].node] = i;
        self.q.swap(i, j);
    }

    fn push(&mut self, k: Key) {
        self.q.push(k);
        let i = self.q.len() - 1;
        self.handles[k.node] = i;
        self.siftup(i);
    }

    fn pop(&mut self) -> Key {
        let last = self.q.len() - 1;
        self.swap_at(0, last);
        let out = self.q.pop().expect("non-empty");
        self.handles[out.node] = Self::NONE;
        if !self.q.is_empty() {
            self.handles[self.q[0].node] = 0;
            self.siftdown(0);
        }
        out
    }

    /// Mutate an element in place and `siftup` — the reference's `increase`, verbatim, including
    /// the case where the key did not in fact increase.
    fn increase(&mut self, k: Key) {
        let i = self.handles[k.node];
        self.q[i] = k;
        self.siftup(i);
    }

    fn siftup(&mut self, mut index: usize) {
        while index != 0 {
            let parent = (index - 1) / Self::D;
            if Self::cmp(&self.q[parent], &self.q[index]) {
                self.swap_at(parent, index);
                index = parent;
            } else {
                return;
            }
        }
    }

    fn siftdown(&mut self, mut index: usize) {
        loop {
            let first = index * Self::D + 1;
            if first >= self.q.len() {
                return;
            }
            let last = (first + Self::D).min(self.q.len());
            // `std::max_element` keeps the FIRST maximum, so a later child that merely ties does
            // not displace an earlier one.
            let mut child = first;
            for c in (first + 1)..last {
                if Self::cmp(&self.q[child], &self.q[c]) {
                    child = c;
                }
            }
            if !Self::cmp(&self.q[child], &self.q[index]) {
                self.swap_at(child, index);
                index = child;
            } else {
                return;
            }
        }
    }
}

/// An edge of the spanning tree, `(parent, child)`, in the order the search added them.
pub type Edge = (usize, usize);

/// Build the Prim-Dijkstra spanning tree.
///
/// ⚠️ **The cost is `edge_length + alpha * parent_path_length`, evaluated in `f32`.** `alpha` is
/// a `float` and beats both integers, so the multiply and the add are single-precision — see
/// `cpp-to-rust-numeric-reference.md` §1. Computing in `f64` would be more accurate and would
/// pick different edges on near-ties.
///
/// ⚠️ **`path_length` is a separate INTEGER** accumulated exactly; only the *weight* is
/// approximate. Deriving one from the other loses that.
pub fn build_spanning_tree(
    pts: &[(i32, i32)],
    driver: usize,
    alpha: f32,
    nn: &[Vec<usize>],
) -> Vec<Edge> {
    let n = pts.len();
    let mut edges = Vec::new();
    if n == 0 {
        return edges;
    }
    let mut visited = vec![false; n];
    let mut num_visited = 0usize;
    let mut path_len: Vec<i32> = vec![0; n];
    let mut heap = BoostHeap::new(n);

    heap.push(Key { weight: 0.0, parent: -1, node: driver });
    path_len[driver] = 0;

    while !heap.is_empty() {
        let top = heap.top();
        heap.pop();

        visited[top.node] = true;
        num_visited += 1;
        if top.parent >= 0 {
            edges.push((top.parent as usize, top.node));
        }
        // ⚠️ The break is checked AFTER adding the edge and BEFORE expanding, so the last node
        // popped never has its neighbours examined. Moving it changes nothing about the tree but
        // does change how much work is done — keep it where the reference has it.
        if num_visited == n {
            break;
        }

        let cur_path = path_len[top.node];
        for &neighbor in &nn[top.node] {
            if visited[neighbor] {
                continue;
            }
            let edge_length = (pts[neighbor].0 - pts[top.node].0).abs()
                + (pts[neighbor].1 - pts[top.node].1).abs();
            let neighbor_path_length = edge_length + cur_path;
            let neighbor_weight = edge_length as f32 + alpha * cur_path as f32;

            let k = Key { weight: neighbor_weight, parent: top.node as i64, node: neighbor };
            if !heap.contains(neighbor) {
                heap.push(k);
                path_len[neighbor] = neighbor_path_length;
            } else if neighbor_weight <= heap.peek(neighbor).weight {
                // ⛔ `<=`, NOT `<`. An EQUAL weight still re-parents the node to the edge found
                // later — and then `increase` cannot sink the entry it just made smaller. Both
                // halves are load-bearing; see `BoostHeap`.
                heap.increase(k);
                path_len[neighbor] = neighbor_path_length;
            }
        }
    }
    edges
}

// ⛔ **TWO DETAILS HERE ARE STILL UNWITNESSED**, and saying which is the point:
//
//   1. the `f32` weight arithmetic (`edge_length + alpha * path_length` in single precision),
//   2. `top_child_index` taking the FIRST maximum, which is `std::max_element`'s contract.
//
// Both were verified by CONTROL to change NOTHING observable: computing the weight in `f64` and
// taking the last maximum instead of the first each leave all 145 `pd_gcd` trees, both `pd`
// cases and the whole gate corpus byte-identical. They are transcribed from the reference and
// believed right; no input we have distinguishes them. ⚠️ That is not the same as verified, and
// a future net that separates them belongs in the corpus the moment one is found.
//
// 🔑 The other two details that were unwitnessed here ARE now pinned, and neither by reasoning:
//   * the `<=` re-parent on an EQUAL weight — `pd_reparent_on_equal_weight` in the gate corpus,
//     which is upstream's own `_111_`; with `<` it and seven other `pd_gcd` nets change.
//   * the comparator's `node` tie-break — `the_heap_pops_in_ascending_weight_then_parent_then_node`.
// Both were found by building the reference with its call sequence instrumented and diffing the
// trace, after three cycles of inference had each produced a plausible wrong answer.

#[cfg(test)]
mod tests {
    use super::*;

    // ── the heap itself ──────────────────────────────────────────────────────────────────────

    fn k(weight: f32, parent: i64, node: usize) -> Key {
        Key { weight, parent, node }
    }

    #[test]
    fn the_heap_pops_in_ascending_weight_then_parent_then_node() {
        let mut h = BoostHeap::new(8);
        for e in [k(5.0, 1, 3), k(2.0, 7, 1), k(5.0, 0, 4), k(2.0, 7, 0)] {
            h.push(e);
        }
        let mut got = Vec::new();
        while !h.is_empty() {
            let t = h.top();
            h.pop();
            got.push((t.weight, t.parent, t.node));
        }
        assert_eq!(got, vec![(2.0, 7, 0), (2.0, 7, 1), (5.0, 0, 4), (5.0, 1, 3)]);
    }

    #[test]
    fn an_INCREASE_that_actually_decreased_the_key_leaves_the_element_where_it_was() {
        // ⛔ **The rule this pins is a boost contract VIOLATION in the reference, and it decides
        // the tree.** `buildSpanningTree` re-parents on an EQUAL weight and then calls
        // `heap.increase(handle)`. Boost: *"The new value is expected to be greater than the
        // current one ... otherwise the behavior of the data structure is undefined"*, and
        // `increase` is `siftup` alone. An equal-weight re-parent to a HIGHER parent id makes the
        // heap key smaller, so the element ought to sink — and `siftup` cannot sink it.
        //
        // 🔑 These are the real numbers from `clk` in upstream's `gcd.nets`: nodes 20 and 4 both
        // end at weight 64.8 with parent 22, a correct priority queue pops node 4 first, and the
        // reference pops node 20. Reproducing the comparator alone gave 144/145 on `pd_gcd`;
        // reproducing the heap gives 145/145.
        let mut h = BoostHeap::new(32);
        h.push(k(64.8, 5, 20));
        h.push(k(65.4, 21, 4));
        assert_eq!(h.top().node, 20, "node 20 is lighter to begin with");

        h.increase(k(64.8, 22, 4)); //  65.4 -> 64.8: a genuine increase, sifts up correctly
        h.increase(k(64.8, 22, 20)); // equal weight, parent 5 -> 22: NOT an increase

        assert_eq!(
            h.top().node, 20,
            "node 20 keeps its place although (64.8, 22, 4) is now the smaller tuple"
        );
    }

    #[test]
    fn an_ORDER_CORRECT_update_would_pop_the_other_node_which_is_why_this_matters() {
        // The control for the test above, spelled out: sifting DOWN as well — which is what
        // `heap.update` does and what any ordered set does — pops node 4 instead. That is one
        // net's topology on `clk`, and it is the entire difference between 144 and 145.
        let mut h = BoostHeap::new(32);
        h.push(k(64.8, 5, 20));
        h.push(k(65.4, 21, 4));
        h.increase(k(64.8, 22, 4));
        let i = h.handles[20];
        h.q[i] = k(64.8, 22, 20);
        h.siftdown(i); // the sink `increase` omits
        assert_eq!(h.top().node, 4);
    }

    #[test]
    fn a_tie_between_children_keeps_the_FIRST_one() {
        // `top_child_index` is `std::max_element`, which returns the first maximum. Two children
        // that compare equal must therefore leave the earlier one selected.
        let mut h = BoostHeap::new(8);
        h.push(k(0.0, -1, 0));
        h.push(k(9.0, 1, 1));
        h.push(k(9.0, 1, 2));
        h.pop();
        assert_eq!(h.top().node, 1, "the lower node id is the smaller tuple and pops first");
    }

    use crate::pd::nearest_neighbors;

    #[test]
    fn a_single_node_yields_no_edges() {
        let pts = [(0, 0)];
        assert!(build_spanning_tree(&pts, 0, 0.0, &nearest_neighbors(&pts)).is_empty());
    }

    #[test]
    fn a_spanning_tree_has_exactly_n_minus_one_edges_and_reaches_every_node() {
        for n in 2..12usize {
            let pts: Vec<(i32, i32)> =
                (0..n).map(|i| ((i as i32 * 37) % 101, (i as i32 * 53) % 97)).collect();
            let nn = nearest_neighbors(&pts);
            for alpha in [0.0f32, 0.3, 0.7, 1.0] {
                let e = build_spanning_tree(&pts, 0, alpha, &nn);
                assert_eq!(e.len(), n - 1, "n={n} alpha={alpha}: n-1 edges");
                let mut seen = vec![false; n];
                seen[0] = true;
                for &(p, c) in &e {
                    assert!(seen[p], "n={n} alpha={alpha}: parent {p} reached before child {c}");
                    seen[c] = true;
                }
                assert!(seen.iter().all(|&b| b), "every node is reached");
            }
        }
    }

    #[test]
    fn alpha_zero_is_a_minimum_spanning_tree_over_the_neighbour_graph() {
        // With alpha = 0 the weight is the edge length alone, so the total must match a
        // straightforward MST over the same neighbour relation.
        let pts: Vec<(i32, i32)> = vec![(0, 0), (10, 0), (0, 10), (10, 10), (5, 5)];
        let nn = nearest_neighbors(&pts);
        let e = build_spanning_tree(&pts, 0, 0.0, &nn);
        let total: i32 = e.iter()
            .map(|&(a, b)| (pts[a].0 - pts[b].0).abs() + (pts[a].1 - pts[b].1).abs())
            .sum();
        // Prim over the same relation, by hand.
        let mut inn = vec![false; pts.len()];
        inn[0] = true;
        let mut mst = 0i32;
        for _ in 1..pts.len() {
            let mut best = (i32::MAX, usize::MAX);
            for a in 0..pts.len() {
                if !inn[a] { continue; }
                for &b in &nn[a] {
                    if inn[b] { continue; }
                    let d = (pts[a].0 - pts[b].0).abs() + (pts[a].1 - pts[b].1).abs();
                    if d < best.0 { best = (d, b); }
                }
            }
            inn[best.1] = true;
            mst += best.0;
        }
        assert_eq!(total, mst, "alpha=0 must be an MST over the neighbour graph");
    }

    #[test]
    fn a_higher_alpha_never_makes_paths_from_the_driver_longer() {
        // 🔑 What alpha is FOR: it trades total wirelength for shallower paths. The deepest path
        // from the driver must not grow as alpha rises.
        let pts: Vec<(i32, i32)> =
            (0..10).map(|i| ((i * 29) % 83, (i * 17) % 71)).collect();
        let nn = nearest_neighbors(&pts);
        let depth = |alpha: f32| -> i32 {
            let e = build_spanning_tree(&pts, 0, alpha, &nn);
            let mut d = vec![0i32; pts.len()];
            for &(p, c) in &e {
                d[c] = d[p] + (pts[p].0 - pts[c].0).abs() + (pts[p].1 - pts[c].1).abs();
            }
            *d.iter().max().unwrap()
        };
        assert!(depth(1.0) <= depth(0.0), "alpha=1 must not deepen the tree vs alpha=0");
    }

    #[test]
    fn the_driver_is_the_root_and_never_a_child() {
        let pts: Vec<(i32, i32)> = (0..8).map(|i| ((i * 13) % 47, (i * 31) % 53)).collect();
        let nn = nearest_neighbors(&pts);
        for driver in 0..pts.len() {
            let e = build_spanning_tree(&pts, driver, 0.5, &nn);
            assert!(e.iter().all(|&(_, c)| c != driver), "driver {driver} is never a child");
        }
    }
}