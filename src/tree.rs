// SPDX-License-Identifier: Apache-2.0
//! A rectilinear Steiner tree, in the reference's own shape.
//!
//! The reference's `Tree` is `{deg, length, branch[]}` and a `Branch` is `{x, y, n}` where `n`
//! indexes the branch this one connects to. The first `deg` branches are the terminals, in the
//! order they were handed in; anything after them is a Steiner point.

/// One branch: a point and the index of the branch it joins.
///
/// ⚠️ `n` is an INDEX INTO THE SAME ARRAY, not a coordinate. The root's `n` points at itself,
/// which is how the walk terminates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Branch {
    pub x: i32,
    pub y: i32,
    pub n: usize,
}

/// A Steiner tree over a net's pins.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tree {
    /// Degree — the number of TERMINALS, which is not the number of branches.
    pub deg: usize,
    /// Total wirelength, as the reference reports it.
    pub length: i64,
    pub branch: Vec<Branch>,
}

impl Tree {
    pub fn branch_count(&self) -> usize {
        self.branch.len()
    }

    /// The reference's `Tree::printTree`, line for line.
    ///
    /// 🔑 **This format IS the golden.** Upstream's `report_flute_net` / `report_pd_net` /
    /// `report_stt_net` print exactly these lines and the `.ok` files hold them, so reproducing
    /// the text is reproducing the observable behaviour rather than a re-derivation of it.
    pub fn print_lines(&self) -> Vec<String> {
        self.branch
            .iter()
            .enumerate()
            .map(|(i, b)| format!("{} ({} {}) neighbor {} length {}", i, b.x, b.y, b.n,
                                  self.branch_length(i)))
            .collect()
    }

    /// Manhattan length of branch `i` to its neighbour.
    pub fn branch_length(&self, i: usize) -> i64 {
        let b = self.branch[i];
        let n = self.branch[b.n];
        (i64::from(b.x) - i64::from(n.x)).abs() + (i64::from(b.y) - i64::from(n.y)).abs()
    }
}

/// An axis-aligned rectangle that NORMALISES its corners on construction.
///
/// ⛔ **The normalisation is load-bearing, not tidiness.** `odb::Rect::init` is
/// `std::tie(xlo_, xhi_) = std::minmax(x1, x2)` — so a branch running right-to-left or
/// top-to-bottom yields the same rectangle as one running the other way, and the overlap test in
/// [`crate::check`] cannot be affected by branch direction. Taking the corners as given would
/// make that test direction-sensitive, which the reference's is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub xlo: i32,
    pub ylo: i32,
    pub xhi: i32,
    pub yhi: i32,
}

impl Rect {
    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Rect {
            xlo: x1.min(x2),
            ylo: y1.min(y2),
            xhi: x1.max(x2),
            yhi: y1.max(y2),
        }
    }

    /// ⚠️ **CLOSED on every side**, matching `odb::Rect::intersects`: rectangles that merely touch
    /// DO intersect. An open test would miss every abutting branch pair, which is most of them.
    pub fn intersects(&self, o: &Rect) -> bool {
        self.xlo <= o.xhi && self.xhi >= o.xlo && self.ylo <= o.yhi && self.yhi >= o.ylo
    }

    /// ⛔ **`rectAreaZero` in the reference means "is a POINT", despite the name.** It is
    /// `xMin == xMax && yMin == yMax` — BOTH dimensions collapsed. A horizontal branch has
    /// `xlo != xhi` and is therefore NOT "zero area" under this predicate, even though its area
    /// is zero in the ordinary sense. Reading the name instead of the body would exclude every
    /// axis-aligned branch from the overlap check, which is all of them.
    pub fn is_point(&self) -> bool {
        self.xlo == self.xhi && self.ylo == self.yhi
    }

    pub fn is_corner(&self, x: i32, y: i32) -> bool {
        (self.xlo == x && self.ylo == y)
            || (self.xlo == x && self.yhi == y)
            || (self.xhi == x && self.ylo == y)
            || (self.xhi == x && self.yhi == y)
    }

    /// Do the two rectangles meet at a shared corner? The reference tests all four corners of
    /// `o` against `self`, and not the reverse.
    pub fn shares_corner(&self, o: &Rect) -> bool {
        self.is_corner(o.xlo, o.ylo)
            || self.is_corner(o.xlo, o.yhi)
            || self.is_corner(o.xhi, o.ylo)
            || self.is_corner(o.xhi, o.yhi)
    }
}