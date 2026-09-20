// SPDX-License-Identifier: Apache-2.0
//! The FLUTE lookup table: its constants, its row type, and the two decoders that read it.
//!
//! 🔑 **Types are transcribed, not chosen.** Every field below is `unsigned char` in the
//! reference, so every field here is `u8`. Widening any of them would make our arithmetic more
//! accurate than the reference's, and more accurate is a defect in a reimplementation.

/// The LUT covers degrees up to this. The reference carries a `static_assert` on it.
pub const MAX_LUT_DEGREE: usize = 9;

/// Degrees are initialised this far at startup; 9 is reached on demand.
///
/// ⚠️ A SEQUENCE detail, not a tuning knob: the table is built to degree 8 eagerly and extended
/// to 9 only when a net of that degree arrives, so a degree-9 net is the first thing to touch
/// the extension path.
pub const LUT_INITIAL_DEGREE: usize = 8;

/// Number of groups per degree, indexed by degree 0..=9.
pub const K_NUM_GROUP: [usize; 10] = [0, 0, 0, 0, 6, 30, 180, 1260, 10080, 90720];

/// `get_max_powv(9)` in the reference.
pub const MAX_POWV: usize = 79;

/// True to construct the routing; false would estimate wirelength only.
pub const CONSTRUCT_ROUTING: bool = true;

/// One solution row of the table.
///
/// ⛔ **Two encodings here are implementation-defined and must be reproduced exactly:**
///
/// - **`rowcol` packs two nibbles**: the reference's own comment is
///   `row = rowcol[]/16, col = rowcol[]%16`. Storing them as separate fields would be a
///   different table.
/// - **`seg` is a sentinel-terminated TWO-part list**: `Add: 0..i, Sub: j..10`, with
///   `seg[i+1] = seg[j-1] = 0`. The front part is filled forward from 0 and the back part
///   BACKWARD from 10, and the two zeros are what separates them — not a length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Csoln {
    pub parent: u8,
    pub seg: [u8; 11],
    pub rowcol: [u8; MAX_LUT_DEGREE - 2],
    pub neighbor: [u8; 2 * MAX_LUT_DEGREE - 2],
}

impl Default for Csoln {
    fn default() -> Self {
        Csoln {
            parent: 0,
            seg: [0; 11],
            rowcol: [0; MAX_LUT_DEGREE - 2],
            neighbor: [0; 2 * MAX_LUT_DEGREE - 2],
        }
    }
}

impl Csoln {
    /// `row = rowcol[i] / 16` — the high nibble.
    pub fn row(&self, i: usize) -> u8 {
        self.rowcol[i] / 16
    }
    /// `col = rowcol[i] % 16` — the low nibble.
    pub fn col(&self, i: usize) -> u8 {
        self.rowcol[i] % 16
    }
}

/// The table's character→value decoder.
///
/// ```text
/// static unsigned char charNum(const unsigned char c) {
///   if (isdigit(c)) return c - '0';
///   if (c >= 'A')   return c - 'A' + 10;
///   return 0;
/// }
/// ```
///
/// ⚠️ **Three behaviours worth stating, because each is easy to "improve" away:**
///
/// 1. The `c >= 'A'` branch has **no upper bound**, so lowercase letters decode too — `'a'` is
///    `'a' - 'A' + 10` = 42, not an error and not 0.
/// 2. Anything below `'A'` that is not a digit returns **0**, and 0 is a meaningful value in this
///    table (it marks "same as some previous group"), so a malformed byte is indistinguishable
///    from a real zero. That is the reference's behaviour and we reproduce it rather than
///    validating.
/// 3. The arithmetic is `unsigned char` throughout and wraps; it never widens.
pub fn char_num(c: u8) -> u8 {
    if c.is_ascii_digit() {
        return c - b'0';
    }
    if c >= b'A' {
        // ⚠️ `wrapping_sub` then `wrapping_add`: `unsigned char` arithmetic in the reference
        // wraps, and a byte far above 'A' must land where the C++ lands.
        return c.wrapping_sub(b'A').wrapping_add(10);
    }
    0
}

/// The table's integer reader, returning the value and the number of bytes consumed.
///
/// ```text
/// value = 0;
/// negative = (*s == '-');
/// if (negative || *s == '+') ++s;
/// while (*s >= '0' && *s <= '9') { value = 10 * value + (int(*s) - '0'); ++s; }
/// if (negative) value = -value;
/// ```
///
/// ⚠️ **`value` is `int`**, and `10 * value + digit` is `int` arithmetic that OVERFLOWS rather
/// than widening. Computing in `i64` here would be more accurate than the reference and
/// therefore wrong; `wrapping_*` reproduces it.
///
/// ⚠️ A lone sign with no digits yields **0 and consumes the sign** — it is not an error.
pub fn read_decimal_int(s: &[u8]) -> (i32, usize) {
    let mut i = 0usize;
    let negative = s.first() == Some(&b'-');
    if negative || s.first() == Some(&b'+') {
        i += 1;
    }
    let mut value: i32 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        value = value
            .wrapping_mul(10)
            .wrapping_add(i32::from(s[i] - b'0'));
        i += 1;
    }
    if negative {
        value = value.wrapping_neg();
    }
    (value, i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_group_counts_are_the_references_table() {
        assert_eq!(K_NUM_GROUP, [0, 0, 0, 0, 6, 30, 180, 1260, 10080, 90720]);
        // The LUT is only defined from degree 4 up; below that the table is empty by
        // construction rather than by a guard.
        assert!(K_NUM_GROUP[..4].iter().all(|&n| n == 0));
        assert_eq!(K_NUM_GROUP[MAX_LUT_DEGREE], 90720, "9! / 4");
    }

    #[test]
    fn the_solution_row_has_the_references_exact_widths() {
        // ⛔ Sizes come from kMaxLutDegree, not from round numbers: rowcol is d-2 and neighbor
        // is 2d-2. Getting either wrong shifts every subsequent field when the table is parsed.
        let c = Csoln::default();
        assert_eq!(c.seg.len(), 11);
        assert_eq!(c.rowcol.len(), 7, "kMaxLutDegree - 2");
        assert_eq!(c.neighbor.len(), 16, "2 * kMaxLutDegree - 2");
    }

    #[test]
    fn rowcol_packs_two_nibbles() {
        // The reference's own comment: row = rowcol[]/16, col = rowcol[]%16.
        let mut c = Csoln::default();
        c.rowcol[0] = 0xAB;
        assert_eq!(c.row(0), 0xA);
        assert_eq!(c.col(0), 0xB);
        c.rowcol[1] = 15;
        assert_eq!(c.row(1), 0, "a value under 16 is all column");
        assert_eq!(c.col(1), 15);
    }

    #[test]
    fn char_num_decodes_digits_then_letters_then_zero() {
        assert_eq!(char_num(b'0'), 0);
        assert_eq!(char_num(b'9'), 9);
        assert_eq!(char_num(b'A'), 10);
        assert_eq!(char_num(b'Z'), 35);
        // ⚠️ No upper bound on the letter branch: lowercase decodes rather than erroring.
        assert_eq!(char_num(b'a'), 42, "'a' - 'A' + 10");
        // ⚠️ Below 'A' and not a digit is 0 — indistinguishable from a real zero, which in this
        // table means "same as some previous group".
        assert_eq!(char_num(b'\n'), 0);
        assert_eq!(char_num(b'#'), 0);
    }

    #[test]
    fn read_decimal_int_matches_the_reference_including_its_edges() {
        assert_eq!(read_decimal_int(b"123x"), (123, 3));
        assert_eq!(read_decimal_int(b"-45,"), (-45, 3));
        assert_eq!(read_decimal_int(b"+7"), (7, 2));
        assert_eq!(read_decimal_int(b"0"), (0, 1));
        // ⚠️ A lone sign is not an error: it yields 0 and consumes the sign.
        assert_eq!(read_decimal_int(b"-"), (0, 1));
        // A non-numeric head consumes nothing.
        assert_eq!(read_decimal_int(b"abc"), (0, 0));
    }

    #[test]
    fn read_decimal_int_wraps_like_int_rather_than_widening() {
        // ⛔ `value = 10 * value + digit` is INT arithmetic in the reference. Computing in i64
        // would be more accurate than the reference, and more accurate is a defect here.
        let (v, n) = read_decimal_int(b"99999999999");
        assert_eq!(n, 11, "every digit is consumed");
        let mut expect: i32 = 0;
        for d in b"99999999999" {
            expect = expect.wrapping_mul(10).wrapping_add(i32::from(d - b'0'));
        }
        assert_eq!(v, expect);
        assert_ne!(i64::from(v), 99_999_999_999i64, "it did NOT widen");
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// The load sequence
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// One degree's worth of solution groups.
///
/// ⛔ **Entries ALIAS.** The reference stores `std::shared_ptr<Csoln[]>` and, when a group's
/// solution count decodes as 0, assigns `(*lut)[d][k] = (*lut)[d][kk]` — the same buffer, not a
/// copy. `Rc` reproduces that; cloning the data would make the table larger than the reference's
/// and, more importantly, would hide that groups are deliberately shared.
pub type Group = std::rc::Rc<[Csoln]>;

/// The parsed table: `lut[d][k]` and `numsoln[d][k]`.
#[derive(Debug, Default)]
pub struct Lut {
    pub lut: Vec<Vec<Option<Group>>>,
    pub numsoln: Vec<Vec<u8>>,
    /// The highest degree initialised, as `lut_valid_d_`.
    pub valid_degree: usize,
}

/// Errors the reference does not have, because it reads a file it shipped and trusts it.
///
/// ⚠️ We surface a truncation rather than reading past the end. That is **not** a behavioural
/// divergence: on the real tables these cannot fire, and the reference would have had undefined
/// behaviour where we return an error.
#[derive(Debug, PartialEq, Eq)]
pub enum LutError {
    Truncated { degree: usize, group: usize },
    AliasOutOfRange { degree: usize, group: usize, target: usize },
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn next(&mut self) -> Option<u8> {
        let c = self.b.get(self.i).copied();
        if c.is_some() {
            self.i += 1;
        }
        c
    }
    fn peek2(&self) -> Option<(u8, u8)> {
        Some((*self.b.get(self.i)?, *self.b.get(self.i + 1)?))
    }
    fn read_int(&mut self) -> i32 {
        let (v, n) = read_decimal_int(&self.b[self.i.min(self.b.len())..]);
        self.i += n;
        v
    }
}

/// Parse the decoded `POWV9` (and, when routing is constructed, `POST9`) tables.
///
/// This is `Flute::initLUT` statement for statement. The sequence matters more than any single
/// rule here, so the order below is the reference's order and the deviations are named.
pub fn parse_lut(pwv: &[u8], prt: Option<&[u8]>, to_d: usize) -> Result<Lut, LutError> {
    let mut out = Lut {
        lut: vec![Vec::new(); MAX_LUT_DEGREE + 1],
        numsoln: vec![Vec::new(); MAX_LUT_DEGREE + 1],
        valid_degree: 0,
    };
    let mut p = Cursor { b: pwv, i: 0 };
    let mut r = prt.map(|b| Cursor { b, i: 0 });

    // ⛔ `d` is the loop variable AND an output of the file: the reference does
    // `readDecimalInt(pwv + 2, d)`, which WRITES INTO `d`. The degree the table is read at is
    // therefore whatever the file says, not what the loop counted — and `kNumGroup[d]` below is
    // indexed by the rewritten value. Transcribing this as a read-only loop counter would
    // silently read the wrong number of groups the moment the file disagreed.
    let mut d = 4usize;
    while d <= to_d {
        if p.peek2() == Some((b'd', b'=')) {
            p.i += 2;
            d = p.read_int().max(0) as usize;
        }
        p.i += 1; // the reference's bare `++pwv`
        if let Some(rc) = r.as_mut() {
            if rc.peek2() == Some((b'd', b'=')) {
                rc.i += 2;
                d = rc.read_int().max(0) as usize; // ⚠️ writes `d` a SECOND time
            }
            rc.i += 1;
        }
        if d >= K_NUM_GROUP.len() {
            break;
        }

        let groups = K_NUM_GROUP[d];
        out.lut[d] = vec![None; groups];
        out.numsoln[d] = vec![0; groups];

        for k in 0..groups {
            let ns = char_num(p.next().ok_or(LutError::Truncated { degree: d, group: k })?);
            if ns == 0 {
                // "same as some previous group" — and the `+ 1` after the integer is the
                // reference's own extra skip, not a delimiter we inferred.
                let kk = p.read_int();
                p.i += 1;
                let kk = usize::try_from(kk).map_err(|_| LutError::AliasOutOfRange {
                    degree: d, group: k, target: kk as usize })?;
                if kk >= groups {
                    return Err(LutError::AliasOutOfRange { degree: d, group: k, target: kk });
                }
                out.numsoln[d][k] = out.numsoln[d][kk];
                out.lut[d][k] = out.lut[d][kk].clone(); // Rc clone: SHARED, as shared_ptr is
            } else {
                p.i += 1; // '\n'
                out.numsoln[d][k] = ns;
                let mut solns: Vec<Csoln> = Vec::with_capacity(ns as usize);
                for _ in 0..ns {
                    let mut c = Csoln {
                        parent: char_num(
                            p.next().ok_or(LutError::Truncated { degree: d, group: k })?),
                        ..Csoln::default()
                    };

                    // `seg` FORWARD from 0, writing the terminating zero as well.
                    let mut ch;
                    let mut j = 0usize;
                    loop {
                        ch = p.next().ok_or(LutError::Truncated { degree: d, group: k })?;
                        let seg = char_num(ch);
                        if j < c.seg.len() {
                            c.seg[j] = seg;
                        }
                        j += 1;
                        if seg == 0 {
                            break;
                        }
                    }
                    // ...then BACKWARD from 10. The two zeros are what separates the halves.
                    if ch == b'\n' {
                        c.seg[10] = 0;
                    } else {
                        let mut j = 10usize;
                        loop {
                            ch = p.next().ok_or(LutError::Truncated { degree: d, group: k })?;
                            let seg = char_num(ch);
                            c.seg[j] = seg;
                            if seg == 0 {
                                break;
                            }
                            if j == 0 {
                                break;
                            }
                            j -= 1;
                        }
                    }

                    if let Some(rc) = r.as_mut() {
                        let nn = 2 * d - 2;
                        // ⚠️ `rowcol` takes j in d..nn, stored at `j - d` — that is d-2 entries,
                        // NOT nn of them.
                        for j in d..nn {
                            c.rowcol[j - d] = char_num(
                                rc.next().ok_or(LutError::Truncated { degree: d, group: k })?);
                        }
                        // `neighbor` is nn entries packed TWO PER BYTE, high nibble first.
                        let mut j = 0usize;
                        while j < nn {
                            let byte = rc.next()
                                .ok_or(LutError::Truncated { degree: d, group: k })?;
                            c.neighbor[j] = byte / 16;
                            c.neighbor[j + 1] = byte % 16;
                            j += 2;
                        }
                        rc.i += 1; // '\n'
                    }
                    solns.push(c);
                }
                out.lut[d][k] = Some(Group::from(solns));
            }
        }
        d += 1;
    }
    out.valid_degree = to_d;
    Ok(out)
}

#[cfg(test)]
mod load_tests {
    use super::*;

    /// Encode a value the way the table does: 0-9 as digits, 10+ as 'A'..
    fn enc(v: u8) -> u8 {
        if v < 10 { b'0' + v } else { b'A' + v - 10 }
    }

    /// One degree-4 group with `ns` solutions, each carrying `parent` and a one-element seg.
    fn group(parent: u8) -> Vec<u8> {
        let mut v = vec![enc(1), b'\n'];          // ns = 1, then the '\n' the reference skips
        v.push(enc(parent));                       // parent
        v.push(enc(3));                            // seg[0] = 3
        v.push(enc(0));                            // seg terminator -> forward half ends
        v.push(b'\n');                             // ch == '\n' -> seg[10] = 0, no backward half
        v
    }

    fn powv_for_degree_4(n_groups: usize) -> Vec<u8> {
        let mut v = b"d=4\n".to_vec();
        // ⚠️ `readDecimalInt` stops at the '\n', then the reference does a bare `++pwv` which
        // consumes it. The fixture must have exactly that one byte.
        for k in 0..n_groups {
            v.extend(group(k as u8 + 1));
        }
        v
    }

    #[test]
    fn a_degree_header_rewrites_the_loop_variable() {
        // ⛔ The reference does `readDecimalInt(pwv + 2, d)` — the FILE sets the degree. A table
        // whose header says 5 must be read as degree 5 even though the loop started at 4, and
        // kNumGroup[5] = 30 groups rather than kNumGroup[4] = 6.
        let mut v = b"d=5\n".to_vec();
        for k in 0..K_NUM_GROUP[5] {
            v.extend(group((k % 9) as u8 + 1));
        }
        let lut = parse_lut(&v, None, 5).expect("parses");
        assert_eq!(lut.lut[5].len(), 30, "degree 5 has 30 groups");
        assert!(lut.lut[4].is_empty(), "degree 4 was never populated — the file said 5");
    }

    #[test]
    fn every_group_of_degree_four_is_read() {
        let v = powv_for_degree_4(K_NUM_GROUP[4]);
        let lut = parse_lut(&v, None, 4).expect("parses");
        assert_eq!(lut.lut[4].len(), 6, "kNumGroup[4]");
        assert!(lut.lut[4].iter().all(|g| g.is_some()));
        assert_eq!(lut.numsoln[4], vec![1; 6]);
        assert_eq!(lut.lut[4][0].as_ref().unwrap()[0].parent, 1);
        assert_eq!(lut.lut[4][3].as_ref().unwrap()[0].parent, 4);
    }

    #[test]
    fn seg_is_filled_forward_and_the_terminator_is_stored() {
        let v = powv_for_degree_4(K_NUM_GROUP[4]);
        let lut = parse_lut(&v, None, 4).expect("parses");
        let c = lut.lut[4][0].as_ref().unwrap()[0];
        assert_eq!(c.seg[0], 3, "the value");
        assert_eq!(c.seg[1], 0, "the terminating zero IS written, not skipped");
        assert_eq!(c.seg[10], 0, "ch was '\\n', so the backward half is just seg[10] = 0");
    }

    #[test]
    fn a_zero_solution_count_ALIASES_an_earlier_group_rather_than_copying() {
        // ⛔ `(*lut)[d][k] = (*lut)[d][kk]` is a shared_ptr assignment. The groups are the SAME
        // buffer, and treating this as a copy would hide that the table deliberately shares.
        let mut v = b"d=4\n".to_vec();
        v.extend(group(7));                         // group 0, parent 7
        for _ in 1..K_NUM_GROUP[4] {
            v.push(enc(0));                          // ns = 0 -> "same as some previous group"
            v.extend(b"0");                          // kk = 0
            v.push(b'\n');                           // the reference's `+ 1` after the integer
        }
        let lut = parse_lut(&v, None, 4).expect("parses");
        assert_eq!(lut.numsoln[4], vec![1; 6], "the alias copies the COUNT too");
        let g0 = lut.lut[4][0].clone().unwrap();
        for k in 1..6 {
            let gk = lut.lut[4][k].clone().unwrap();
            assert_eq!(gk[0].parent, 7);
            assert!(Group::ptr_eq(&g0, &gk), "group {k} must be the SAME buffer, not a copy");
        }
    }

    #[test]
    fn an_alias_past_the_end_is_refused_rather_than_read() {
        let mut v = b"d=4\n".to_vec();
        v.push(enc(0));
        v.extend(b"99");
        v.push(b'\n');
        match parse_lut(&v, None, 4) {
            Err(LutError::AliasOutOfRange { degree, group, target }) => {
                assert_eq!((degree, group, target), (4, 0, 99));
            }
            other => panic!("expected AliasOutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn routing_fills_rowcol_as_d_minus_two_and_neighbor_two_per_byte() {
        // For d = 4: nn = 2d-2 = 6, so rowcol takes j in 4..6 -> TWO entries, and neighbor takes
        // 6 entries from THREE bytes, high nibble first.
        // ⚠️ Both tables must carry ALL kNumGroup[4] groups. A fixture with one group made the
        // parser report Truncated at group 1 — correctly: the file really was short.
        let v = powv_for_degree_4(K_NUM_GROUP[4]);
        let mut post = b"d=4\n".to_vec();
        for _ in 0..K_NUM_GROUP[4] {
            post.push(enc(5));                 // rowcol[0]
            post.push(enc(6));                 // rowcol[1]
            post.extend([0x12u8, 0x34, 0x56]); // neighbor: 1,2,3,4,5,6
            post.push(b'\n');
        }
        let lut = parse_lut(&v, Some(&post), 4).expect("parses");
        let c = lut.lut[4][0].as_ref().unwrap()[0];
        assert_eq!(&c.rowcol[..2], &[5, 6], "d - 2 = 2 entries, not nn of them");
        assert_eq!(&c.neighbor[..6], &[1, 2, 3, 4, 5, 6], "two per byte, high nibble first");
        assert_eq!(c.neighbor[6], 0, "nothing beyond nn is touched");
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// Loading the real tables
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// Where `build.rs` put the fetched tables. Kept for diagnostics; the bytes below are what the
/// engine actually reads.
pub const FLUTE_DIR: &str = env!("VYGES_STT_FLUTE_DIR");

/// The commit the tables were fetched from — see `flute-tables.yaml`.
pub const OPENROAD_PIN: &str = env!("VYGES_STT_OPENROAD_PIN");

/// ⛔ **EMBEDDED, not read from disk at run time, and that is a shipping requirement.**
/// `build.rs` fetches the tables into `OUT_DIR`, which exists only on the machine that built the
/// binary. Reading them from there at run time worked on a development box and would have failed
/// on every user's — the released tarball carries the binary, not a 9.4 MB data directory, and
/// the baked path would point at a CI runner's scratch space.
///
/// ⚠️ This is also what the reference does: `etc/file_to_string.py` encodes the same bytes into a
/// C++ string array that is compiled in. Embedding is the behaviour being reproduced, not an
/// optimisation.
///
/// ℹ️ It costs ~9.4 MB of binary. That is the price of a self-contained engine, and the
/// alternative — shipping the data beside it and resolving a path — has more ways to go wrong.
const POWV9: &[u8] = include_bytes!(concat!(env!("VYGES_STT_FLUTE_DIR"), "/POWV9.dat"));
const POST9: &[u8] = include_bytes!(concat!(env!("VYGES_STT_FLUTE_DIR"), "/POST9.dat"));

/// Parse the real `POWV9` / `POST9` tables.
///
/// 🔑 **No base64 step.** Upstream encodes these into a C++ string array with
/// `etc/file_to_string.py` and calls `utl::base64_decode` at runtime; that round trip is identity,
/// so the `.dat` bytes are exactly what the parser consumes. `POWV9.dat` begins `d=4`.
pub fn load_tables(to_d: usize) -> Result<Lut, LutError> {
    parse_lut(POWV9, if CONSTRUCT_ROUTING { Some(POST9) } else { None }, to_d)
}

#[cfg(test)]
mod real_table_tests {
    use super::*;

    #[test]
    fn the_real_tables_parse_to_every_group_of_every_degree() {
        // ⭐ The whole parser against 9.4 MB of real data. Every degree from 4 to 9 must come
        // back with exactly kNumGroup[d] groups, every one populated.
        let lut = load_tables(MAX_LUT_DEGREE).expect("the shipped tables parse");
        for d in 4..=MAX_LUT_DEGREE {
            assert_eq!(lut.lut[d].len(), K_NUM_GROUP[d], "degree {d} group count");
            let missing = lut.lut[d].iter().filter(|g| g.is_none()).count();
            assert_eq!(missing, 0, "degree {d} has {missing} unpopulated groups");
        }
    }

    #[test]
    fn every_solution_count_is_within_the_references_bound() {
        // numsoln is read with charNum, so it cannot exceed what one character encodes, and the
        // reference sizes its arrays on kMaxPowv.
        let lut = load_tables(MAX_LUT_DEGREE).expect("parses");
        for d in 4..=MAX_LUT_DEGREE {
            for (k, &ns) in lut.numsoln[d].iter().enumerate() {
                assert!(ns as usize <= MAX_POWV, "degree {d} group {k}: ns={ns} > {MAX_POWV}");
                assert!(ns > 0, "degree {d} group {k}: every group resolves to a solution set");
                assert_eq!(lut.lut[d][k].as_ref().unwrap().len(), ns as usize,
                           "degree {d} group {k}: numsoln must match the buffer length");
            }
        }
    }

    #[test]
    fn groups_really_do_alias_in_the_shipped_tables() {
        // ⛔ Proves the aliasing path is EXERCISED by real data, not just by a fixture. If the
        // shipped tables never used `ns == 0`, the sharing logic would be untested in practice.
        let lut = load_tables(MAX_LUT_DEGREE).expect("parses");
        let mut shared = 0usize;
        for d in 4..=MAX_LUT_DEGREE {
            for k in 1..lut.lut[d].len() {
                let gk = lut.lut[d][k].as_ref().unwrap();
                if lut.lut[d][..k].iter().any(|g| Group::ptr_eq(g.as_ref().unwrap(), gk)) {
                    shared += 1;
                }
            }
        }
        assert!(shared > 0, "the shipped tables DO reuse groups; found {shared}");
        println!("shared groups in the shipped tables: {shared}");
    }
}