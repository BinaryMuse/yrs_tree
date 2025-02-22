use std::error::Error;
use std::fmt;
use std::sync::Arc;

use uuid::Uuid;

use crate::{Tree, TreeError};

/// A node in a tree.
#[derive(Clone)]
pub struct Node {
    id: String,
    tree: Arc<Tree>,
}

impl Node {
    pub(crate) fn new(id: &str, tree: Arc<Tree>) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_owned(),
            tree,
        })
    }

    /// Returns the id of the node.
    pub fn id(self: &Arc<Self>) -> &str {
        &self.id
    }

    /// Returns the tree that the node belongs to.
    pub fn tree(self: &Arc<Self>) -> Arc<Tree> {
        self.tree.clone()
    }

    /// Creates a new child node.
    pub fn create_child(self: &Arc<Self>) -> Result<Arc<Self>, Box<dyn Error>> {
        let id = Uuid::now_v7().to_string();
        self.create_child_with_id(&id)
    }

    /// Creates a new child node at the given index.
    pub fn create_child_at(self: &Arc<Self>, index: usize) -> Result<Arc<Self>, Box<dyn Error>> {
        let id = Uuid::now_v7().to_string();
        self.do_create_child(&id, Some(index))
    }

    /// Creates a new child node with the given id.
    pub fn create_child_with_id(self: &Arc<Self>, id: &str) -> Result<Arc<Self>, Box<dyn Error>> {
        self.do_create_child(id, None)
    }

    /// Creates a new child node with the given id at the given index.
    pub fn create_child_with_id_at(
        self: &Arc<Self>,
        id: &str,
        index: usize,
    ) -> Result<Arc<Self>, Box<dyn Error>> {
        self.do_create_child(id, Some(index))
    }

    fn do_create_child(
        self: &Arc<Self>,
        id: &str,
        index: Option<usize>,
    ) -> Result<Arc<Self>, Box<dyn Error>> {
        self.tree.update_node(id, &self.id, index)?;
        Ok(Self::new(id, self.tree.clone()))
    }

    /// Moves the node to the given parent node at the given index.
    /// If the index is not provided, the node will be moved to the end of the
    /// parent's children.
    pub fn move_to(
        self: &Arc<Self>,
        parent: &Node,
        index: Option<usize>,
    ) -> Result<(), Box<dyn Error>> {
        self.tree.update_node(&self.id, &parent.id, index)
    }

    /// Moves the node before the given node.
    pub fn move_before(self: &Arc<Self>, other: &Arc<Node>) -> Result<(), Box<dyn Error>> {
        self.move_relative(other, 0)
    }

    /// Moves the node after the given node.
    pub fn move_after(self: &Arc<Self>, other: &Arc<Node>) -> Result<(), Box<dyn Error>> {
        self.move_relative(other, 1)
    }

    fn move_relative(
        self: &Arc<Self>,
        other: &Arc<Node>,
        offset: usize,
    ) -> Result<(), Box<dyn Error>> {
        if other.id == self.id {
            return Err(TreeError::Cycle(self.id.clone(), other.id.clone()).into());
        }

        if other.id == "__ROOT__" {
            return Err(TreeError::InvalidTarget(other.id.clone()).into());
        }

        let new_parent = other.parent().unwrap();
        let siblings = other.siblings();
        let cur_idx = siblings
            .iter()
            .position(|sibling| sibling.id == other.id)
            .unwrap();

        let new_index = cur_idx + offset;
        self.tree
            .update_node(&self.id, &new_parent.id, Some(new_index))?;

        Ok(())
    }

    /// Returns the children of the node.
    pub fn children(self: &Arc<Self>) -> Vec<Arc<Self>> {
        self.tree
            .get_children(&self.id)
            .into_iter()
            .map(|id| Node::new(&id, self.tree.clone()))
            .collect()
    }

    /// Returns the parent of the node.
    pub fn parent(self: &Arc<Self>) -> Option<Arc<Self>> {
        self.tree
            .get_parent(&self.id)
            .map(|id| Node::new(&id, self.tree.clone()))
    }

    /// Returns the siblings of the node.
    pub fn siblings(self: &Arc<Self>) -> Vec<Arc<Self>> {
        if let Some(parent) = self.parent() {
            let children = parent
                .children()
                .iter()
                .map(|child| Node::new(&child.id, self.tree.clone()))
                .collect();

            children
        } else {
            vec![]
        }
    }

    /// Returns the depth of the node.
    /// The root node has a depth of 0; any user added nodes will have
    /// a depth of at least 1.
    pub fn depth(&self) -> usize {
        if self.id == "__ROOT__" {
            return 0;
        }

        let mut depth = 1;
        let mut current = self.tree.get_parent(&self.id);

        while let Some(parent_id) = current {
            if parent_id == "__ROOT__" {
                break;
            }
            depth += 1;
            current = self.tree.get_parent(&parent_id);
        }
        depth
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node({})", self.id)
    }
}
