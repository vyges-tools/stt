// SPDX-License-Identifier: Apache-2.0
//! The table's group index — how a point ordering becomes a row of the LUT.
//!
//! `flutes_low_degree` turns the `s[]` permutation into a single integer `k`, looks up
//! `lut[d][k]`, and may first reflect the problem horizontally. That index computation is the
//! joint between the geometry and the 9.4 MB of precomputed topologies, so it is isolated here
//! and tested on its own.

use super::lut::K_NUM_GROUP;

/// Where a degree's permutation landed, and whether the problem was reflected to get there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupIndex {
    pub k: usize,
    /// True when the index exceeded the stored half and the problem was mirrored in x.
    pub hflip: bool,
}

/// Encode `s[]` as the reference does.
///
/// ```text
/// int k = 0;
/// if (s[0] < s[2]) k++;
/// if (s[1] < s[2]) k++;
/// for (int i = 3; i <= d - 1; i++) {   // p0 = 0 always, skip i = 1 for symmetry
///     int pi = s[i];
///     for (int j = d - 1; j > i; j--)
///         if (s[j] < s[i]) pi--;
///     k = pi + (i + 1) * k;
/// }
/// ```
///
/// ⚠️ **This is a mixed-radix (Lehmer-style) code, and every part of it is load-bearing:**
///
/// - The seed uses **`s[2]` as the pivot** for both comparisons, not `s[0] < s[1]`.
/// - The loop starts at **`i = 3`**, because `p0` is always 0 and `i = 1` is skipped for
///   symmetry — that skip is what halves the space and makes the horizontal flip necessary.
/// - The inner loop counts **later** elements smaller than `s[i]`, walking DOWNWARD from `d-1`.
///   The direction does not change the count, but the reference's bound `j > i` does: element
///   `i` itself is excluded.
/// - The accumulation is `k = pi + (i + 1) * k` — radix `i + 1` at step `i`, applied to the
///   running total, not to `pi`.
pub fn group_index(d: usize, s: &[usize]) -> GroupIndex {
    debug_assert!(d >= 4, "the table is only indexed for degree >= 4");
    let mut k = 0usize;
    if s[0] < s[2] {
        k += 1;
    }
    if s[1] < s[2] {
        k += 1;
    }
    for i in 3..=(d - 1) {
        let mut pi = s[i];
        for j in ((i + 1)..=(d - 1)).rev() {
            if s[j] < s[i] {
                pi -= 1;
            }
        }
        k = pi + (i + 1) * k;
    }

    // ⛔ Only HALF the permutations are stored. An index past the stored half is the mirror of
    // one inside it: reflect in x and read `2 * kNumGroup[d] - 1 - k`. Treating an out-of-range
    // index as an error would reject half of all nets.
    if k < K_NUM_GROUP[d] {
        GroupIndex { k, hflip: false }
    } else {
        GroupIndex { k: 2 * K_NUM_GROUP[d] - 1 - k, hflip: true }
    }
}

/// The per-step gap arrays the table's solutions are evaluated against.
///
/// `dd[1 ..= d-3]` are the vertical gaps and `dd[d-1+i]` the horizontal ones. ⚠️ Under a
/// horizontal flip the horizontal gaps are read **from the far end** — `xs[d-1-i] - xs[d-2-i]`
/// rather than `xs[i+1] - xs[i]` — which is the reflection actually being applied. The vertical
/// gaps are identical in both branches.
pub fn gaps(d: usize, xs: &[i32], ys: &[i32], hflip: bool) -> Vec<i32> {
    let mut dd = vec![0i32; 2 * super::lut::MAX_LUT_DEGREE - 2];
    for i in 1..=(d - 3) {
        dd[i] = ys[i + 1] - ys[i];
        dd[d - 1 + i] = if hflip {
            xs[d - 1 - i] - xs[d - 2 - i]
        } else {
            xs[i + 1] - xs[i]
        };
    }
    dd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_pivots_on_s2_for_both_comparisons() {
        // ⚠️ Not `s[0] < s[1]`. With d = 4 the loop body runs once (i = 3), and for the identity
        // permutation pi = s[3] minus nothing = 3, so k = 3 + 4*2 = 11.
        let gi = group_index(4, &[0, 1, 2, 3]);
        assert_eq!(gi.k, 2 * K_NUM_GROUP[4] - 1 - 11, "11 >= kNumGroup[4] = 6, so it mirrors");
        assert!(gi.hflip);
    }

    #[test]
    fn an_index_inside_the_stored_half_is_not_flipped() {
        // s[2] smallest -> neither seed comparison fires -> k starts at 0.
        let gi = group_index(4, &[1, 2, 0, 3]);
        assert!(!gi.hflip, "k stayed below kNumGroup[4]");
        assert!(gi.k < K_NUM_GROUP[4]);
    }

    #[test]
    fn every_permutation_of_degree_four_lands_in_range() {
        // ⭐ The real invariant: whatever the ordering, the index must address a group that
        // exists. Out of range would mean reading past the table.
        let mut seen = std::collections::BTreeSet::new();
        for p in permutations(4) {
            let gi = group_index(4, &p);
            assert!(gi.k < K_NUM_GROUP[4], "{p:?} -> k={} out of range", gi.k);
            seen.insert(gi.k);
        }
        assert_eq!(seen.len(), K_NUM_GROUP[4], "all 6 groups are reachable");
    }

    #[test]
    fn every_permutation_of_degrees_five_through_nine_lands_in_range() {
        for d in 5..=7 {
            let mut seen = std::collections::BTreeSet::new();
            for p in permutations(d) {
                let gi = group_index(d, &p);
                assert!(gi.k < K_NUM_GROUP[d], "d={d} {p:?} -> k={} out of range", gi.k);
                seen.insert(gi.k);
            }
            assert_eq!(seen.len(), K_NUM_GROUP[d], "d={d}: every group is reachable");
        }
    }

    #[test]
    fn the_flip_reads_horizontal_gaps_from_the_far_end() {
        let xs = [0, 1, 3, 6, 10];
        let ys = [0, 2, 5, 9, 14];
        let d = 5;
        let plain = gaps(d, &xs, &ys, false);
        let flipped = gaps(d, &xs, &ys, true);
        // Vertical gaps are the same either way.
        assert_eq!(plain[1..=(d - 3)], flipped[1..=(d - 3)]);
        // Horizontal gaps reverse: xs[i+1]-xs[i] vs xs[d-1-i]-xs[d-2-i].
        assert_eq!(plain[d - 1 + 1], xs[2] - xs[1]);
        assert_eq!(flipped[d - 1 + 1], xs[3] - xs[2]);
    }

    /// All permutations of 0..n, smallest-first.
    fn permutations(n: usize) -> Vec<Vec<usize>> {
        let mut out = Vec::new();
        let mut cur: Vec<usize> = (0..n).collect();
        loop {
            out.push(cur.clone());
            // next lexicographic permutation
            let Some(i) = (0..n.saturating_sub(1)).rev().find(|&i| cur[i] < cur[i + 1]) else {
                break;
            };
            let j = (i + 1..n).rev().find(|&j| cur[j] > cur[i]).unwrap();
            cur.swap(i, j);
            cur[i + 1..].reverse();
        }
        out
    }
}