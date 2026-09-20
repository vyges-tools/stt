// SPDX-License-Identifier: Apache-2.0
//! **The gating test.** One test that drives the whole engine — both builders, the gate between
//! them and the printer — against a golden captured from OpenROAD itself, and fails if any of it
//! drifts.
//!
//! 🔑 **The corpus is the single source.** `examples/stt_gate/corpus.json` is what this test
//! reads and what the golden beside it was captured from, so there is no second spelling of the
//! case to drift against.
//!
//! ⛔ **Upstream ships no single end-to-end case for `stt`** — its suite is per-feature
//! (`flute1`, `pd1`, `pd2`, `check`, `parse_clocks`, and two sweeps over `gcd.nets`). ⚠️ `check`
//! is the one case that calls `makeSteinerTree`, with a single 11-pin net at one alpha, so the
//! alpha branch has a witness on ONE side, `checkTree` is never observed rejecting anything, and
//! the FLUTE fallback after a failed check has none at all. The two builders can be wired to each
//! other wrongly and every shipped case still passes. That gap is why this exists.
//!
//! 🔑 **It needs no box, no container and no network.** `examples/stt_gate/stt_gate.ok` was produced by
//! the oracle at our pin and committed, so this runs anywhere `cargo test` runs.
//!
//! What the corpus covers:
//!
//! | cases | what they reach |
//! | --- | --- |
//! | `deg0`–`deg3` | the degenerate returns, before any table is consulted |
//! | `lut4`–`lut9` | every lookup-table degree, both parities of the group index |
//! | `med10`–`med36` | the decomposition, the scoring heuristic, and all three merges |
//! | `collinear_*`, `grid` | ties in x and in y, where the tie-breaks decide the answer |
//! | `pd_alpha_*` | the alpha branch: 0 is FLUTE, every positive value is Prim-Dijkstra |
//! | `pd_drvr_*` | the driver as a NODE ID, not a coordinate lookup |
//! | `pd_dup_*`, `pd_star8` | duplicate pins, and a centre that must be split below degree 4 |
//! | `pd_check_fallback` | a PD tree that FAILS `check_tree`, so FLUTE rebuilds the net |
//! | `pd_fanout_120` | above the degree-100 bound, where the check is skipped outright |
//! | `gap_deg*` | degrees 2 and 9 through the façade, which no shipped case reaches |

// ⚠️ Test names carry the rule; the capitalised word is the part that must not be missed when a
// gate goes red and the failure line is the first thing read.
#![allow(non_snake_case)]

use std::collections::BTreeMap;

const CORPUS: &str = include_str!("../examples/stt_gate/corpus.json");
const GOLDEN: &str = include_str!("../examples/stt_gate/stt_gate.ok");

#[derive(Debug, PartialEq, Eq)]
struct Golden {
    summary: String,
    branches: Vec<String>,
}

/// Split `reportSteinerTree` output into one entry per net.
fn parse_golden(text: &str) -> BTreeMap<String, Golden> {
    let mut out = BTreeMap::new();
    let mut cur: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Net ") {
            cur = Some(rest.trim().to_string());
            out.insert(
                rest.trim().to_string(),
                Golden { summary: String::new(), branches: Vec::new() },
            );
            continue;
        }
        let Some(name) = cur.as_ref() else { continue };
        let e = out.get_mut(name).unwrap();
        if line.starts_with("Wire length") {
            e.summary = line.to_string();
        } else if !line.is_empty() {
            e.branches.push(line.to_string());
        }
    }
    out
}

struct Net {
    name: String,
    x: Vec<i32>,
    y: Vec<i32>,
    drvr: usize,
    alpha: f32,
    why: String,
}

fn corpus() -> Vec<Net> {
    let v: serde_json::Value = serde_json::from_str(CORPUS).expect("corpus.json");
    v.as_array()
        .expect("an array of nets")
        .iter()
        .map(|n| {
            let num = |k: &str| -> Vec<i32> {
                n[k].as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect()
            };
            Net {
                name: n["name"].as_str().unwrap().to_string(),
                x: num("x"),
                y: num("y"),
                drvr: n["drvr"].as_u64().unwrap() as usize,
                alpha: n["alpha"].as_f64().unwrap() as f32,
                why: n["why"].as_str().unwrap().to_string(),
            }
        })
        .collect()
}

fn lut() -> vyges_stt::flute::lut::Lut {
    vyges_stt::flute::lut::load_tables(vyges_stt::flute::MAX_LUT_DEGREE)
        .expect("the FLUTE tables load")
}

#[test]
fn every_net_in_the_corpus_reproduces_the_reference_LINE_FOR_LINE() {
    let golden = parse_golden(GOLDEN);
    let lut = lut();
    let mut scored = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for net in corpus() {
        let Some(want) = golden.get(&net.name) else {
            // ⛔ Only two shapes legitimately have no golden, and both are checked below by
            // [`a_net_under_two_pins_through_FLUTE_is_the_references_own_STT_0007`]: a net with
            // no pins, which `report_stt_net` cannot express, and a ONE-pin net through FLUTE,
            // which aborts the reference's case. Anything else missing is a corpus that outgrew
            // its golden.
            assert!(net.x.len() < 2 && net.alpha <= 0.0,
                    "{}: missing from the golden but has {} pins at alpha {}",
                    net.name, net.x.len(), net.alpha);
            continue;
        };

        let (tree, _why) =
            vyges_stt::make_steiner_tree(&lut, &net.x, &net.y, net.drvr, net.alpha);
        let Some(tree) = tree else {
            failures.push(format!("{}: engine REFUSED a net the reference answered", net.name));
            continue;
        };
        let Some(lines) =
            vyges_stt::report_steiner_tree(&tree, net.x[net.drvr], net.y[net.drvr])
        else {
            failures.push(format!("{}: no branch at the driver's location (STT-0007)", net.name));
            continue;
        };
        scored += 1;

        if lines[0] != want.summary {
            failures.push(format!("{}: {:?} != reference {:?}", net.name, lines[0], want.summary));
            continue;
        }
        if lines[1..] != want.branches[..] {
            let first = lines[1..].iter().zip(&want.branches).find(|(a, b)| a != b);
            failures.push(format!("{}: branches differ, first at {first:?}", net.name));
        }
    }

    assert!(scored >= 35, "the corpus shrank — only {scored} nets were scored");
    assert!(failures.is_empty(), "{} net(s) differ:\n  {}", failures.len(), failures.join("\n  "));
}

#[test]
fn each_net_reaches_the_BUILDER_the_corpus_says_it_should() {
    // 🔑 Matching the reference's output is necessary but not sufficient: two builders can agree
    // on a tree. This pins WHICH one produced it, which is the only thing that fails when the
    // alpha branch or the `check_tree` fallback is wired wrongly.
    use vyges_stt::PdOutcome;
    let lut = lut();
    for net in corpus() {
        if net.x.is_empty() {
            continue;
        }
        let (_t, why) = vyges_stt::make_steiner_tree(&lut, &net.x, &net.y, net.drvr, net.alpha);
        let want = match net.why.as_str() {
            "flute" => PdOutcome::Flute,
            "pd" => PdOutcome::PrimDijkstra,
            "fallback" => PdOutcome::FellBackCheckFailed,
            other => panic!("{}: unknown `why` {other:?}", net.name),
        };
        assert_eq!(why, want, "{} took the wrong path", net.name);
    }
}

#[test]
fn a_net_under_two_pins_through_FLUTE_is_the_references_own_STT_0007() {
    // ⛔ Not a limitation of ours. `Flute::flute` returns `deg = 1` with an EMPTY branch vector,
    // so `reportSteinerTree`'s `findLocationIndex` finds no branch at the driver's location and
    // the reference raises `STT-0007 Invalid driver index -1`, aborting the case. Verified by
    // running the unmodified reference at the pin on exactly this net.
    //
    // ⚠️ Prim-Dijkstra has no such problem — upstream's own `pd1` reports a one-pin `one` net —
    // which is why `pd_one_pin` is in the corpus at alpha > 0 and `deg1` is not.
    let lut = lut();
    let (tree, _) = vyges_stt::make_steiner_tree(&lut, &[7], &[3], 0, 0.0);
    let tree = tree.expect("flute answers, it is the REPORT that cannot");
    assert_eq!((tree.deg, tree.branch.len()), (1, 0));
    assert!(vyges_stt::report_steiner_tree(&tree, 7, 3).is_none(), "STT-0007");

    let (tree, _) = vyges_stt::make_steiner_tree(&lut, &[10], &[10], 0, 0.4);
    let tree = tree.expect("Prim-Dijkstra builds a one-pin tree");
    assert_eq!(
        vyges_stt::report_steiner_tree(&tree, 10, 10).unwrap(),
        vec!["Wire length = 0 Path depth = 0", "0 (10 10) neighbor 0 length 0"]
    );
}

#[test]
fn the_degenerate_returns_are_exactly_the_references_initialisers() {
    let lut = lut();
    // ⚠️ Below two pins: degree 1, no branches at all.
    let t = vyges_stt::flute::flute(&lut, &[7], &[3]).unwrap();
    assert_eq!((t.deg, t.length, t.branch.len()), (1, 0, 0));
    // ⚠️ Two pins: BOTH branches point at index 1 — the root self-references.
    let t = vyges_stt::flute::flute(&lut, &[0, 10], &[0, 4]).unwrap();
    assert_eq!((t.deg, t.length), (2, 14));
    assert_eq!((t.branch[0].n, t.branch[1].n), (1, 1));
}

#[test]
fn every_tree_the_engine_builds_passes_its_own_self_overlap_check() {
    // 🔑 Ties the two halves together. FLUTE's output must satisfy the predicate that gates
    // Prim-Dijkstra, and a PD tree that reaches the caller must satisfy it by construction —
    // the one net that does not is `pd_check_fallback`, and it is answered by FLUTE.
    let lut = lut();
    for net in corpus() {
        if net.x.is_empty() {
            continue;
        }
        let (Some(t), _) = vyges_stt::make_steiner_tree(&lut, &net.x, &net.y, net.drvr, net.alpha)
        else {
            panic!("{}: no tree", net.name)
        };
        assert!(vyges_stt::check_tree(&t), "{}: returned tree fails check_tree", net.name);
    }
}

#[test]
fn every_tree_is_structurally_a_tree() {
    // ⛔ The invariant that caught two truncated merges: exactly ONE self-referencing root, and
    // every neighbour index in range. A miswired tree keeps both indices valid, so nothing else
    // would notice.
    let lut = lut();
    for net in corpus() {
        if net.x.is_empty() {
            continue;
        }
        let (Some(t), _) = vyges_stt::make_steiner_tree(&lut, &net.x, &net.y, net.drvr, net.alpha)
        else {
            continue;
        };
        if t.branch.is_empty() {
            continue;
        }
        assert!(t.branch.iter().all(|b| b.n < t.branch.len()), "{}: index out of range", net.name);
        let roots = (0..t.branch.len()).filter(|&i| t.branch[i].n == i).count();
        assert_eq!(roots, 1, "{}: expected exactly one root, found {roots}", net.name);
    }
}