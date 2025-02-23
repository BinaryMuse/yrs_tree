#![doc = include_str!("../README.md")]

mod error;
mod events;
mod node;
mod tree;
mod tree_structure;

pub use error::TreeError;
pub use events::{Subscription, TreeEvent, TreeObserver};
pub use node::{Node, NodeApi, NodeId};
pub use tree::{DfsIter, Tree};

/// A convenience type alias for the result of tree operations.
pub type Result<T> = std::result::Result<T, TreeError>;
