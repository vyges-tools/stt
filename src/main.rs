// SPDX-License-Identifier: Apache-2.0
//! `vyges-stt` — rectilinear Steiner trees over a net's pin locations.
//!
//! ## Exit codes
//!
//! ⛔ **"found something" and "could not run" never share a code.** A CI gate that cannot tell a
//! broken tool from a bad design will eventually read a missing engine as a clean run.
//!
//! | code | meaning |
//! | --- | --- |
//! | 0 | ok — the check passed, or the tree was built |
//! | 1 | the tree FAILED the self-overlap check (a finding, not an error) |
//! | 2 | usage, or input that could not be read |
//! | 3 | the requested builder is NOT IMPLEMENTED in this build |

// ⚠️ Test names carry the rule; the capitalised word is the part that must not be missed.
#![allow(non_snake_case)]

use std::process::ExitCode;

use vyges_stt::{check_tree, Branch, Tree};

const USAGE: &str = "\
vyges loom stt — rectilinear Steiner minimal trees over a net's pin locations

USAGE:
  vyges loom stt run   <job.json>   [-o FILE] [--json]
  vyges loom stt batch <nets.json>  [-o FILE] [--json]
  vyges loom stt check <tree.json>  [-o FILE] [--json]
  vyges loom stt --describe
  vyges loom stt --help
  vyges loom stt --version

JOB FIELDS (`run`; `batch` takes an array of these, each with a `name`):
  x, y      required, equal length — the pin locations, in database units
  drvr      driver PIN INDEX (default 0) — an index into x/y, never a coordinate
  alpha     > 0 selects Prim-Dijkstra; 0 (the default) selects FLUTE

OPTIONS:
  -o FILE               write the report to FILE instead of stdout
  --json                emit JSON instead of the reference's printed tree
  -q, --quiet           raise the causal trail's floor to `error` (VYGES_LOG)
  -v, --verbose         lower it to `debug` — adds the per-net trail in `batch`
  --describe            print a machine-readable JSON description of the command
  --bug-report          file a bug (central: vyges/community)
  --feature-request     request a feature (central)
  --sponsor             sponsor Vyges (github.com/sponsors/vyges-ip)
  --star                star this tool on GitHub

EXIT STATUS:
  0  ok           a tree was built, or the check passed
  1  failed       the tree FAILED the self-overlap check — a finding, not an error
  2  vacuous      nothing was built: a net with no pins, or a batch of none. NOT a pass.
  2  error        usage, unreadable input, or a driver index that is not a pin
  3  refused      the engine declined to answer — see stderr for which path declined
";

/// ⚠️ **The pin is taken from `flute-tables.yaml`** at build time, not written here. The
/// correlation harness reads this field to refuse a binary built against a different reference,
/// so a hand-copied pin left behind at a bump would make it certify the wrong one.
fn describe() -> String {
    DESCRIBE.replace(PIN_TOKEN, env!("VYGES_STT_OPENROAD_PIN"))
}

const PIN_TOKEN: &str = "@OPENROAD_PIN@";

/// ⚠️ **`maturity` is `workflow-validated`, and that is a claim with a test behind it.** The
/// level requires a pinned fixture the suite runs end to end and asserts against; ours is
/// `examples/stt_gate/stt_gate.ok`, 41 nets captured from the reference at the pin, which
/// `tests/end_to_end.rs` compares line for line on every `cargo test`.
const DESCRIBE: &str = r#"{
  "schema": "vyges-tool-descriptor/1.1",
  "openroad_pin": "@OPENROAD_PIN@",
  "name": "stt",
  "summary": "rectilinear Steiner minimal trees over a net's pin locations (FLUTE and Prim-Dijkstra)",
  "maturity": "workflow-validated",
  "provenance_limitations": [
      "input_hash covers the argument vector, not the content of the job file it names.",
      "status is one of ok, failed, vacuous, refused or error. VACUOUS IS NOT OK: it means no tree was built because the input carried no pins, and the declared assertion passes only on ok, so a vacuous run fails it rather than signing off a net nothing was built for. Exit status is 0 for ok, 1 for a failed self-overlap check, 2 for vacuous and for error, 3 for refused.",
      "Correlated against OpenROAD at the pin above, 2026-09-20, over every tree-producing case in src/stt/test plus a 41-net gate of our own: 341 of 341 trees byte-identical, comparing the wire length, the path depth and every branch line of reportSteinerTree. That covers check, flute1, flute_gcd, pd1, pd2 and pd_gcd. A NUMBER HERE MEANS NOTHING WITHOUT THE BUILD: the reference's own answer moves between OpenROAD builds, so the pin is part of the claim.",
      "Two transcription details are UNWITNESSED: the f32 weight arithmetic in the Prim-Dijkstra search, and top_child_index taking the first maximum. Both are transcribed from the reference and neither changes any of the 341 trees, so no input we have distinguishes them from the alternatives.",
      "alpha > 0 selects Prim-Dijkstra, strictly. alpha = 0 never reaches it, which is the reference's own branch and not a simplification. A Prim-Dijkstra tree that fails the self-overlap check is DISCARDED and FLUTE rebuilds the net; the JSON reports which builder answered, which the reference does not.",
      "A one-pin net at alpha 0 has no reportable tree: Flute::flute returns degree 1 with an empty branch vector, so there is no branch at the driver's location. The reference raises STT-0007 and aborts on exactly this input; we report it rather than inventing a branch.",
      "Path depth is the longest root-to-leaf distance from the branch at the driver's COORDINATES, not from the pin index passed in. That is the reference's findLocationIndex, which exists because flute reorders the points.",
      "The FLUTE lookup tables are the reference's own POWV9/POST9 data, fetched at build time from the pinned commit rather than vendored. Degrees above 9 go through the decomposition and the scoring heuristic, not the table."
  ],
  "invocation": {
    "args_template": ["run", "{job}"],
    "optional": [ { "arg": "out", "flag": "-o" } ],
    "emits_json": true
  },
  "inputs": {
    "type": "object",
    "required": ["job"],
    "properties": {
      "job": { "type": "string", "description": "path to a JSON job: {x, y, drvr, alpha}" },
      "out": { "type": "string", "description": "write the report to FILE instead of stdout" }
    }
  },
  "consumes": ["job"],
  "artifacts": [ { "role": "steiner_tree", "field": "report_path" } ],
  "assertion": {
    "id": "steiner-tree-built",
    "field": "status",
    "pass_when": { "eq": "ok" }
  }
}"#;

fn read_tree(path: &str) -> Result<Tree, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    let arr = v.get("branch").and_then(|b| b.as_array())
        .ok_or_else(|| format!("{path}: no `branch` array"))?;
    let branch = arr.iter().map(|b| {
        Ok(Branch {
            x: b.get("x").and_then(|n| n.as_i64()).ok_or("branch missing x")? as i32,
            y: b.get("y").and_then(|n| n.as_i64()).ok_or("branch missing y")? as i32,
            n: b.get("n").and_then(|n| n.as_u64()).ok_or("branch missing n")? as usize,
        })
    }).collect::<Result<Vec<_>, String>>()?;
    // ⚠️ `deg` is the TERMINAL count and is NOT derivable from the branch count — a tree with
    // Steiner points has more branches than terminals. Refuse rather than guess: the degree
    // decides whether the check runs at all (> 100 skips it).
    let deg = v.get("deg").and_then(|d| d.as_u64())
        .ok_or_else(|| format!("{path}: no `deg` — it is the terminal count and cannot be \
                                inferred from branch count"))? as usize;
    if branch.iter().any(|b| b.n >= branch.len()) {
        return Err(format!("{path}: a branch neighbour index is out of range"));
    }
    let length = v.get("length").and_then(|l| l.as_i64()).unwrap_or(0);
    Ok(Tree { deg, length, branch })
}

/// A job file: `{"x": [...], "y": [...], "drvr": 0, "alpha": 0.0}`.
///
/// ⚠️ `drvr` is a PIN INDEX into `x`/`y`, matching `primDijkstra`'s `driver_index`. ⚠️ `alpha`
/// defaults to 0, which is the value that routes a net to FLUTE — the same default the reference
/// would reach with no alpha set on the net.
fn read_job(path: &str) -> Result<(Vec<i32>, Vec<i32>, usize, f32), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    let get = |k: &str| -> Result<Vec<i32>, String> {
        v.get(k).and_then(|a| a.as_array())
            .ok_or_else(|| format!("{path}: no `{k}` array"))?
            .iter()
            .map(|n| n.as_i64().map(|n| n as i32).ok_or_else(|| format!("{path}: {k} not an int")))
            .collect()
    };
    let (x, y) = (get("x")?, get("y")?);
    // ⚠️ The reference errors STT-8 on this rather than truncating to the shorter.
    if x.len() != y.len() {
        return Err(format!("{path}: x size ({}) != y size ({})", x.len(), y.len()));
    }
    let drvr = v.get("drvr").and_then(|d| d.as_u64()).unwrap_or(0) as usize;
    if !x.is_empty() && drvr >= x.len() {
        return Err(format!("{path}: drvr {drvr} is not a pin index (0..{})", x.len()));
    }
    let alpha = v.get("alpha").and_then(|a| a.as_f64()).unwrap_or(0.0) as f32;
    Ok((x, y, drvr, alpha))
}

/// The builder that answered, for `--json`. ⛔ A fallback is reported as such, never as FLUTE:
/// the two reach the same function but mean different things about the net.
fn builder_name(why: &vyges_stt::PdOutcome) -> &'static str {
    match why {
        vyges_stt::PdOutcome::Flute => "flute",
        vyges_stt::PdOutcome::PrimDijkstra => "prim-dijkstra",
        vyges_stt::PdOutcome::FellBackCheckFailed => "flute-after-pd-check-failed",
        vyges_stt::PdOutcome::Error(_) => "error",
    }
}

/// The status word this run settles on, in ONE place.
///
/// ⛔ **`vacuous` is reserved across every engine and it is NOT a pass.** A pass word asserts that
/// work was done; a run that built nothing must never emit one. The descriptor's assertion is a
/// single equality on `ok`, so `vacuous` fails it automatically — which is the whole point: an
/// engine that can report its pass word having done nothing hands a machine a pass on a net it
/// never looked at.
///
/// ⚠️ Settled by a pure function so the branch cannot be forgotten at one call site, and so the
/// vocabulary is testable without a job file.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Status {
    /// A tree was built, or the check passed.
    Ok,
    /// The tree FAILED the self-overlap check — a finding, not an error.
    Failed,
    /// Nothing was built: no pins, or a batch with no nets.
    Vacuous,
    /// The engine declined to answer.
    Refused,
}

impl Status {
    fn word(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Failed => "failed",
            Status::Vacuous => "vacuous",
            Status::Refused => "refused",
        }
    }

    fn code(self) -> ExitCode {
        match self {
            Status::Ok => ExitCode::SUCCESS,
            Status::Failed => ExitCode::from(1),
            // ⛔ Deliberately NOT 0. Vacuous shares its code with `error` because both mean
            // "no verdict here", and the one thing neither may do is read as success.
            Status::Vacuous => ExitCode::from(2),
            Status::Refused => ExitCode::from(3),
        }
    }
}

/// `settle_status` for a build: how many trees were asked for, and how many came back.
///
/// ⚠️ **Zero requested is VACUOUS, zero answered out of some requested is REFUSED.** They are
/// different failures — an empty input file and an engine that declined — and collapsing them
/// loses which one happened.
fn settle_build(requested: usize, built: usize) -> Status {
    if requested == 0 {
        Status::Vacuous
    } else if built == 0 {
        Status::Refused
    } else {
        Status::Ok
    }
}

/// Where the report goes: `-o FILE`, or stdout.
fn emit(out: Option<&str>, text: &str) -> Result<(), String> {
    match out {
        Some(path) => std::fs::write(path, text).map_err(|e| format!("{path}: {e}")),
        None => {
            print!("{text}");
            Ok(())
        }
    }
}

/// Community links, the same four every shipped engine carries.
///
/// ⛔ **Bugs and features go to the CENTRAL `vyges/community` tracker, not to this repo's
/// issues.** One queue for the whole suite is where they get triaged; a per-engine tracker
/// scatters them across twenty repos and invites a reporter to paste context into whichever one
/// they happened to be using.
fn link(flag: &str) -> Option<(&'static str, &'static str)> {
    Some(match flag {
        "--bug-report" => (
            "Report a bug",
            "https://github.com/vyges/community/issues/new?template=bug_report_template.yaml",
        ),
        "--feature-request" => (
            "Request a feature",
            "https://github.com/vyges/community/issues/new?labels=enhancement",
        ),
        "--sponsor" => ("Sponsor Vyges", "https://github.com/sponsors/vyges-ip"),
        "--star" => ("Star this tool", "https://github.com/vyges-tools/stt"),
        _ => return None,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    // ⛔ Both flags SET SOMETHING. `vyges-events` reads its severity floor from `VYGES_LOG`
    // once, so the flags have to land there before the first event — a `-v` that is parsed and
    // then ignored is the same failure mode as an engine option that never arrives: it does not
    // error, it silently does nothing, and nobody notices for a fortnight.
    //
    // ⚠️ An explicit `VYGES_LOG` in the environment WINS. The caller who set it meant it, and a
    // flag default should not override a deliberate setting.
    let quiet = args.iter().any(|a| a == "-q" || a == "--quiet");
    let verbose = args.iter().any(|a| a == "-v" || a == "--verbose");
    if std::env::var_os("VYGES_LOG").is_none() {
        if quiet {
            std::env::set_var("VYGES_LOG", "error");
        } else if verbose {
            std::env::set_var("VYGES_LOG", "debug");
        }
    }

    // `-o FILE` — the value is consumed here so it never reaches the positional scan.
    let mut out_path: Option<String> = None;
    {
        let mut i = 0;
        while i < args.len() {
            if args[i] == "-o" {
                match args.get(i + 1) {
                    Some(v) => out_path = Some(v.clone()),
                    None => {
                        eprintln!("vyges-stt: -o needs a FILE");
                        return ExitCode::from(2);
                    }
                }
                i += 1;
            }
            i += 1;
        }
    }
    let skip: std::collections::HashSet<&str> = out_path
        .as_deref()
        .into_iter()
        .collect();
    let positional: Vec<&String> = args
        .iter()
        .filter(|a| !a.starts_with('-') && !skip.contains(a.as_str()))
        .collect();

    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some("-V") | Some("--version") => {
            // ⚠️ Version AND the commit it was built from: a released binary and a local build
            // of the same version are different artifacts, and a correlation run has to say which.
            println!(
                "vyges-stt {} ({})\nCopyright (c) Vyges. Apache-2.0.",
                env!("CARGO_PKG_VERSION"),
                env!("VYGES_STT_GIT_SHA")
            );
            return ExitCode::SUCCESS;
        }
        Some("--describe") => {
            println!("{}", describe());
            return ExitCode::SUCCESS;
        }
        Some(f) if link(f).is_some() => {
            let (label, url) = link(f).unwrap();
            println!("{label}:\n  {url}");
            // ⚠️ Only on a terminal. Launching a browser out of a pipeline would be a surprise.
            if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
                let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
                let _ = std::process::Command::new(opener).arg(url).status();
            }
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    let out = out_path.as_deref();
    // ⚠️ The single-net verbs feed the same census as `batch`, so the trail cannot say
    // "0 fell back" one line after emitting STT-FELLBACK.
    let mut pd_count = 0usize;
    let mut fallback_count = 0usize;
    let status = match positional.first().map(|s| s.as_str()) {
        Some("check") => {
            let Some(path) = positional.get(1) else {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            };
            let tree = match read_tree(path) {
                Ok(t) => t,
                Err(e) => { eprintln!("stt: {e}"); return ExitCode::from(2); }
            };
            // ⛔ A tree with no branches consulted no rule. Reporting `ok` there would sign off
            // a check that never ran.
            if tree.branch.is_empty() {
                let text = render_check(json, Status::Vacuous, &tree, &[]);
                if let Err(e) = emit(out, &text) { eprintln!("stt: {e}"); return ExitCode::from(2); }
                Status::Vacuous
            } else {
                let ok = check_tree(&tree);
                let st = if ok { Status::Ok } else { Status::Failed };
                let lines = tree.print_lines();
                let text = render_check(json, st, &tree, &lines);
                if let Err(e) = emit(out, &text) { eprintln!("stt: {e}"); return ExitCode::from(2); }
                st
            }
        }
        Some("run") => {
            let Some(path) = positional.get(1) else {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            };
            let (x, y, drvr, alpha) = match read_job(path) {
                Ok(p) => p,
                Err(e) => { eprintln!("stt: {e}"); return ExitCode::from(2); }
            };
            if x.is_empty() {
                let text = render_run(json, Status::Vacuous, None, "", &[]);
                if let Err(e) = emit(out, &text) { eprintln!("stt: {e}"); return ExitCode::from(2); }
                Status::Vacuous
            } else {
                let lut = match load_lut() {
                    Ok(l) => l,
                    Err(e) => { eprintln!("stt: {e}"); return ExitCode::from(2); }
                };
                let (tree, why) = vyges_stt::make_steiner_tree(&lut, &x, &y, drvr, alpha);
                match tree {
                    // ⚠️ `reportSteinerTree` locates the driver by COORDINATE, not by the index
                    // passed in — flute reorders the points and pdrev moves the driver to 0.
                    // Reproduced rather than short-circuited.
                    Some(t) => match vyges_stt::report_steiner_tree(&t, x[drvr], y[drvr]) {
                        Some(lines) => {
                            match why {
                                vyges_stt::PdOutcome::PrimDijkstra => pd_count += 1,
                                vyges_stt::PdOutcome::FellBackCheckFailed => {
                                    fallback_count += 1;
                                    vyges_stt::events::fell_back(path);
                                }
                                _ => {}
                            }
                            vyges_stt::events::built(path, builder_name(&why), t.deg, t.length);
                            let text =
                                render_run(json, Status::Ok, Some(&t), builder_name(&why), &lines);
                            if let Err(e) = emit(out, &text) {
                                eprintln!("stt: {e}");
                                return ExitCode::from(2);
                            }
                            Status::Ok
                        }
                        None => {
                            // ⛔ The reference's own STT-0007, reproduced. `Flute::flute` returns
                            // degree 1 with an EMPTY branch vector, so a one-pin net at alpha 0
                            // has no branch at the driver's location and upstream aborts here too.
                            vyges_stt::events::refused(
                                path,
                                &format!(
                                    "no branch at the driver's location ({}, {}) — STT-0007. \
                                     A one-pin net through FLUTE has this shape, and the \
                                     reference aborts on it as well.",
                                    x[drvr], y[drvr]
                                ),
                            );
                            Status::Refused
                        }
                    },
                    None => {
                        // ⚠️ Do NOT attribute this to the degree. Medium-degree IS implemented; a
                        // refusal here means the decomposition and heuristic both declined, or
                        // the merged tree contained a parent cycle — a known defect of ours.
                        // ⚠️ Do NOT attribute this to the degree in the message either.
                        vyges_stt::events::refused(
                            path,
                            &format!(
                                "degree {} — the medium-degree path declined (decomposition \
                                 and heuristic both, or a cycle in the merge)",
                                x.len()
                            ),
                        );
                        Status::Refused
                    }
                }
            }
        }
        Some("batch") => {
            // ⛔ The LUT is 9.4 MB and `run` reparses it EVERY invocation, where the reference
            // builds it once and keeps it (`ensureLUT`). Scoring 145 nets one process at a time
            // therefore paid for 145 table loads. This verb loads once and answers many.
            let Some(path) = positional.get(1) else {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            };
            let nets = match read_nets(path) {
                Ok(n) => n,
                Err(e) => { eprintln!("stt: {e}"); return ExitCode::from(2); }
            };
            let lut = match load_lut() {
                Ok(l) => l,
                Err(e) => { eprintln!("stt: {e}"); return ExitCode::from(2); }
            };
            let mut text = String::new();
            let mut built = 0usize;
            let mut pd = 0usize;
            let mut fallback = 0usize;
            for net in &nets {
                let name = net.get("name").and_then(|s| s.as_str()).unwrap_or("?");
                let num = |k: &str| -> Vec<i32> {
                    net.get(k)
                        .and_then(|a| a.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_i64()).map(|v| v as i32).collect())
                        .unwrap_or_default()
                };
                let (x, y) = (num("x"), num("y"));
                let drvr = net.get("drvr").and_then(|d| d.as_u64()).unwrap_or(0) as usize;
                let alpha = net.get("alpha").and_then(|a| a.as_f64()).unwrap_or(0.0) as f32;
                text.push_str(&format!("Net {name}\n"));
                if x.is_empty() || drvr >= x.len() {
                    // ⚠️ Named in the stream rather than skipped: a net that vanishes from the
                    // output is a net the reader has to notice is missing.
                    text.push_str(&format!("REFUSED no pins or drvr {drvr} out of range\n"));
                    continue;
                }
                let (tree, why) = vyges_stt::make_steiner_tree(&lut, &x, &y, drvr, alpha);
                match why {
                    vyges_stt::PdOutcome::PrimDijkstra => pd += 1,
                    vyges_stt::PdOutcome::FellBackCheckFailed => {
                        fallback += 1;
                        vyges_stt::events::fell_back(name);
                    }
                    _ => {}
                }
                match tree {
                    Some(t) => match vyges_stt::report_steiner_tree(&t, x[drvr], y[drvr]) {
                        Some(lines) => {
                            built += 1;
                            vyges_stt::events::net(name, builder_name(&why), t.deg, t.length);
                            for line in lines {
                                text.push_str(&line);
                                text.push('\n');
                            }
                        }
                        None => text.push_str("REFUSED no branch at the driver's location\n"),
                    },
                    None => text.push_str(&format!("REFUSED degree {}\n", x.len())),
                }
            }
            let st = settle_build(nets.len(), built);
            vyges_stt::events::done(st.word(), nets.len(), built, pd, fallback);
            if json {
                // ⚠️ `refused` is a FIELD, not `nets - built`. A caller reading the envelope
                // should not have to do arithmetic to notice that a net went unanswered.
                text = format!(
                    "{{\"status\":\"{}\",\"nets\":{},\"built\":{},\"refused\":{},\
                      \"prim_dijkstra\":{},\"fell_back\":{}}}\n",
                    st.word(),
                    nets.len(),
                    built,
                    nets.len() - built,
                    pd,
                    fallback
                );
            }
            if let Err(e) = emit(out, &text) { eprintln!("stt: {e}"); return ExitCode::from(2); }
            st
        }
        _ => { eprintln!("{USAGE}"); return ExitCode::from(2); }
    };

    // ⛔ Every invocation ends with a census, `batch` included — it emits its own with the real
    // per-builder counts, and this one covers the single-net verbs.
    // ⚠️ Not gated on `quiet` here — the floor above already decides whether it is printed, and
    // suppressing it twice would mean `VYGES_LOG=info -q` behaved differently from `VYGES_LOG=error`.
    if !matches!(positional.first().map(|s| s.as_str()), Some("batch")) {
        let built = usize::from(status == Status::Ok);
        vyges_stt::events::done(status.word(), 1, built, pd_count, fallback_count);
    }
    status.code()
}

fn load_lut() -> Result<vyges_stt::flute::lut::Lut, String> {
    vyges_stt::flute::lut::load_tables(vyges_stt::flute::MAX_LUT_DEGREE)
        .map_err(|e| format!("FLUTE tables: {e:?}"))
}

fn read_nets(path: &str) -> Result<Vec<serde_json::Value>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    v.as_array()
        .cloned()
        .ok_or_else(|| format!("{path}: expected an array of {{name, x, y, drvr, alpha}}"))
}

fn render_check(json: bool, st: Status, tree: &Tree, lines: &[String]) -> String {
    if json {
        return format!(
            "{{\"status\":\"{}\",\"deg\":{},\"branches\":{}}}\n",
            st.word(),
            tree.deg,
            tree.branch_count()
        );
    }
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str(&match st {
        Status::Ok => "check passed\n".to_string(),
        Status::Failed => "check FAILED\n".to_string(),
        // ⛔ Says what was NOT done, so it cannot be read as a pass.
        _ => "check VACUOUS — the tree has no branches, so no rule was consulted\n".to_string(),
    });
    text
}

fn render_run(json: bool, st: Status, tree: Option<&Tree>, builder: &str, lines: &[String]) -> String {
    if json {
        return match tree {
            Some(t) => format!(
                "{{\"status\":\"{}\",\"builder\":\"{}\",\"deg\":{},\"length\":{},\"branches\":{}}}\n",
                st.word(),
                builder,
                t.deg,
                t.length,
                t.branch_count()
            ),
            None => format!("{{\"status\":\"{}\",\"deg\":0,\"branches\":0}}\n", st.word()),
        };
    }
    if lines.is_empty() {
        return "no tree — the net has no pins\n".to_string();
    }
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vacuous_is_never_the_success_code_and_never_the_pass_word() {
        // ⛔ The one property the descriptor's `pass_when: {"eq": "ok"}` depends on. An engine
        // that emits the pass word having built nothing hands a machine a pass on a net it
        // never looked at.
        assert_eq!(settle_build(0, 0), Status::Vacuous);
        assert_ne!(Status::Vacuous.word(), Status::Ok.word());
        assert_ne!(
            format!("{:?}", Status::Vacuous.code()),
            format!("{:?}", ExitCode::SUCCESS)
        );
    }

    #[test]
    fn zero_built_out_of_some_requested_is_REFUSED_not_vacuous() {
        // ⚠️ Different failures: an empty input file, and an engine that declined on every net.
        // Collapsing them loses which one happened.
        assert_eq!(settle_build(0, 0), Status::Vacuous);
        assert_eq!(settle_build(5, 0), Status::Refused);
        assert_eq!(settle_build(5, 1), Status::Ok);
    }

    #[test]
    fn every_status_word_is_declared_in_the_descriptor() {
        // 🔑 The descriptor's prose promises a closed set. A word the engine can emit but the
        // descriptor does not name is a contract the caller cannot rely on.
        let d = describe();
        for st in [Status::Ok, Status::Failed, Status::Vacuous, Status::Refused] {
            assert!(d.contains(st.word()), "{} missing from --describe", st.word());
        }
        assert!(d.contains("error"), "the error word too");
    }

    #[test]
    fn the_descriptor_is_valid_json_and_carries_the_build_pin() {
        let d: serde_json::Value = serde_json::from_str(&describe()).expect("valid JSON");
        assert_eq!(d["name"], "stt");
        assert_eq!(d["assertion"]["pass_when"]["eq"], "ok");
        assert_eq!(d["openroad_pin"], env!("VYGES_STT_OPENROAD_PIN"));
        assert!(!d["openroad_pin"].as_str().unwrap().contains('@'), "the token was substituted");
        assert!(
            d["provenance_limitations"].as_array().unwrap().len() >= 3,
            "provenance_limitations is required and must say something"
        );
    }
}