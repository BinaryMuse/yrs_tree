use std::error::Error;
use std::fmt;
use std::sync::Arc;

use uuid::Uuid;

use crate::{Tree, TreeError};

/// The ID of a node in a tree.
///
/// Note that `Into<NodeId>` for the string `"<ROOT>"` will return `NodeId::Root`,
/// which cannot be used as a node ID as it is reserved for the actual root node of the tree.
#[derive(Clone, Debug, Default, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum NodeId {
    #[default]
    Root,
    Id(String),
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            NodeId::Root => write!(f, "<ROOT>"),
            NodeId::Id(id) => write!(f, "{}", id),
        }
    }
}

impl PartialEq<&str> for NodeId {
    fn eq(&self, other: &&str) -> bool {
        match self {
            NodeId::Root => *other == "<ROOT>",
            NodeId::Id(id) => id == other,
        }
    }
}

impl From<&str> for NodeId {
    fn from(id: &str) -> Self {
        match id {
            "<ROOT>" => NodeId::Root,
            _ => NodeId::Id(id.to_string()),
        }
    }
}

impl From<&String> for NodeId {
    fn from(id: &String) -> Self {
        NodeId::from(id.as_str())
    }
}

impl From<String> for NodeId {
    fn from(id: String) -> Self {
        NodeId::from(id.as_str())
    }
}

/// A trait for objects that can behave like a node in a tree;
/// this is implemented for [`Node`] and [`Tree`]. When these methods
/// are used on a [`Tree`], they behave as if they were called on the root node.
pub trait NodeApi {
    /// Returns the ID of the node.
    fn id(self: &Arc<Self>) -> &NodeId;

    /// Creates a new child node.
    fn create_child(self: &Arc<Self>) -> Result<Arc<Node>, Box<dyn Error>>;

    /// Creates a new child node at the given index.
    fn create_child_at(self: &Arc<Self>, index: usize) -> Result<Arc<Node>, Box<dyn Error>>;

    /// Creates a new child node with the given ID.
    fn create_child_with_id(
        self: &Arc<Self>,
        id: impl Into<NodeId>,
    ) -> Result<Arc<Node>, Box<dyn Error>>;

    /// Creates a new child node with the given ID at the given index.
    fn create_child_with_id_at(
        self: &Arc<Self>,
        id: impl Into<NodeId>,
        index: usize,
    ) -> Result<Arc<Node>, Box<dyn Error>>;

    /// Moves the node to the given parent, placing it in that parent's children at the given index.
    ///
    /// Given:
    ///
    /// ```text
    /// <ROOT>
    /// ├──A
    /// │  ├──C
    /// │  ├──D
    /// │  └──E
    /// └──B
    /// ```
    ///
    /// If we call `B.move_to(&A, Some(1))`, we get:
    ///
    /// ```text
    /// <ROOT>
    /// └──A
    ///    ├──C
    ///    ├──B
    ///    ├──D
    ///    └──E
    /// ```
    ///
    /// Passing `None` as the index moves the node to the end of the parent's children.
    fn move_to(self: &Arc<Self>, parent: &Node, index: Option<usize>)
        -> Result<(), Box<dyn Error>>;

    /// Moves the node before the given node.
    ///
    /// Given:
    ///
    /// ```text
    /// <ROOT>
    /// ├──A
    /// │  ├──C
    /// │  ├──D
    /// │  └──E
    /// └──B
    /// ```
    ///
    /// If we call `B.move_before(&E)`, we get:
    ///
    /// ```text
    /// <ROOT>
    /// └──A
    ///    ├──C
    ///    ├──D
    ///    ├──B
    ///    └──E
    /// ```
    fn move_before(self: &Arc<Self>, other: &Arc<Node>) -> Result<(), Box<dyn Error>>;

    /// Moves the node after the given node.
    ///
    /// Given:
    ///
    /// ```text
    /// <ROOT>
    /// ├──A
    /// │  ├──C
    /// │  ├──D
    /// │  └──E
    /// └──B
    /// ```
    ///
    /// If we call `B.move_after(&E)`, we get:
    ///
    /// ```text
    /// <ROOT>
    /// └──A
    ///    ├──C
    ///    ├──D
    ///    ├──E
    ///    └──B
    /// ```
    fn move_after(self: &Arc<Self>, other: &Arc<Node>) -> Result<(), Box<dyn Error>>;

    /// Returns the children of the node.
    fn children(self: &Arc<Self>) -> Vec<Arc<Node>>;

    /// Returns the parent of the node.
    fn parent(self: &Arc<Self>) -> Option<Arc<Node>>;

    /// Returns the siblings of the node.
    fn siblings(self: &Arc<Self>) -> Vec<Arc<Node>>;

    /// Returns the depth of the node. The root node has a depth of 0; all other
    /// nodes have a depth of 1 plus the depth of their parent.
    fn depth(self: &Arc<Self>) -> usize;
}

/// A node in a tree.
///
/// * See [`Tree`] for methods to create and find nodes in the tree.
/// * See [`NodeApi`] for the operations that can be performed on a node.
#[derive(Clone)]
pub struct Node {
    id: NodeId,
    tree: Arc<Tree>,
}

impl Node {
    pub(crate) fn new(id: NodeId, tree: Arc<Tree>) -> Arc<Self> {
        Arc::new(Self { id, tree })
    }

    fn do_create_child(
        self: &Arc<Self>,
        id: impl Into<NodeId>,
        index: Option<usize>,
    ) -> Result<Arc<Self>, Box<dyn Error>> {
        let id = id.into();

        if id == NodeId::Root {
            return Err(
                TreeError::InvalidId("<ROOT> cannot be used as a node ID".to_string()).into(),
            );
        }

        self.tree.update_node(&id, &self.id, index)?;
        Ok(Self::new(id, self.tree.clone()))
    }

    fn move_relative(
        self: &Arc<Self>,
        other: &Arc<Node>,
        offset: usize,
    ) -> Result<(), Box<dyn Error>> {
        if other.id == self.id {
            return Err(TreeError::Cycle(self.id.clone(), other.id.clone()).into());
        }

        if other.id == NodeId::Root {
            return Err(TreeError::InvalidTarget(NodeId::Root).into());
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
}

impl NodeApi for Node {
    fn id(self: &Arc<Self>) -> &NodeId {
        &self.id
    }

    fn create_child(self: &Arc<Self>) -> Result<Arc<Self>, Box<dyn Error>> {
        let id = Uuid::now_v7().to_string();
        self.create_child_with_id(id)
    }

    fn create_child_at(self: &Arc<Self>, index: usize) -> Result<Arc<Self>, Box<dyn Error>> {
        let id = Uuid::now_v7().to_string();
        self.do_create_child(id, Some(index))
    }

    fn create_child_with_id(
        self: &Arc<Self>,
        id: impl Into<NodeId>,
    ) -> Result<Arc<Self>, Box<dyn Error>> {
        self.do_create_child(id, None)
    }

    fn create_child_with_id_at(
        self: &Arc<Self>,
        id: impl Into<NodeId>,
        index: usize,
    ) -> Result<Arc<Self>, Box<dyn Error>> {
        self.do_create_child(id, Some(index))
    }

    fn children(self: &Arc<Self>) -> Vec<Arc<Self>> {
        self.tree
            .get_children(&self.id)
            .into_iter()
            .map(|id| Node::new(id.clone(), self.tree.clone()))
            .collect()
    }

    fn parent(self: &Arc<Self>) -> Option<Arc<Self>> {
        self.tree
            .get_parent(&self.id)
            .map(|id| Node::new(id, self.tree.clone()))
    }

    fn siblings(self: &Arc<Self>) -> Vec<Arc<Self>> {
        if let Some(parent) = self.parent() {
            parent.children().clone()
        } else {
            vec![]
        }
    }

    fn depth(self: &Arc<Self>) -> usize {
        if self.id == NodeId::Root {
            return 0;
        }

        let mut depth = 1;
        let mut current = self.tree.get_parent(&self.id);

        while let Some(parent_id) = current {
            if parent_id == NodeId::Root {
                break;
            }
            depth += 1;
            current = self.tree.get_parent(&parent_id);
        }
        depth
    }

    fn move_to(
        self: &Arc<Self>,
        parent: &Node,
        index: Option<usize>,
    ) -> Result<(), Box<dyn Error>> {
        self.tree.update_node(&self.id, &parent.id, index)
    }

    fn move_before(self: &Arc<Self>, other: &Arc<Node>) -> Result<(), Box<dyn Error>> {
        self.move_relative(other, 0)
    }

    fn move_after(self: &Arc<Self>, other: &Arc<Node>) -> Result<(), Box<dyn Error>> {
        self.move_relative(other, 1)
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node({})", self.id)
    }
}
