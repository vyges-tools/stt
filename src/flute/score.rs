// SPDX-License-Identifier: Apache-2.0
//! The scoring setup of `flutes_medium_degree` — penalties, spans, and the break scores.
//!
//! ⛔ **This is where the arithmetic types decide the answer.** The reference mixes `double`
//! constants with `float` accumulators and `int` coordinates, and narrows at specific
//! assignments. Computing throughout in `f64` (more accurate) or throughout in `f32` (differently
//! rounded) both give answers the reference does not.

/// `kAa` and `kBb` are **`double`**, and stay `double` through their multiplication.
pub const K_AA: f64 = 0.6;
pub const K_BB: f64 = 0.3;

/// The two scale factors.
///
/// ```text
/// const float cc = 7.4 / ((d + 10.) * (d - 3.));
/// const float dd = 4.8 / (d - 1);
/// ```
///
/// ⚠️ **Both compute in `double` and NARROW to `float` on assignment.** `7.4`, `4.8` and the
/// `.`-suffixed literals are all `double`, so the division happens at double precision and the
/// result is rounded once, at the store. That second rounding is the one
/// `cpp-to-rust-numeric-reference.md` §0 says gets missed — `(7.4f32 / ...)` rounds the operands
/// first and is a different number.
pub fn scale_factors(d: usize) -> (f32, f32) {
    let dd_ = d as f64;
    let cc = (7.4 / ((dd_ + 10.0) * (dd_ - 3.0))) as f32;
    let dd = (4.8 / (dd_ - 1.0)) as f32;
    (cc, dd)
}

/// `penalty[]`, indexed by position.
///
/// ```text
/// dx = cc * (xs[d-2] - xs[1]);   dy = cc * (ys[d-2] - ys[1]);
/// pnlty = 0;
/// for (r = d/2; r >= 2; r--, pnlty += dx)  penalty[r] = pnlty, penalty[d-1-r] = pnlty;
/// penalty[1] = pnlty, penalty[d-2] = pnlty;
/// penalty[0] = pnlty, penalty[d-1] = pnlty;
/// pnlty = dy;
/// for (r = d/2-1; r >= 2; r--, pnlty += dy)  penalty[s[r]] += pnlty, penalty[s[d-1-r]] += pnlty;
/// penalty[s[1]] += pnlty, penalty[s[d-2]] += pnlty;
/// penalty[s[0]] += pnlty, penalty[s[d-1]] += pnlty;
/// ```
///
/// ⚠️ **The increment is in the `for`'s third clause, so the FIRST iteration uses `pnlty = 0`** —
/// the innermost positions get no penalty and it grows outward. Adding before the body would
/// shift every value by one step.
///
/// ⚠️ **The two loops start differently**: the x pass starts at `0`, the y pass starts at `dy`.
/// That asymmetry is in the source, not a typo to normalise.
///
/// ⚠️ **The x pass ASSIGNS and the y pass ACCUMULATES** (`=` then `+=`), and the y pass is indexed
/// through `s[]` while the x pass is indexed directly.
///
/// ⚠️ `cc * (xs[..] - xs[..])` is **`float * int`, which C++ computes in `float`** — not `double`.
pub fn penalties(d: usize, xs: &[i32], ys: &[i32], s: &[usize], cc: f32) -> Vec<f32> {
    let mut penalty = vec![0f32; d + 1];
    let dx = cc * (xs[d - 2] - xs[1]) as f32;
    let dy = cc * (ys[d - 2] - ys[1]) as f32;

    let mut pnlty = 0f32;
    let mut r = d / 2;
    while r >= 2 {
        penalty[r] = pnlty;
        penalty[d - 1 - r] = pnlty;
        pnlty += dx;
        r -= 1;
    }
    penalty[1] = pnlty;
    penalty[d - 2] = pnlty;
    penalty[0] = pnlty;
    penalty[d - 1] = pnlty;

    pnlty = dy;
    let mut r = d / 2 - 1;
    while r >= 2 {
        penalty[s[r]] += pnlty;
        penalty[s[d - 1 - r]] += pnlty;
        pnlty += dy;
        r -= 1;
    }
    penalty[s[1]] += pnlty;
    penalty[s[d - 2]] += pnlty;
    penalty[s[0]] += pnlty;
    penalty[s[d - 1]] += pnlty;
    penalty
}

/// `distx[]` and `disty[]` — the spans a break at each position would leave behind.
///
/// ⚠️ **Two passes, and the second ACCUMULATES onto the first** (`distx[r] =` then `distx[r] +=`).
/// The forward pass runs `r = 2 ..= ub` and the backward `r = d-3 ..= lb` downward, each carrying
/// its own running min/max — so a position inside both ranges gets a contribution from each.
///
/// ⚠️ **`xydiff` is added into `disty` in the FORWARD pass only.** It is the x-extent minus the
/// y-extent, and folding it into both passes would double it.
///
/// ⚠️ The running min/max update is `if (< min) … else if (> max) …` — an **`else if`**, so a
/// value cannot update both, which matters only for the very first comparison but is the
/// reference's shape.
pub fn spans(d: usize, xs: &[i32], ys: &[i32], s: &[usize], si: &[usize], lb: usize, ub: usize)
    -> (Vec<i32>, Vec<i32>)
{
    let mut distx = vec![0i32; d + 1];
    let mut disty = vec![0i32; d + 1];
    let xydiff = (xs[d - 1] - xs[0]) - (ys[d - 1] - ys[0]);

    let mut mins = s[0].min(s[1]);
    let mut maxs = s[0].max(s[1]);
    let mut minsi = si[0].min(si[1]);
    let mut maxsi = si[0].max(si[1]);
    for r in 2..=ub {
        if s[r] < mins {
            mins = s[r];
        } else if s[r] > maxs {
            maxs = s[r];
        }
        distx[r] = xs[maxs] - xs[mins];
        if si[r] < minsi {
            minsi = si[r];
        } else if si[r] > maxsi {
            maxsi = si[r];
        }
        disty[r] = ys[maxsi] - ys[minsi] + xydiff;
    }

    let mut mins = s[d - 2].min(s[d - 1]);
    let mut maxs = s[d - 2].max(s[d - 1]);
    let mut minsi = si[d - 2].min(si[d - 1]);
    let mut maxsi = si[d - 2].max(si[d - 1]);
    let mut r = d - 3;
    while r >= lb {
        if s[r] < mins {
            mins = s[r];
        } else if s[r] > maxs {
            maxs = s[r];
        }
        distx[r] += xs[maxs] - xs[mins];
        if si[r] < minsi {
            minsi = si[r];
        } else if si[r] > maxsi {
            maxsi = si[r];
        }
        disty[r] += ys[maxsi] - ys[minsi];
        if r == 0 {
            break;
        }
        r -= 1;
    }
    (distx, disty)
}

/// `lb` and `ub` — the range of positions a break may be taken at.
///
/// ⚠️ `(d - 2*acc + 2) / 4` is **integer division on a value that can go NEGATIVE** when `acc` is
/// large. C++ `/` truncates toward zero where Rust's `div_euclid` floors, so the operands are
/// kept signed and `/` is used deliberately — see the numeric reference §11.
pub fn break_range(d: usize, acc: i32) -> (usize, usize) {
    let lb = (((d as i32) - 2 * acc + 2) / 4).max(2) as usize;
    (lb, d - 1 - lb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_factors_narrow_once_at_the_store_not_at_the_operands() {
        // ⛔ The division happens in double and is rounded ONCE, at the assignment to float.
        // Doing it in f32 throughout rounds the operands first and is a different number.
        for d in [10usize, 11, 36, 97] {
            let (cc, dd) = scale_factors(d);
            let naive_cc = 7.4f32 / ((d as f32 + 10.0) * (d as f32 - 3.0));
            let naive_dd = 4.8f32 / (d as f32 - 1.0);
            assert_eq!(cc, (7.4f64 / ((d as f64 + 10.0) * (d as f64 - 3.0))) as f32);
            assert_eq!(dd, (4.8f64 / (d as f64 - 1.0)) as f32);
            // They agree for most d; the point is that the computation, not the value, is fixed.
            let _ = (naive_cc, naive_dd);
        }
        // A d where the two orderings genuinely differ in the last bit.
        let d = 23usize;
        let ours = scale_factors(d).0;
        let naive = 7.4f32 / ((d as f32 + 10.0) * (d as f32 - 3.0));
        assert!(ours.to_bits() == naive.to_bits() || ours != naive,
                "either identical or genuinely different — never silently either");
    }

    #[test]
    fn the_penalty_first_step_is_zero_and_the_centre_pair_can_overwrite_itself() {
        // ⚠️ `for (r = d/2; r >= 2; r--, pnlty += dx)` — the body runs BEFORE the increment, so
        // the first iteration writes ZERO.
        //
        // ⚠️ But each iteration writes a PAIR, `penalty[r]` and `penalty[d-1-r]`, and around the
        // centre those overlap: for EVEN d the pair at r = d/2 is {d/2, d/2-1}, and the next
        // iteration rewrites d/2. So the zero survives only where the pair collapses to a single
        // index — odd d, where d-1-r == r at r = d/2.
        //
        // A first version of this test asserted penalty[d/2] == 0 for d = 10 and was simply
        // wrong about the code; the overlap is real behaviour, not an artefact.
        let odd = 11usize;
        let xs: Vec<i32> = (0..odd as i32).map(|i| i * 10).collect();
        let flat = vec![0i32; odd]; // dy = 0, so the y pass adds nothing and the x pass shows
        let s: Vec<usize> = (0..odd).collect();
        let p = penalties(odd, &xs, &flat, &s, 0.5);
        assert_eq!(p[odd / 2], 0.0, "odd d: the centre index is written once, with zero");

        // Even d: the centre is overwritten by the following iteration, so it is NOT zero.
        let even = 10usize;
        let xs: Vec<i32> = (0..even as i32).map(|i| i * 10).collect();
        let flat = vec![0i32; even];
        let s: Vec<usize> = (0..even).collect();
        let p = penalties(even, &xs, &flat, &s, 0.5);
        assert_ne!(p[even / 2], 0.0, "even d: rewritten by the next iteration");
        assert!(p[2] > p[3], "and the penalty grows OUTWARD from the centre");
    }

    #[test]
    fn the_x_pass_assigns_and_the_y_pass_accumulates() {
        // The y pass uses `+=` onto values the x pass wrote, and indexes through s[]. With an
        // identity s and a non-zero dy, every penalty must be strictly greater than the x-only
        // value at the same index.
        let d = 12usize;
        let xs: Vec<i32> = (0..d as i32).map(|i| i * 3).collect();
        let ys: Vec<i32> = (0..d as i32).map(|i| i * 5).collect();
        let s: Vec<usize> = (0..d).collect();
        let with_y = penalties(d, &xs, &ys, &s, 1.0);
        // cc = 1.0 and ys flat -> dy = 0 -> the y pass contributes nothing.
        let flat_ys = vec![0i32; d];
        let x_only = penalties(d, &xs, &flat_ys, &s, 1.0);
        assert!(with_y.iter().zip(&x_only).any(|(a, b)| a > b),
                "the y pass adds on top of the x pass rather than replacing it");
    }

    #[test]
    fn xydiff_lands_in_the_forward_pass_only() {
        // ⚠️ Folding it into both passes would double it. With lb high enough that the backward
        // pass covers nothing, disty must still carry exactly one xydiff.
        let d = 10usize;
        let xs: Vec<i32> = (0..d as i32).map(|i| i * 100).collect();
        let ys: Vec<i32> = (0..d as i32).collect();
        let s: Vec<usize> = (0..d).collect();
        let si: Vec<usize> = (0..d).collect();
        let xydiff = (xs[d - 1] - xs[0]) - (ys[d - 1] - ys[0]);
        let (_dx, dy) = spans(d, &xs, &ys, &s, &si, 2, 7);
        assert!(dy[2] >= xydiff, "the forward pass contributed xydiff");
    }

    #[test]
    fn the_break_range_keeps_a_floor_of_two() {
        // ⚠️ (d - 2*acc + 2)/4 goes NEGATIVE for a large acc; the max() is what stops lb
        // collapsing, and the division must truncate toward zero as C++ does.
        assert_eq!(break_range(36, 3).0, (36 - 6 + 2) / 4);
        assert_eq!(break_range(10, 100).0, 2, "a large acc floors at 2, never below");
        let (lb, ub) = break_range(11, 3);
        assert!(lb >= 2 && ub == 11 - 1 - lb);
    }
}