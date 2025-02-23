use std::{error::Error, fmt};

use crate::node::NodeId;

/// Errors that can occur when manipulating a tree.
pub enum TreeError {
    Cycle(NodeId, NodeId),
    MissingParent(NodeId),
    InvalidTarget(NodeId),
    UnsupportedOperation(String),
    InvalidId(String),
}

impl Error for TreeError {}

impl fmt::Debug for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeError::Cycle(child, parent) => write!(f, "Cycle({} -> {})", child, parent),
            TreeError::MissingParent(parent) => write!(f, "MissingParent({})", parent),
            TreeError::InvalidTarget(target) => write!(f, "InvalidTarget({})", target),
            TreeError::UnsupportedOperation(operation) => {
                write!(f, "UnsupportedOperation({})", operation)
            }
            TreeError::InvalidId(id) => write!(f, "InvalidId({})", id),
        }
    }
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeError::Cycle(child, parent) => {
                write!(f, "Operation would create a cycle: {} -> {}", child, parent)
            }
            TreeError::MissingParent(parent) => {
                write!(f, "Requested parent node does not exist: {}", parent)
            }
            TreeError::InvalidTarget(target) => {
                write!(f, "Invalid target: {}", target)
            }
            TreeError::UnsupportedOperation(operation) => {
                write!(f, "Unsupported operation: {}", operation)
            }
            TreeError::InvalidId(id) => write!(f, "Invalid ID: {}", id),
        }
    }
}
