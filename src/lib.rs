#![doc = include_str!("../README.md")]

mod error;
pub mod events;
pub mod iter;
pub mod node;
mod tree;
mod tree_structure;

pub use error::TreeError;
pub use events::TreeEvent;
pub use iter::TraversalOrder;
pub use node::{DeleteStrategy, Node, NodeApi, NodeId};
pub use tree::Tree;

/// A convenience type alias for the result of tree operations.
pub type Result<T> = std::result::Result<T, TreeError>;
