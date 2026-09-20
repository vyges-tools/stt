// SPDX-License-Identifier: Apache-2.0
//! The break-selection loop of `flutes_medium_degree`.
//!
//! Every position in `lb ..= ub` gets two candidate breaks — one in x, one in y — scored by how
//! much they look like a good place to cut. The best `acc` of them are tried for real, each
//! splitting the net and recursing, and the shortest result wins.

use super::score::{K_AA, K_BB};

/// A candidate break: which position, and which direction.
///
/// ⚠️ **The encoding is positional, not a struct in the reference**: `bp / 2 + lb` is the
/// position and `bp % 2 == 0` means "break in x". Scores are written in pairs as `nbp` advances,
/// so the parity of the index IS the direction. Storing the two separately and sorting would
/// lose the pairing that `break_pt`/`break_in_x` rely on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Break {
    pub position: usize,
    pub in_x: bool,
}

pub fn break_pt(bp: usize, lb: usize) -> usize {
    bp / 2 + lb
}

// ⚠️ `bp % 2 == 0`, not `bp.is_multiple_of(2)`. The reference writes the modulus, and the point
// of this function is to be read against `#define break_in_x(bp) (!((bp) & 1))`.
#[allow(clippy::manual_is_multiple_of)]
pub fn break_in_x(bp: usize) -> bool {
    bp % 2 == 0
}

pub fn decode(bp: usize, lb: usize) -> Break {
    Break { position: break_pt(bp, lb), in_x: break_in_x(bp) }
}

/// Build the score array — two entries per position, x-break first.
///
/// ```text
/// if (si[r] <= 1)        score = (xs[r+1]-xs[r-1]) - penalty[r] - kAa*(ys[2]-ys[1])      - dd*disty[r];
/// else if (si[r] >= d-2) score = (xs[r+1]-xs[r-1]) - penalty[r] - kAa*(ys[d-2]-ys[d-3])  - dd*disty[r];
/// else                   score = (xs[r+1]-xs[r-1]) - penalty[r] - kBb*(ys[si[r]+1]-ys[si[r]-1]) - dd*disty[r];
/// ```
///
/// ⚠️ **`kAa`/`kBb` are `double` and the rest is `float`**, so each term
/// `kAa * (ys[..] - ys[..])` is a **double** multiply that then participates in a mixed
/// expression. The reference's `score[]` is `float`, so the whole right-hand side is evaluated at
/// the widest operand — `double` — and rounded once into the `float` slot.
///
/// ⚠️ **The x-break uses `disty` and the y-break uses `distx`** — crossed, not matched. Pairing
/// each with its own axis looks natural and is a different heuristic.
///
/// ⚠️ The boundary tests are on `si[r]` for the x-break and on `s[r]` for the y-break.
pub fn scores(
    d: usize,
    xs: &[i32],
    ys: &[i32],
    s: &[usize],
    si: &[usize],
    penalty: &[f32],
    distx: &[i32],
    disty: &[i32],
    dd: f32,
    lb: usize,
    ub: usize,
) -> Vec<f32> {
    let mut out = Vec::with_capacity(2 * (ub + 1 - lb));
    for r in lb..=ub {
        // x-break: keyed on si[r], penalised by penalty[r], spanned by disty[r].
        let span = if si[r] <= 1 {
            K_AA * f64::from(ys[2] - ys[1])
        } else if si[r] >= d - 2 {
            K_AA * f64::from(ys[d - 2] - ys[d - 3])
        } else {
            K_BB * f64::from(ys[si[r] + 1] - ys[si[r] - 1])
        };
        let v = f64::from(xs[r + 1] - xs[r - 1])
            - f64::from(penalty[r])
            - span
            - f64::from(dd) * f64::from(disty[r]);
        out.push(v as f32);

        // y-break: keyed on s[r], penalised by penalty[s[r]], spanned by distx[r].
        let span = if s[r] <= 1 {
            K_AA * f64::from(xs[2] - xs[1])
        } else if s[r] >= d - 2 {
            K_AA * f64::from(xs[d - 2] - xs[d - 3])
        } else {
            K_BB * f64::from(xs[s[r] + 1] - xs[s[r] - 1])
        };
        let v = f64::from(ys[r + 1] - ys[r - 1])
            - f64::from(penalty[s[r]])
            - span
            - f64::from(dd) * f64::from(distx[r]);
        out.push(v as f32);
    }
    out
}

/// The value the reference writes over a score once it has been tried.
///
/// ⚠️ **`-9e9` is a magic sentinel, not negative infinity**, and it is stored into a `float`. Any
/// genuine score below it would be picked again — the reference accepts that, and substituting
/// `f32::NEG_INFINITY` would be a different (and more correct) program.
pub const SUPPRESSED: f32 = -9e9;

/// How much of the score list to try, and the accuracy the recursion runs at.
///
/// ```text
/// if (acc <= 3) newacc = 1;
/// else { newacc = acc / 2; if (acc >= nbp) acc = nbp - 1; }
/// ```
///
/// ⚠️ **The clamp on `acc` happens ONLY in the `else`** — with `acc <= 3` it is left alone even
/// if it exceeds `nbp`. And `newacc` is derived from the ORIGINAL `acc`, before the clamp.
pub fn accuracy(acc: i32, nbp: usize) -> (i32, i32) {
    if acc <= 3 {
        (1, acc)
    } else {
        let newacc = acc / 2;
        let acc = if acc >= nbp as i32 { nbp as i32 - 1 } else { acc };
        (newacc, acc)
    }
}

/// Take the highest remaining score, suppressing it so the next call takes the next one.
///
/// ⚠️ **The scan keeps the FIRST maximum** (`score[maxbp] < score[bp]` is strict), so ties
/// resolve to the lower index — which, given the pairing, means the x-break before the y-break
/// at the same position.
pub fn take_best(score: &mut [f32]) -> usize {
    let mut maxbp = 0usize;
    for bp in 1..score.len() {
        if score[maxbp] < score[bp] {
            maxbp = bp;
        }
    }
    score[maxbp] = SUPPRESSED;
    maxbp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_parity_encodes_the_direction() {
        // ⚠️ Scores are written in pairs, so parity IS the direction and bp/2 + lb is the
        // position. Storing direction separately and sorting would lose that pairing.
        assert_eq!(decode(0, 2), Break { position: 2, in_x: true });
        assert_eq!(decode(1, 2), Break { position: 2, in_x: false });
        assert_eq!(decode(2, 2), Break { position: 3, in_x: true });
        assert_eq!(decode(7, 2), Break { position: 5, in_x: false });
    }

    #[test]
    fn take_best_suppresses_with_the_magic_value_and_keeps_the_first_maximum() {
        let mut score = vec![1.0f32, 5.0, 5.0, 2.0];
        assert_eq!(take_best(&mut score), 1, "the FIRST of the tied maxima");
        assert_eq!(score[1], SUPPRESSED);
        assert_eq!(take_best(&mut score), 2, "the second one is taken next");
        assert_eq!(take_best(&mut score), 3);
        assert_eq!(take_best(&mut score), 0);
    }

    #[test]
    fn the_sentinel_is_a_magic_number_not_negative_infinity() {
        // ⚠️ A genuine score below -9e9 would be selected again. The reference accepts that;
        // NEG_INFINITY would be a different, more correct program.
        assert!(SUPPRESSED.is_finite());
        assert_eq!(SUPPRESSED, -9e9f32);
        let mut score = vec![-1e10f32, 0.0];
        take_best(&mut score);           // takes index 1
        assert_eq!(take_best(&mut score), 1,
                   "the suppressed slot still outranks a score below the sentinel");
    }

    #[test]
    fn accuracy_clamps_only_on_the_high_branch_and_newacc_uses_the_original() {
        // acc <= 3: newacc is 1 and acc is untouched, even when it exceeds nbp.
        assert_eq!(accuracy(3, 2), (1, 3));
        assert_eq!(accuracy(1, 100), (1, 1));
        // acc > 3: newacc = acc/2 from the ORIGINAL acc, then acc is clamped to nbp-1.
        assert_eq!(accuracy(8, 20), (4, 8));
        assert_eq!(accuracy(8, 5), (4, 4), "newacc is 4 from the original 8, acc clamps to 4");
    }

    #[test]
    fn the_x_break_is_scored_against_disty_and_the_y_break_against_distx() {
        // 🔑 Crossed, not matched. Feeding each its own axis looks natural and is a different
        // heuristic — so the test moves ONLY disty and requires only the x-break to react.
        let d = 8usize;
        let xs: Vec<i32> = (0..d as i32).map(|i| i * 10).collect();
        let ys: Vec<i32> = (0..d as i32).map(|i| i * 10).collect();
        let s: Vec<usize> = (0..d).collect();
        let si: Vec<usize> = (0..d).collect();
        let penalty = vec![0f32; d + 1];
        let distx = vec![0i32; d + 1];
        let mut disty = vec![0i32; d + 1];
        let base = scores(d, &xs, &ys, &s, &si, &penalty, &distx, &disty, 1.0, 2, 5);
        disty[3] = 1000;
        let moved = scores(d, &xs, &ys, &s, &si, &penalty, &distx, &disty, 1.0, 2, 5);
        // position 3 is bp = 2 (x) and bp = 3 (y).
        assert_ne!(base[2], moved[2], "the x-break at position 3 reacts to disty");
        assert_eq!(base[3], moved[3], "the y-break at position 3 does NOT");
    }
}