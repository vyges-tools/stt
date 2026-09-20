# vyges-stt

Rectilinear Steiner minimal trees over a net's pin locations.

Two builders behind one entry point: **FLUTE** for minimum wirelength, and **Prim-Dijkstra** when a
wirelength/depth trade-off is asked for. A Prim-Dijkstra tree is used only if it passes a
self-overlap check; otherwise the net falls back to FLUTE.

The FLUTE lookup tables (`POWV9`/`POST9`, 9.4 MB of precomputed optimal topologies for degree ≤ 9)
are third-party data, fetched at build time rather than vendored — see `flute-tables.yaml`.

## Correctness

Validated against OpenROAD's Steiner tree builders at a pinned commit, over every tree-producing
case in their suite plus a 41-net corpus of our own — 341 trees, each compared on wire length,
path depth and every branch. `--describe` carries the detail and the two transcription choices no
input we have distinguishes.

Every net is checked through the public entry point rather than through the two builders
separately, because the branch between them is the part that has no other witness.

## Usage

```
vyges loom stt run   <job.json>   [-o FILE] [--json]
vyges loom stt batch <nets.json>  [-o FILE] [--json]
vyges loom stt check <tree.json>  [-o FILE] [--json]
vyges loom stt --describe
```

A job is `{"x": [...], "y": [...], "drvr": 0, "alpha": 0.0}`. `drvr` is a **pin index** into
`x`/`y`, never a coordinate — that is the reference's `driver_index`. `alpha > 0` selects
Prim-Dijkstra; `0`, the default, selects FLUTE.

`batch` takes an array of jobs each with a `name`, and loads the 9.4 MB lookup table **once**.

### Exit status

| code | status | meaning |
| --- | --- | --- |
| 0 | `ok` | a tree was built, or the check passed |
| 1 | `failed` | the tree FAILED the self-overlap check — a finding, not an error |
| 2 | `vacuous` | nothing was built: no pins, or a batch of none. **Not a pass.** |
| 2 | `error` | usage, unreadable input, or a driver index that is not a pin |
| 3 | `refused` | the engine declined to answer; stderr says which path declined |

⛔ **"Found something" and "could not run" never share a code.** A CI gate that cannot tell a broken
tool from a bad design will eventually read a missing engine as a clean run.

## The gate

`tests/end_to_end.rs` compares every net in `examples/stt_gate/corpus.json` against the golden
beside it, line for line. Nothing there is illustrative: a change that moves any one of those 802
lines fails the build.

## Licence

Apache-2.0. See `LICENSE` and `NOTICE`.
