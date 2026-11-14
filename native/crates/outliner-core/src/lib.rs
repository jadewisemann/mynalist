//! Shared outline logic for the native app: the Rust port of `packages/core`.
//!
//! The wire format (ops, serialized trees, sync requests) is the TS one, so the native app syncs with the same
//! server as the web app. `tests/parity.rs` checks this crate against fixtures recorded from the TS code.

pub mod history;
pub mod ids;
pub mod item;
pub mod session;
pub mod tree;
pub mod tx;

pub use ids::new_id;
pub use session::{compact, Change, ChangeKind, DocSession, DocSnapshot, SyncRequest, SyncResponse};
pub use tree::{NodeId, Op, Props, SerializedTree, TNode, Tree, ROOT};
pub use tx::Tx;
