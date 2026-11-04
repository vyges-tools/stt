// SPDX-License-Identifier: Apache-2.0
//! **P4** — the nearest-neighbour relation Prim-Dijkstra searches over (`pdr::getNearestNeighbors`,
//! the reference's `neighbors.cpp` since OpenROAD 4155ac3).
//!
//! The relation, from *"Prim-Dijkstra Revisited"* §4: two nodes are neighbours when the smallest
//! bounding box holding both holds no other node. Since 4155ac3 the box is **closed**: a node on
//! its boundary also separates the pair. Routing through such a node costs no more than the
//! direct edge, so Prim-Dijkstra loses nothing, and pins sharing an x or a y (rows, sites, macro
//! edges) no longer link every pair on a line.
//!
//! ⚠️ This is deliberately NOT a Delaunay-style neighbourhood: one excludes edges a high alpha can
//! need, and substituting it changes which trees are reachable.
//!
//! The search is a left-to-right sweep after the double-monotone-chains method (Guo et al.,
//! DATE'26), adapted by the reference for shared coordinates:
//! - coincident points are merged: each later copy gets a zero-length edge to the FIRST point at
//!   that location (least index), which alone takes part in the sweep;
//! - a column (equal x) is swept at a time; column mates adjacent in y are linked, and they bound
//!   the rows a point's left neighbours may lie in;
//! - per row only the rightmost swept point can be a neighbour of a later one (any other is on the
//!   box boundary); a max tree over rows finds each step of the left staircases.

/// A max segment tree over y rows holding the x of the rightmost point swept in each row
/// (`i32::MIN` for none yet).
struct RowMaxTree {
    size: usize,
    max_x: Vec<i32>,
}

impl RowMaxTree {
    fn new(num_rows: usize) -> Self {
        let mut size = 1;
        while size < num_rows {
            size *= 2;
        }
        RowMaxTree { size, max_x: vec![i32::MIN; 2 * size] }
    }

    fn x(&self, row: usize) -> i32 {
        self.max_x[self.size + row]
    }

    /// A row's x only grows as the sweep moves right.
    fn set(&mut self, row: usize, x: i32) {
        let mut node = self.size + row;
        self.max_x[node] = x;
        node /= 2;
        while node > 0 {
            self.max_x[node] = self.max_x[2 * node].max(self.max_x[2 * node + 1]);
            node /= 2;
        }
    }

    /// The lowest row in `[lo, hi)` whose x exceeds `min_x`. An empty range finds none (4155ac3,
    /// 6e7b6d549d3: the descent is skipped rather than walked with an inverted range).
    fn first(&self, lo: usize, hi: usize, min_x: i32) -> Option<usize> {
        if lo >= hi {
            return None;
        }
        self.first_in(1, 0, self.size, lo, hi, min_x)
    }

    /// The highest row in `[lo, hi)` whose x exceeds `min_x`.
    fn last(&self, lo: usize, hi: usize, min_x: i32) -> Option<usize> {
        if lo >= hi {
            return None;
        }
        self.last_in(1, 0, self.size, lo, hi, min_x)
    }

    fn first_in(&self, node: usize, node_lo: usize, node_hi: usize, lo: usize, hi: usize, min_x: i32) -> Option<usize> {
        if node_hi <= lo || hi <= node_lo || self.max_x[node] <= min_x {
            return None;
        }
        if node_hi - node_lo == 1 {
            return Some(node_lo);
        }
        let mid = (node_lo + node_hi) / 2;
        self.first_in(2 * node, node_lo, mid, lo, hi, min_x).or_else(|| self.first_in(2 * node + 1, mid, node_hi, lo, hi, min_x))
    }

    fn last_in(&self, node: usize, node_lo: usize, node_hi: usize, lo: usize, hi: usize, min_x: i32) -> Option<usize> {
        if node_hi <= lo || hi <= node_lo || self.max_x[node] <= min_x {
            return None;
        }
        if node_hi - node_lo == 1 {
            return Some(node_lo);
        }
        let mid = (node_lo + node_hi) / 2;
        self.last_in(2 * node + 1, mid, node_hi, lo, hi, min_x).or_else(|| self.last_in(2 * node, node_lo, mid, lo, hi, min_x))
    }
}

/// Each point's neighbours, **sorted by index** (since 4155ac3, so the Prim-Dijkstra search does
/// not depend on the sweep order). Coincident points are neighbours only of the least-indexed
/// point at their location.
pub fn nearest_neighbors(pts: &[(i32, i32)]) -> Vec<Vec<usize>> {
    let n = pts.len();
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut link = |a: usize, b: usize| {
        neighbors[a].push(b);
        neighbors[b].push(a);
    };

    // Sweep order: columns left to right, each bottom to top; coincident points adjacent, least
    // index first. The key is unique, so the sort's stability does not matter.
    let mut sorted: Vec<usize> = (0..n).collect();
    sorted.sort_unstable_by_key(|&i| (pts[i].0, pts[i].1, i));

    // A later copy of a location needs only a zero-length edge to the first, which stands in for
    // it in the sweep.
    let mut unique: Vec<usize> = Vec::with_capacity(n);
    for &p in &sorted {
        match unique.last() {
            Some(&u) if pts[u] == pts[p] => link(u, p),
            _ => unique.push(p),
        }
    }

    let mut row_ys: Vec<i32> = unique.iter().map(|&p| pts[p].1).collect();
    row_ys.sort_unstable();
    row_ys.dedup();
    let row_of = |y: i32| row_ys.partition_point(|&v| v < y);

    let mut swept = RowMaxTree::new(row_ys.len());
    let mut row_pt: Vec<usize> = vec![usize::MAX; row_ys.len()];

    let mut col_begin = 0;
    let mut col_rows: Vec<usize> = Vec::new();
    while col_begin < unique.len() {
        let col_x = pts[unique[col_begin]].0;
        let mut col_end = col_begin;
        col_rows.clear();
        while col_end < unique.len() && pts[unique[col_end]].0 == col_x {
            col_rows.push(row_of(pts[unique[col_end]].1));
            col_end += 1;
        }

        for k in col_begin..col_end {
            let pt = unique[k];
            let at = k - col_begin;
            let row = col_rows[at];
            // Column mates bound the rows a left neighbour can be in: strictly between the mate
            // below and the mate above.
            let lo_row = if k > col_begin { col_rows[at - 1] + 1 } else { 0 };
            let hi_row = if k + 1 < col_end { col_rows[at + 1] } else { row_ys.len() };
            if k + 1 < col_end {
                link(pt, unique[k + 1]);
            }

            // Upper-left staircase, from the point's own row up: each step must be strictly
            // nearer in x than the last, or the last lies in its box.
            let mut min_x = i32::MIN;
            let mut r = swept.first(row, hi_row, min_x);
            while let Some(rr) = r {
                link(pt, row_pt[rr]);
                min_x = swept.x(rr);
                r = swept.first(rr + 1, hi_row, min_x);
            }

            // Lower-left staircase, from just below the row down. The point in the same row (if
            // any) bounds it too.
            min_x = swept.x(row);
            let mut r = swept.last(lo_row, row, min_x);
            while let Some(rr) = r {
                link(pt, row_pt[rr]);
                min_x = swept.x(rr);
                r = swept.last(lo_row, rr, min_x);
            }
        }

        // The column joins the swept set only after all of it has been searched.
        for k in col_begin..col_end {
            let row = col_rows[k - col_begin];
            swept.set(row, col_x);
            row_pt[row] = unique[k];
        }
        col_begin = col_end;
    }

    for list in &mut neighbors {
        list.sort_unstable();
    }
    neighbors
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference's own oracle for the relation (upstream gtest `bruteForceNeighbors`): a later
    /// copy of a location links to the first; other first-copies are neighbours when their CLOSED
    /// box holds no other first-copy; lists sorted.
    fn brute_force(pts: &[(i32, i32)]) -> Vec<Vec<usize>> {
        let n = pts.len();
        let rep: Vec<usize> = (0..n).map(|i| pts.iter().position(|p| *p == pts[i]).unwrap()).collect();
        let mut nn = vec![Vec::new(); n];
        for i in 0..n {
            if rep[i] != i {
                nn[i].push(rep[i]);
                nn[rep[i]].push(i);
            }
        }
        for i in 0..n {
            for j in 0..n {
                if i == j || rep[i] != i || rep[j] != j {
                    continue;
                }
                let (x0, x1) = (pts[i].0.min(pts[j].0), pts[i].0.max(pts[j].0));
                let (y0, y1) = (pts[i].1.min(pts[j].1), pts[i].1.max(pts[j].1));
                let blocked = (0..n).any(|k| k != i && k != j && rep[k] == k && (x0..=x1).contains(&pts[k].0) && (y0..=y1).contains(&pts[k].1));
                if !blocked {
                    nn[i].push(j);
                }
            }
        }
        for l in &mut nn {
            l.sort_unstable();
        }
        nn
    }

    /// A small deterministic generator (xorshift64*): the tests need many varied inputs, not the
    /// reference's random sequence.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 32) as u32
        }
    }

    fn entries(nn: &[Vec<usize>]) -> usize {
        nn.iter().map(Vec::len).sum()
    }

    // Upstream gtest `MatchesBruteForceWithTies`: tiny coordinate ranges, so ties in x, in y and
    // coincident points are the rule rather than the exception.
    #[test]
    fn matches_brute_force_with_ties() {
        let mut rng = Rng(1);
        for trial in 0..20_000 {
            let count = 1 + rng.next() % 14;
            let (xr, yr) = (1 + rng.next() % 6, 1 + rng.next() % 6);
            let pts: Vec<(i32, i32)> = (0..count).map(|_| ((rng.next() % xr) as i32, (rng.next() % yr) as i32)).collect();
            assert_eq!(nearest_neighbors(&pts), brute_force(&pts), "trial {trial}: {pts:?}");
        }
    }

    // Upstream gtest `MatchesBruteForceGeneralPosition`: distinct x and distinct y.
    #[test]
    fn matches_brute_force_in_general_position() {
        let mut rng = Rng(2);
        for trial in 0..500 {
            let count = 2 + (rng.next() % 60) as usize;
            let mut xs: Vec<i32> = (0..count as i32).collect();
            let mut ys = xs.clone();
            for v in [&mut xs, &mut ys] {
                for i in (1..v.len()).rev() {
                    v.swap(i, rng.next() as usize % (i + 1));
                }
            }
            let pts: Vec<(i32, i32)> = (0..count).map(|i| (xs[i] * 7, ys[i] * 3)).collect();
            assert_eq!(nearest_neighbors(&pts), brute_force(&pts), "trial {trial}");
        }
    }

    // Upstream gtest `CollinearOnlyLinksAdjacent`: 1000 pins on a line give 999 edges — the
    // defect 4155ac3 fixed linked every pair on a vertical line.
    #[test]
    fn collinear_points_link_only_adjacent_ones() {
        let k = 1000;
        let vertical: Vec<(i32, i32)> = (0..k).map(|i| (5, (i * 37) % k)).collect();
        let horizontal: Vec<(i32, i32)> = (0..k).map(|i| ((i * 37) % k, 5)).collect();
        assert_eq!(entries(&nearest_neighbors(&vertical)), 2 * (k as usize - 1));
        assert_eq!(entries(&nearest_neighbors(&horizontal)), 2 * (k as usize - 1));
    }

    // Upstream gtest `GridOnlyLinksOrthogonal`: on a grid only the four orthogonal mates remain.
    #[test]
    fn a_grid_links_only_orthogonal_mates() {
        let side = 50;
        let pts: Vec<(i32, i32)> = (0..side).flat_map(|x| (0..side).map(move |y| (x * 10, y * 20))).collect();
        assert_eq!(entries(&nearest_neighbors(&pts)), 2 * 2 * side as usize * (side as usize - 1));
    }

    // Upstream gtest `ColumnBesideStaircase`: each staircase point sees only the column point in
    // its own row.
    #[test]
    fn a_staircase_beside_a_column_sees_only_its_row() {
        let k = 500usize;
        let mut pts: Vec<(i32, i32)> = (0..k as i32).map(|i| (0, i)).collect();
        pts.extend((0..k as i32).map(|i| (1 + i, k as i32 - 1 - i)));
        let nn = nearest_neighbors(&pts);
        assert_eq!(entries(&nn), 2 * (k - 1) * 3 + 2);
        for i in 0..k {
            assert!(nn[k + i].iter().all(|&j| j >= k || j == k - 1 - i), "staircase point {i}: {:?}", nn[k + i]);
        }
    }

    // Upstream gtest `CoincidentPointsLinkToFirst`.
    #[test]
    fn coincident_points_link_to_the_first() {
        let nn = nearest_neighbors(&[(3, 3), (0, 0), (3, 3), (3, 3), (6, 6)]);
        assert_eq!(nn, vec![vec![1, 2, 3, 4], vec![0], vec![0], vec![0], vec![0]]);
    }

    // Upstream gtest `SinglePoint`.
    #[test]
    fn a_single_point_has_no_neighbours() {
        assert_eq!(nearest_neighbors(&[(4, 2)]), vec![Vec::<usize>::new()]);
    }

    // 🔑 The closed box: a point on the boundary (here on the shared x line) separates the pair,
    // which the open-box relation before 4155ac3 did not.
    #[test]
    fn a_point_on_the_box_boundary_blocks_the_pair() {
        let nn = nearest_neighbors(&[(0, 0), (0, 10), (0, 5)]);
        assert_eq!(nn, vec![vec![2], vec![2], vec![0, 1]]);
        let nn = nearest_neighbors(&[(0, 0), (10, 10), (5, 5)]);
        assert!(!nn[0].contains(&1) && nn[0].contains(&2));
    }
}
