use std::{
    cell::{Cell, RefCell},
    error::Error,
    fmt,
    sync::Arc,
};

use parking_lot::{ReentrantMutex, RwLock};
use yrs::{DeepObservable, MapRef, Transact};

use crate::{
    events::TreeUpdateEvent, node::Node, tree_structure::TreeStructure, Subscription, TreeObserver,
};

#[derive(Clone)]
pub struct Tree {
    structure: Arc<ReentrantMutex<RefCell<TreeStructure>>>,
    doc: Arc<yrs::Doc>,
    yjs_map: Arc<RwLock<MapRef>>,
    observer: Arc<TreeObserver>,
    #[allow(dead_code)] // cancels subscription when dropped
    subscription: RefCell<Option<yrs::Subscription>>,
    yjs_observer_disabled: Cell<bool>,
}

/// Creates a new tree in the Yjs doc with the given container name.
/// The tree owns this Yjs map, and it should not be modified manually.
impl Tree {
    pub fn new(doc: Arc<yrs::Doc>, tree_name: &str) -> Arc<Self> {
        let yjs_map = Arc::new(RwLock::new(doc.get_or_insert_map(tree_name)));
        let structure = Arc::new(ReentrantMutex::new(RefCell::new(TreeStructure::new())));
        let observer = Arc::new(TreeObserver::new());

        {
            let txn = doc.transact_mut_with("yrs_tree");
            let map = yjs_map.read();
            structure.lock().borrow_mut().init_from_yjs(&map, &txn);
        }

        let structure_clone = structure.clone();
        let yjs_map_clone = yjs_map.clone();
        let observer_clone = observer.clone();

        let tree = Arc::new(Self {
            doc: doc.clone(),
            structure,
            yjs_map,
            observer,
            subscription: RefCell::new(None),
            yjs_observer_disabled: Cell::new(false),
        });

        let tree_clone = tree.clone();

        let map_lock = yjs_map_clone.read().clone();
        let subscription = map_lock.observe_deep(move |txn, _events| {
            // We manually disable the observer when we apply pending edge map updates
            // to avoid trying to re-borrow the structure
            if tree_clone.yjs_observer_disabled.get() {
                return;
            }

            let check_origin = yrs::Origin::from("yrs_tree");
            let lock = structure_clone.lock();
            let mut structure = lock.borrow_mut();

            if txn.origin() == Some(&check_origin) {
                // TODO: handle same origin updates as individual operations
                structure.apply_yjs_update(yjs_map_clone.clone(), txn);
            } else {
                // TODO: determine if we can split this into individual operations
                // If not, reinitialize from the Yjs map
                structure.apply_yjs_update(yjs_map_clone.clone(), txn);
            }

            drop(structure);
            observer_clone.notify(&TreeUpdateEvent(tree_clone.clone()));
        });

        tree.subscription.replace(Some(subscription));

        tree
    }

    pub(crate) fn get_children(&self, id: &str) -> Vec<String> {
        self.structure
            .lock()
            .borrow()
            .get_children(id)
            .unwrap_or_default()
            .to_vec()
    }

    pub(crate) fn update_node(
        self: &Arc<Self>,
        id: &str,
        parent: &str,
        index: Option<usize>,
    ) -> Result<(), Box<dyn Error>> {
        let lock = self.structure.lock();
        let mut structure = lock.borrow_mut();

        if structure.has_pending_edge_map_updates() {
            let mut txn = self.doc.transact_mut_with("yrs_tree");
            let map = self.yjs_map.write();
            self.yjs_observer_disabled.set(true);
            structure.apply_pending_edge_map_updates(&map, &mut txn);
        }

        let mut txn = self.doc.transact_mut_with("yrs_tree");
        let map = self.yjs_map.write();
        let ret = structure.update_node(id, parent, index, &map, &mut txn);
        drop(structure);
        self.yjs_observer_disabled.set(false);
        ret
    }

    pub(crate) fn get_parent(&self, id: &str) -> Option<String> {
        self.structure
            .lock()
            .borrow()
            .get_parent(id)
            .map(|s| s.to_string())
    }

    /// Returns the root node of the tree.
    pub fn root(self: &Arc<Self>) -> Arc<Node> {
        Node::new("__ROOT__", self.clone())
    }

    /// Returns true if the tree has a node with the given id.
    pub fn has_node(self: &Arc<Self>, id: &str) -> bool {
        self.structure.lock().borrow().nodes.contains_key(id)
    }

    /// Returns the node with the given id.
    pub fn get_node(self: &Arc<Self>, id: &str) -> Option<Arc<Node>> {
        self.structure
            .lock()
            .borrow()
            .nodes
            .get(id)
            .map(|node| Node::new(&node.id, self.clone()))
    }

    /// Returns a subscription to the tree. When dropped, the subscription is
    /// automatically cancelled.
    pub fn on_change(
        &self,
        callback: impl Fn(&TreeUpdateEvent) + Send + Sync + 'static,
    ) -> Subscription {
        self.observer.subscribe(callback)
    }

    /// Returns an iterator over the nodes in the tree in depth-first order.
    pub fn traverse_dfs(self: &Arc<Self>) -> DfsIter {
        let structure = self.structure.lock().borrow().clone();
        DfsIter::new(structure, self.clone())
    }
}

impl PartialEq for Tree {
    fn eq(&self, other: &Self) -> bool {
        *self.structure.lock().borrow() == *other.structure.lock().borrow()
    }
}

impl fmt::Debug for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tree({:?})", self.structure.lock().borrow().nodes.len())
    }
}

impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tree = Arc::new(self.clone());
        let iter = tree.traverse_dfs();
        let mut last_depth = 0;
        let mut is_last_at_depth = vec![false];

        for node in iter {
            let depth = node.depth();

            // Adjust the is_last_at_depth vector
            if depth > last_depth {
                is_last_at_depth.extend((last_depth..depth).map(|_| false));
            } else if depth < last_depth {
                is_last_at_depth.truncate(depth + 1);
            }

            // Update is_last status for current depth
            let parent = if node.id() == "__ROOT__" {
                None
            } else {
                self.get_parent(node.id())
            };

            if let Some(parent_id) = parent {
                let siblings = self.get_children(&parent_id);
                is_last_at_depth[depth] = siblings.last().map(|s| s.as_str()) == Some(node.id());
            }

            // Build the prefix
            let mut prefix = String::new();
            for d in 1..depth {
                prefix.push_str(if is_last_at_depth[d] { "   " } else { "│  " });
            }
            if depth > 0 {
                prefix.push_str(if is_last_at_depth[depth] {
                    "└──"
                } else {
                    "├──"
                });
            }

            writeln!(f, "{}{}", prefix, node.id())?;

            last_depth = depth;
        }
        Ok(())
    }
}

pub struct DfsIter {
    tree: Arc<Tree>,
    structure: TreeStructure,
    last_node: Option<String>,
}

impl DfsIter {
    pub fn new(structure: TreeStructure, tree: Arc<Tree>) -> Self {
        Self {
            tree,
            structure,
            last_node: None,
        }
    }
}

impl Iterator for DfsIter {
    type Item = Arc<Node>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(last_node) = &self.last_node {
            // Get children of last visited node
            let children = self.structure.get_children(last_node)?;

            if !children.is_empty() {
                // If there are children, visit first child
                let next_id = &children[0];
                self.last_node = Some(next_id.clone());
                Some(Node::new(next_id, self.tree.clone()))
            } else {
                // No children, backtrack to find next sibling
                let mut current = last_node.clone();
                loop {
                    let parent_id = self.structure.get_parent(&current)?;
                    let siblings = self.structure.get_children(parent_id)?;
                    let current_idx = siblings.iter().position(|id| id == &current)?;

                    if current_idx + 1 < siblings.len() {
                        // Found next sibling
                        let next_id = &siblings[current_idx + 1];
                        self.last_node = Some(next_id.clone());
                        return Some(Node::new(next_id, self.tree.clone()));
                    }

                    if parent_id == "__ROOT__" {
                        // Reached root while backtracking, iteration complete
                        return None;
                    }

                    // Continue backtracking
                    current = parent_id.to_string();
                }
            }
        } else {
            // Start at root
            let root = Node::new("__ROOT__", self.tree.clone());
            self.last_node = Some(root.id().to_string());
            Some(root)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use yrs::{updates::decoder::Decode, ReadTxn, Transact, Update};

    use super::*;

    #[test]
    fn it_works() -> Result<(), Box<dyn Error>> {
        let doc = yrs::Doc::new();
        let tree = Tree::new(Arc::new(doc), "test");
        let root = tree.root();
        // let _sub = tree.on_change(|e| {
        //     let TreeUpdateEvent(tree) = e;
        //     println!("> \n{}", tree);
        // });

        let node1 = root.create_child_with_id("1")?;
        let node2 = root.create_child_with_id("2")?;
        let node3 = node1.create_child_with_id("3")?;
        let node4 = node2.create_child_with_id("4")?;
        node3.move_to(&node2, Some(0))?;
        node1.move_after(&node2)?;
        node4.move_before(&node3)?;

        let nodes = tree
            .traverse_dfs()
            .map(|n| n.id().to_string())
            .collect::<Vec<_>>();

        assert_eq!(nodes, vec!["__ROOT__", "2", "3", "4", "1"]);

        Ok(())
    }

    #[test]
    fn test_sync() -> Result<(), Box<dyn Error>> {
        let doc1 = Arc::new(yrs::Doc::new());
        let doc2 = Arc::new(yrs::Doc::new());

        let tree1 = Tree::new(doc1.clone(), "test");
        let root1 = tree1.root();
        let tree2 = Tree::new(doc2.clone(), "test");

        let node1 = root1.create_child_with_id("1")?;
        let node2 = root1.create_child_with_id("2")?;
        let node3 = node1.create_child_with_id("3")?;
        let node4 = node2.create_child_with_id("4")?;
        node3.move_to(&node2, Some(0))?;
        node1.move_after(&node2)?;
        node4.move_before(&node3)?;

        let txn = doc1.transact();
        let update = txn.encode_state_as_update_v1(&Default::default());
        drop(txn);

        println!("applying update: {} bytes", update.len());

        doc2.transact_mut()
            .apply_update(Update::decode_v1(&update).unwrap())?;

        assert_eq!(tree1, tree2);

        Ok(())
    }

    #[test]
    fn handles_moving_after_cycles() -> Result<(), Box<dyn Error>> {
        let doc1 = Arc::new(yrs::Doc::new());
        let doc2 = Arc::new(yrs::Doc::new());

        let tree1 = Tree::new(doc1.clone(), "test");
        let root1 = tree1.root();
        let tree2 = Tree::new(doc2.clone(), "test");

        let node_c1 = root1.create_child_with_id("C")?;
        let node_d1 = root1.create_child_with_id("D")?;
        let node_a1 = node_c1.create_child_with_id("A")?;
        let node_b1 = node_c1.create_child_with_id("B")?;

        sync_docs(&doc1, &doc2)?;

        // Peer 1 moves A to be a child of B
        node_a1.move_to(&node_b1, None)?;
        // Peer 2 moves B to be a child of A
        let node_b2 = tree2.get_node("B").unwrap();
        let node_a2 = tree2.get_node("A").unwrap();
        node_b2.move_to(&node_a2, None)?;

        sync_docs(&doc1, &doc2)?;

        // Without specially handling the edge map when
        // reparenting, this will unintuitively move A to be a child of D
        // as well as B, because moving B undoes the creation of the cycle that
        // caused it to be parented to C. See
        // https://madebyevan.com/algos/crdt-mutable-tree-hierarchy/
        node_b1.move_to(&node_d1, None)?;

        let nodes = tree1
            .traverse_dfs()
            .map(|n| n.id().to_string())
            .collect::<Vec<_>>();

        assert_eq!(nodes, vec!["__ROOT__", "C", "A", "D", "B"]);

        Ok(())
    }

    fn sync_docs(doc1: &yrs::Doc, doc2: &yrs::Doc) -> Result<(), Box<dyn Error>> {
        let mut txn1 = doc1.transact_mut();
        let sv1 = txn1.state_vector();

        let mut txn2 = doc2.transact_mut();
        let sv2 = txn2.state_vector();

        let update1 = txn1.encode_diff_v1(&sv2);
        let update2 = txn2.encode_diff_v1(&sv1);

        txn1.apply_update(Update::decode_v1(&update2).unwrap())?;
        txn2.apply_update(Update::decode_v1(&update1).unwrap())?;

        Ok(())
    }
}
