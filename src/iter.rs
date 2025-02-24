use std::collections::VecDeque;
use std::sync::Arc;

use crate::{tree_structure::TreeStructure, Node, NodeApi, NodeId, Tree};

/// The traversal order for iterating over the nodes in a tree.
#[derive(Clone, Copy)]
pub enum TraversalOrder {
    /// Depth-first traversal
    DepthFirst,
    /// Breadth-first traversal
    BreadthFirst,
}

/// An iterator over the nodes in the tree in either depth-first or breadth-first order.
///
/// The iterator represents a snapshot of the tree at the time of the iterator's creation,
/// and will not reflect changes to the tree after it was created.
pub struct TreeIter {
    tree: Arc<Tree>,
    structure: TreeStructure,
    order: TraversalOrder,
    start: NodeId,
    // For BFS
    queue: VecDeque<NodeId>,
    // For DFS
    last_node: Option<NodeId>,
}

impl TreeIter {
    pub(crate) fn new(tree: Arc<Tree>, start: &NodeId, order: TraversalOrder) -> Self {
        let structure = {
            let lock = tree.structure.lock();
            let structure = lock.borrow().clone();
            structure
        };

        let mut queue = VecDeque::new();
        if matches!(order, TraversalOrder::BreadthFirst) {
            queue.push_back(start.clone());
        }

        Self {
            tree,
            structure,
            order,
            queue,
            start: start.clone(),
            last_node: None,
        }
    }
}

impl Iterator for TreeIter {
    type Item = Arc<Node>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.order {
            TraversalOrder::BreadthFirst => {
                let current_id = self.queue.pop_front()?;

                if let Some(children) = self.structure.get_children(&current_id) {
                    for child in children {
                        self.queue.push_back(child.clone());
                    }
                }

                Some(Node::new(current_id, self.tree.clone()))
            }

            TraversalOrder::DepthFirst => {
                if let Some(last_node) = &self.last_node {
                    let children = self.structure.get_children(last_node)?;

                    if !children.is_empty() {
                        let next_id = &children[0];
                        self.last_node = Some(next_id.clone());
                        Some(Node::new(next_id.clone(), self.tree.clone()))
                    } else {
                        // No children, backtrack to find next sibling
                        let mut current = last_node.clone();
                        loop {
                            // Stop if we've reached the start node while backtracking
                            if current == self.start {
                                return None;
                            }

                            let parent_id = self.structure.get_parent(&current)?;
                            let siblings = self.structure.get_children(parent_id)?;
                            let current_idx = siblings.iter().position(|id| id == &current)?;

                            if current_idx + 1 < siblings.len() {
                                let next_id = &siblings[current_idx + 1];
                                self.last_node = Some(next_id.clone());
                                return Some(Node::new(next_id.clone(), self.tree.clone()));
                            }

                            current = parent_id.clone();
                        }
                    }
                } else {
                    let start = Node::new(self.start.clone(), self.tree.clone());
                    self.last_node = Some(start.id().clone());
                    Some(start)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, sync::Arc};

    use crate::{NodeApi, Tree};

    use super::TraversalOrder;

    fn setup_tree() -> Result<Arc<Tree>, Box<dyn Error>> {
        let doc = Arc::new(yrs::Doc::new());
        let tree = Tree::new(doc.clone(), "test")?;

        let node1 = tree.create_child_with_id("1")?;
        let node2 = tree.create_child_with_id("2")?;
        let node3 = tree.create_child_with_id("3")?;

        let _node4 = node1.create_child_with_id("4")?;
        let _node5 = node1.create_child_with_id("5")?;

        let _node6 = node2.create_child_with_id("6")?;
        let _node7 = node2.create_child_with_id("7")?;
        let _node8 = node2.create_child_with_id("8")?;

        let _node9 = node3.create_child_with_id("9")?;

        Ok(tree)
    }

    #[test]
    fn test_dfs() -> Result<(), Box<dyn Error>> {
        let tree = setup_tree()?;
        let result = tree
            .traverse(TraversalOrder::DepthFirst)
            .map(|n| n.id().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            result,
            vec!["<ROOT>", "1", "4", "5", "2", "6", "7", "8", "3", "9"]
        );

        Ok(())
    }

    #[test]
    fn test_bfs() -> Result<(), Box<dyn Error>> {
        let tree = setup_tree()?;
        let result = tree
            .traverse(TraversalOrder::BreadthFirst)
            .map(|n| n.id().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            result,
            vec!["<ROOT>", "1", "2", "3", "4", "5", "6", "7", "8", "9"]
        );

        Ok(())
    }

    #[test]
    fn test_start_at() -> Result<(), Box<dyn Error>> {
        let tree = setup_tree()?;
        let node = tree.get_node("1").unwrap();
        let result = node
            .traverse(TraversalOrder::DepthFirst)
            .map(|n| n.id().to_string())
            .collect::<Vec<_>>();

        assert_eq!(result, vec!["1", "4", "5"]);

        Ok(())
    }
}
