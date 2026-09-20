// SPDX-License-Identifier: Apache-2.0
//! Rectilinear Steiner minimal trees over a net's pin locations.
//!
//! Two builders behind one entry point: Prim-Dijkstra when a wirelength/depth trade-off is asked
//! for, FLUTE otherwise — and FLUTE again whenever a Prim-Dijkstra tree fails
//! [`check::check_tree`].

// ⛔ **These three are transcription fidelity, not laziness, and the reason is the same for all
// of them: this crate is read side by side with the reference, and a shape that reads differently
// from the C++ is a shape the next reader cannot check.**
//
// * `needless_range_loop` — the reference indexes `branch[i]`, `s[i]` and `sorted[i]` by position,
//   and several of those loops index TWO arrays by the same `i` or write back to the one they
//   read. An iterator rewrite would either need `zip` chains that obscure the correspondence or
//   would not compile against the borrow.
// * `too_many_arguments` — `flutes_medium_degree_acc` takes what the reference's function takes.
//   Bundling them into a struct would hide which call site passes what, and the call sites are
//   exactly what §0 of the authoring checklist says to cross-check.
// * `non_snake_case` — test names carry the RULE, and the capitalised word is the part that must
//   not be missed (`an_INCREASE_that_actually_decreased_the_key…`). A failure line is the first
//   thing read when a gate goes red.
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_arguments)]
#![allow(non_snake_case)]

pub mod check;
pub mod events;
pub mod flute;
pub mod pd;
pub mod report;
pub mod tree;

pub use check::check_tree;
pub use report::{find_path_depth, report_steiner_tree};
pub use pd::{prim_dijkstra, PdError};
pub use tree::{Branch, Rect, Tree};

/// The FLUTE accuracy `makeSteinerTree` uses.
///
/// ⚠️ `SteinerTreeBuilder::kFluteAccuracy = 3` is its OWN constant, distinct from the accuracy
/// `flute()` defaults to even though both are 3 today. The accuracy decides how many break points
/// `medium_degree` tries, so a different value gives a different tree on the nets where it
/// matters; passing it explicitly is what keeps the two independent.
pub const FLUTE_ACCURACY: i32 = 3;

/// `SteinerTreeBuilder::makeSteinerTree` — the single entry point both builders sit behind.
///
/// ```text
/// if (alpha > 0.0) {
///   Tree tree = pdr::primDijkstra(x, y, drvr_index, alpha, logger_);
///   if (checkTree(tree)) { return tree; }
///   // Fall back to flute if PD fails.
/// }
/// return flute_->flute(x, y, kFluteAccuracy);
/// ```
///
/// ⚠️ **`alpha > 0.0`, strictly.** `alpha == 0` never reaches Prim-Dijkstra at all, even though
/// `primDijkstra` accepts it and yields a plain MST — the reference routes that case to FLUTE.
///
/// ⚠️ **A failed check is silent.** The PD tree is discarded and FLUTE builds the net; nothing is
/// logged at default verbosity. [`PdOutcome`] is returned alongside so a correlation harness can
/// attribute a topology to the builder that produced it, which the reference can only do under
/// `debugPrint`.
pub fn make_steiner_tree(
    lut: &flute::lut::Lut,
    x: &[i32],
    y: &[i32],
    drvr_index: usize,
    alpha: f32,
) -> (Option<Tree>, PdOutcome) {
    if alpha > 0.0 {
        match pd::prim_dijkstra(x, y, drvr_index, alpha) {
            Ok(tree) => {
                if check_tree(&tree) {
                    return (Some(tree), PdOutcome::PrimDijkstra);
                }
                return (
                    flute::flute_acc(lut, x, y, FLUTE_ACCURACY),
                    PdOutcome::FellBackCheckFailed,
                );
            }
            // The reference calls `Logger::error`, which aborts the command rather than falling
            // back. Surfacing it as an outcome keeps that distinguishable from a check failure.
            Err(e) => return (None, PdOutcome::Error(e)),
        }
    }
    (flute::flute_acc(lut, x, y, FLUTE_ACCURACY), PdOutcome::Flute)
}

/// Which builder produced the tree — the attribution the reference does not expose.
#[derive(Debug, PartialEq, Eq)]
pub enum PdOutcome {
    /// `alpha <= 0`, so FLUTE was asked directly.
    Flute,
    /// Prim-Dijkstra built it and it passed [`check_tree`].
    PrimDijkstra,
    /// Prim-Dijkstra built a tree, it FAILED the check, and FLUTE rebuilt the net.
    FellBackCheckFailed,
    /// `primDijkstra` rejected the input; the reference aborts here.
    Error(pd::PdError),
}

#[cfg(test)]
mod facade_tests {
    use super::*;

    fn lut() -> flute::lut::Lut {
        flute::lut::load_tables(flute::MAX_LUT_DEGREE).expect("load FLUTE tables")
    }

    #[test]
    fn alpha_zero_goes_to_flute_not_to_a_zero_alpha_prim_dijkstra() {
        // ⚠️ `alpha > 0.0` is STRICT. `prim_dijkstra` happily accepts 0 and returns an MST, so
        // the only thing keeping alpha=0 on the FLUTE path is the comparison itself.
        let (t, why) = make_steiner_tree(&lut(), &[0, 100, 0, 100], &[0, 0, 100, 100], 0, 0.0);
        assert_eq!(why, PdOutcome::Flute);
        assert!(t.is_some());
    }

    #[test]
    fn a_positive_alpha_uses_prim_dijkstra_when_the_tree_checks_out() {
        let (t, why) = make_steiner_tree(&lut(), &[0, 100, 0, 100], &[0, 0, 100, 100], 0, 0.5);
        assert_eq!(why, PdOutcome::PrimDijkstra);
        assert!(check_tree(&t.unwrap()));
    }

    #[test]
    fn the_two_builders_agree_on_a_two_pin_net_but_by_different_routes() {
        let (pd, why_pd) = make_steiner_tree(&lut(), &[0, 30], &[0, 40], 0, 0.5);
        let (fl, why_fl) = make_steiner_tree(&lut(), &[0, 30], &[0, 40], 0, 0.0);
        assert_eq!((why_pd, why_fl), (PdOutcome::PrimDijkstra, PdOutcome::Flute));
        assert_eq!(pd.unwrap().length, fl.unwrap().length, "70 either way");
    }

    #[test]
    fn an_empty_net_is_the_references_abort_not_a_flute_fallback() {
        // ⛔ The reference calls `Logger::error` inside `primDijkstra`, which ABORTS. It does not
        // reach the `return flute_->flute(...)` line, so treating this as a fallback would build
        // a tree where upstream builds none.
        let (t, why) = make_steiner_tree(&lut(), &[], &[], 0, 0.5);
        assert!(t.is_none());
        assert_eq!(why, PdOutcome::Error(PdError::Empty));
    }

    /// A 7-pin net whose Prim-Dijkstra tree self-overlaps, found by sweeping random nets: about
    /// one in thirty thousand does. ⛔ **Without a witness the fallback branch is untestable**,
    /// and an implementation that ignored `check_tree` entirely would pass every other test here.
    const OVERLAPPING: ([i32; 7], [i32; 7], f32) = (
        [157, 100, 99, 195, 83, 116, 172],
        [199, 49, 92, 50, 51, 55, 173],
        0.8,
    );

    #[test]
    fn a_self_overlapping_pd_tree_is_DISCARDED_and_flute_rebuilds_the_net() {
        let (xs, ys, alpha) = OVERLAPPING;
        let pd = pd::prim_dijkstra(&xs, &ys, 0, alpha).unwrap();
        assert!(!check_tree(&pd), "the witness must actually fail the check");

        let (t, why) = make_steiner_tree(&lut(), &xs, &ys, 0, alpha);
        assert_eq!(why, PdOutcome::FellBackCheckFailed);
        let t = t.expect("flute builds a 7-pin net");
        assert_ne!(t.length, pd.length, "the returned tree is FLUTE's, not the discarded one");
        assert!(check_tree(&t), "and FLUTE's does check out");
    }

    #[test]
    fn a_high_fanout_net_skips_the_check_and_keeps_its_pd_tree() {
        // deg > 100 short-circuits `check_tree`, so a large net can never fall back however its
        // segments lie. 120 pins on a diagonal.
        let xs: Vec<i32> = (0..120).map(|i| i * 37 % 1000).collect();
        let ys: Vec<i32> = (0..120).map(|i| i * 53 % 1000).collect();
        let (t, why) = make_steiner_tree(&lut(), &xs, &ys, 0, 0.4);
        assert_eq!(why, PdOutcome::PrimDijkstra, "deg 120 > 100 never falls back");
        assert_eq!(t.unwrap().deg, 120);
    }
}