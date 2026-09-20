// SPDX-License-Identifier: Apache-2.0
//! The `vyges-events` causal trail for this engine.
//!
//! Every event goes to **stderr**; the result goes to stdout (or `-o`), so a caller can parse one
//! without the other. `code` is the clustering key — the thing you group by when the same failure
//! shows up across a hundred runs — and `objects` are the cross-stage co-reference keys, so a
//! refusal here can be linked back to the net that caused it.
//!
//! Codes are stable, and each names *a situation*, not a message:
//!
//! | code | meaning |
//! |---|---|
//! | `STT-BUILT` | a tree was built; carries the builder that answered |
//! | `STT-FELLBACK` | a Prim-Dijkstra tree FAILED the self-overlap check and FLUTE rebuilt the net |
//! | `STT-REFUSED` | no tree for this net, and which path declined |
//! | `STT-NET` | one net in a batch, at `debug` — the per-net trail `-v` turns on |
//! | `STT-DONE` | one invocation finished; the census that discloses coverage |
//!
//! ⛔ **`STT-DONE` is a CENSUS, not a summary.** It reports how many nets were asked for, how many
//! were built, and how many of those came from each builder — so a run that answered a third of
//! its input says so in the trail rather than leaving the caller to infer it from a count they
//! were never given. A fallback is named separately from a plain FLUTE build for the same reason:
//! the two reach the same function and mean different things about the net.

use vyges_events::{emit, Event, Severity};

const TOOL: &str = "vyges-stt";

/// A tree was built. `builder` is which one answered, which the reference does not report.
pub fn built(net: &str, builder: &str, deg: usize, length: i64) {
    emit(
        &Event::new(
            TOOL,
            Severity::Info,
            format!("{net}: {builder} built a degree-{deg} tree, wirelength {length}"),
        )
        .with_code("STT-BUILT")
        .with_objects(vec![format!("net:{net}"), format!("builder:{builder}")]),
    );
}

/// A Prim-Dijkstra tree failed the self-overlap check and was discarded.
///
/// ⚠️ **`warn`, not `info`.** The reference does this silently and the caller gets a perfectly
/// good tree, so nothing is broken — but the net did not get the depth trade-off it asked for,
/// and that is a thing to be able to group on.
pub fn fell_back(net: &str) {
    emit(
        &Event::new(
            TOOL,
            Severity::Warn,
            format!(
                "{net}: the Prim-Dijkstra tree self-overlaps and was DISCARDED; \
                 FLUTE rebuilt the net, so it carries no alpha trade-off"
            ),
        )
        .with_code("STT-FELLBACK")
        .with_objects(vec![format!("net:{net}")]),
    );
}

/// No tree for this net. `why` says which path declined, never just "failed".
pub fn refused(net: &str, why: &str) {
    emit(
        &Event::new(TOOL, Severity::Error, format!("{net}: no tree — {why}"))
            .with_code("STT-REFUSED")
            .with_objects(vec![format!("net:{net}")]),
    );
}

/// One net inside a batch, at `debug`.
///
/// ⚠️ **`debug`, not `info`.** A 145-net sweep would emit 145 info lines and bury the census and
/// the fallbacks, which are the two things worth reading. `-v` (or `VYGES_LOG=debug`) turns it on
/// when the per-net trail is what you actually want.
pub fn net(name: &str, builder: &str, deg: usize, length: i64) {
    emit(
        &Event::new(
            TOOL,
            Severity::Debug,
            format!("{name}: {builder}, degree {deg}, wirelength {length}"),
        )
        .with_code("STT-NET")
        .with_objects(vec![format!("net:{name}"), format!("builder:{builder}")]),
    );
}

/// One invocation finished. The census that discloses coverage.
pub fn done(status: &str, requested: usize, built: usize, pd: usize, fallback: usize) {
    // ⚠️ Anything short of "everything asked for was built" is a `warn`: a partial answer that
    // logs at `info` is a partial answer nobody groups on.
    let sev = if status == "ok" && built == requested {
        Severity::Info
    } else if status == "ok" {
        Severity::Warn
    } else {
        Severity::Error
    };
    emit(
        &Event::new(
            TOOL,
            sev,
            format!(
                "{status} — {built} of {requested} net(s) built \
                 ({pd} by prim-dijkstra, {fallback} fell back to flute after a failed check)"
            ),
        )
        .with_code("STT-DONE")
        .with_objects(vec![format!("status:{status}")]),
    );
}