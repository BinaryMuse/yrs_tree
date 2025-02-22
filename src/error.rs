use std::{error::Error, fmt};

/// Errors that can occur when manipulating a tree.
pub enum TreeError {
    Cycle(String, String),
    MissingParent(String),
    InvalidTarget(String),
}

impl Error for TreeError {}

impl fmt::Debug for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeError::Cycle(child, parent) => write!(f, "Cycle({} -> {})", child, parent),
            TreeError::MissingParent(parent) => write!(f, "MissingParent({})", parent),
            TreeError::InvalidTarget(target) => write!(f, "InvalidTarget({})", target),
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
        }
    }
}
