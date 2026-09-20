# `stt_gate` — the pinned corpus this engine gates on

`corpus.json` is 41 nets chosen to reach every path the engine has; `stt_gate.ok` is the
reference output for them, captured at the commit in `../../flute-tables.yaml`.

`tests/end_to_end.rs` runs every net through `make_steiner_tree` and compares the wire length,
the path depth and every branch line against the golden. ⛔ Nothing here is illustrative: a change
that moves any one of those 802 lines fails the build.

| field | meaning |
| --- | --- |
| `x`, `y` | pin locations, equal length |
| `drvr` | driver **pin index** into `x`/`y` — never a coordinate |
| `alpha` | `> 0` selects Prim-Dijkstra; `0` selects FLUTE |
| `why` | which builder must answer: `flute`, `pd`, or `fallback` |

`why` is the part a golden alone cannot check. Two builders can agree on a tree, so matching the
reference's output does not prove the right one produced it — that is what fails when the alpha
branch or the `check_tree` fallback is wired wrongly.

## Regenerating

⛔ **Never edit `stt_gate.ok` by hand**, and recapture it whenever `../../flute-tables.yaml`
moves — the reference's own answer moves between commits, so a golden carried across a bump is a
golden nobody checked.
