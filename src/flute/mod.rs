// SPDX-License-Identifier: Apache-2.0
//! FLUTE — rectilinear Steiner minimal trees from a precomputed lookup table.
//!
//! ⛔ **The algorithm is mostly DATA.** Upstream ships `POWV9.dat` (2.9 MB) and `POST9.dat`
//! (6.5 MB), base64-encoded into the binary and decoded at runtime. They are the precomputed
//! optimal topologies for degree ≤ 9; there is no deriving them, only carrying them.
//!
//! This module is the table's shape and its decoders. Everything else in FLUTE reads through it.

pub mod breaks;
pub mod entry;
pub mod index;
pub mod low_degree;
pub mod medium_degree;
pub mod merge;
pub mod lut;
pub mod refine;
pub mod score;
pub mod solve;

pub use entry::{degenerate, prepare, Prepared};
pub use index::{gaps, group_index, GroupIndex};
pub use low_degree::flutes_low_degree;
pub use medium_degree::{flutes_all_degree, flutes_medium_degree};
pub use merge::d_merge_tree;
pub use solve::{best_solution, build_tree};
pub use lut::{char_num, read_decimal_int, Csoln, K_NUM_GROUP, MAX_LUT_DEGREE};

use crate::tree::Tree;

/// `Flute::flute` — the public entry point.
///
/// Returns `None` when the net cannot be built. ⚠️ **Two different reasons produce that**, and
/// the caller should not conflate them: a degree above the lookup table whose decomposition and
/// heuristic both decline, and — currently — a merged tree containing a parent cycle, which is a
/// defect of ours rather than a property of the net. See `refine::local_refinement`.
pub fn flute(lut: &lut::Lut, x: &[i32], y: &[i32]) -> Option<Tree> {
    flute_acc(lut, x, y, medium_degree::ACCURACY)
}

/// `Flute::flute(x, y, acc)` — the same entry point with the accuracy the caller chose.
///
/// ⚠️ **The accuracy is a CALLER's parameter in the reference, not a constant of the builder.**
/// `SteinerTreeBuilder` passes its own `kFluteAccuracy`; `grt` and the Tcl `stt` command pass
/// others. The two happen to be 3 today, and folding them into one constant would hide the day
/// one of them moves.
pub fn flute_acc(lut: &lut::Lut, x: &[i32], y: &[i32], acc: i32) -> Option<Tree> {
    if let Some(t) = entry::degenerate(x, y) {
        return Some(t);
    }
    let d = x.len();
    let p = entry::prepare(x, y);
    medium_degree::flutes_all_degree_acc(lut, d, &p.xs, &p.ys, &p.s, acc)
}