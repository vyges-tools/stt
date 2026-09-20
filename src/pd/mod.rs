// SPDX-License-Identifier: Apache-2.0
//! Prim-Dijkstra — the wirelength/depth trade-off builder.
//!
//! The façade reaches this whenever `alpha > 0`, and falls back to FLUTE when the resulting tree
//! fails [`crate::check::check_tree`]. Nine stages, in the reference's order; see
//! `pd.cpp::primDijkstra`.

pub mod driver;
pub mod finish;
pub mod graph;
pub mod neighbors;
pub mod spanning;
pub mod steiner;

pub use neighbors::nearest_neighbors;
pub use driver::{prim_dijkstra, PdError};
pub use finish::{make_tree, split_degree4_nodes};
pub use graph::Graph;
pub use spanning::{build_spanning_tree, Edge};
pub use steiner::{best_steiner_for_node, steinerize};