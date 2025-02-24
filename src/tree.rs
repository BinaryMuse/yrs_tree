use std::{
    cell::{Cell, RefCell},
    fmt,
    sync::Arc,
};

use parking_lot::{ReentrantMutex, RwLock};
use yrs::{block::Prelim, DeepObservable, MapRef, Transact};

use crate::{
    events::{Subscription, TreeEvent, TreeObserver},
    iter::{TraversalOrder, TreeIter},
    node::{DeleteStrategy, Node, NodeId},
    tree_structure::TreeStructure,
    Result, TreeError,
};

pub use crate::node::NodeApi;

/// A tree CRDT backed by a Yrs document.
///
/// `Tree` implements [`NodeApi`], forwarding the calls to the root node of the tree,
/// allowing you to add nodes to the root node without calling `root()`.
///
/// ## Tree Poisoning
///
/// When the underlying Yrs document is updated, the tree automatically updates its
/// state in response. If the library detects that the Yrs document is malformed in a way
/// that cannot be reconciled, it will mark the tree as "poisoned."
///
/// Once a tree is poisoned, any operations on the tree that rely on the Yrs document will
/// fail with a `TreePoisoned` error. Operations that only rely on the tree's cached state
/// will continue to succeed, but will not reflect the latest state of the Yrs document.
///
/// You can receive a notification when a tree is poisoned by subscribing to the tree's
/// events via [`Tree::on_change`].
#[derive(Clone)]
pub struct Tree {
    pub(crate) structure: Arc<ReentrantMutex<RefCell<TreeStructure>>>,
    doc: Arc<yrs::Doc>,
    yjs_map: Arc<RwLock<MapRef>>,
    observer: Arc<TreeObserver>,
    #[allow(dead_code)] // cancels subscription when dropped
    subscription: RefCell<Option<yrs::Subscription>>,
    yjs_observer_disabled: Cell<bool>,
    poisioned: RefCell<Option<TreeError>>,
}

impl Tree {
    /// Creates a new tree in the Yjs doc with the given container name.
    /// The tree will take over the map at the given name in the Yrs doc, and it should not
    /// be modified manually after creation.
    pub fn new(doc: Arc<yrs::Doc>, tree_name: &str) -> Result<Arc<Self>> {
        let yjs_map = Arc::new(RwLock::new(doc.get_or_insert_map(tree_name)));
        let structure = Arc::new(ReentrantMutex::new(RefCell::new(TreeStructure::new())));
        let observer = Arc::new(TreeObserver::new());

        {
            let txn = doc.transact_mut_with("yrs_tree");
            let map = yjs_map.read();
            structure.lock().borrow_mut().init_from_yjs(&map, &txn)?;
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
            poisioned: RefCell::new(None),
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
            let data_origin = yrs::Origin::from("yrs_tree_data");

            if txn.origin() == Some(&data_origin) {
                return;
            }

            let lock = structure_clone.lock();
            let mut structure = lock.borrow_mut();

            let update_result = if txn.origin() == Some(&check_origin) {
                // TODO: handle same origin updates as individual operations
                structure.apply_yjs_update(yjs_map_clone.clone(), txn)
            } else {
                // TODO: determine if we can split this into individual operations
                // If not, reinitialize from the Yjs map
                structure.apply_yjs_update(yjs_map_clone.clone(), txn)
            };

            drop(structure);

            match update_result {
                Ok(_) => observer_clone.notify(&TreeEvent::TreeUpdated(tree_clone.clone())),
                Err(e) => {
                    tree_clone.mark_poisoned(e);
                }
            }
        });

        tree.subscription.replace(Some(subscription));

        Ok(tree)
    }

    fn mark_poisoned(self: &Arc<Self>, orig: TreeError) {
        self.poisioned.borrow_mut().replace(orig.clone());
        self.observer.notify(&TreeEvent::TreePoisoned(
            self.clone(),
            TreeError::TreePoisoned(Box::new(orig)),
        ))
    }

    /// Returns true if the tree is poisoned.
    pub fn is_poisoned(&self) -> bool {
        self.poisioned.borrow().is_some()
    }

    /// Returns the reason the tree is poisoned, if it is poisoned.
    pub fn get_poisoned_reason(&self) -> Option<TreeError> {
        self.poisioned.borrow().as_ref().map(|e| e.clone())
    }

    pub(crate) fn get_children(&self, id: &NodeId) -> Vec<NodeId> {
        self.structure
            .lock()
            .borrow()
            .get_children(id)
            .unwrap_or_default()
            .to_vec()
    }

    pub(crate) fn update_node(
        self: &Arc<Self>,
        id: &NodeId,
        parent: &NodeId,
        index: Option<usize>,
    ) -> Result<()> {
        if let Some(poisioned) = self.poisioned.borrow().as_ref() {
            return Err(TreeError::TreePoisoned(Box::new(poisioned.clone())).into());
        }

        let lock = self.structure.lock();
        let mut structure = lock.borrow_mut();

        let res = if structure.has_pending_edge_map_updates() {
            let mut txn = self.doc.transact_mut_with("yrs_tree");
            let map = self.yjs_map.write();
            self.yjs_observer_disabled.set(true);
            structure.apply_pending_edge_map_updates(&map, &mut txn)
        } else {
            Ok(())
        };

        if let Err(e) = &res {
            if let TreeError::TreePoisoned(err) = e {
                self.mark_poisoned((**err).clone());
                let clone = e.clone();
                return Err(clone);
            }
        }

        let mut txn = self.doc.transact_mut_with("yrs_tree");
        let map = self.yjs_map.write();
        let ret = structure.update_node(id, parent, index, &map, &mut txn);
        drop(structure);
        self.yjs_observer_disabled.set(false);
        ret
    }

    pub(crate) fn get_parent(&self, id: &NodeId) -> Option<NodeId> {
        match id {
            NodeId::Root => None,
            NodeId::Id(_) => self.structure.lock().borrow().get_parent(id).cloned(),
        }
    }

    /// Returns the root node of the tree.
    pub fn root(self: &Arc<Self>) -> Arc<Node> {
        Node::new(NodeId::Root, self.clone())
    }

    /// Returns true if the tree has a node with the given ID.
    pub fn has_node(self: &Arc<Self>, id: impl Into<NodeId>) -> bool {
        let id = id.into();
        match &id {
            NodeId::Root => true,
            NodeId::Id(_) => self.structure.lock().borrow().nodes.contains_key(&id),
        }
    }

    /// Returns the node with the given ID.
    pub fn get_node(self: &Arc<Self>, id: impl Into<NodeId>) -> Option<Arc<Node>> {
        let id = id.into();
        match &id {
            NodeId::Root => Some(self.root()),
            NodeId::Id(_) => self
                .structure
                .lock()
                .borrow()
                .nodes
                .get(&id)
                .map(|node| Node::new(node.id.clone(), self.clone())),
        }
    }

    pub(crate) fn delete_node(
        self: &Arc<Self>,
        id: &NodeId,
        strategy: DeleteStrategy,
    ) -> Result<()> {
        let to_delete = match strategy {
            DeleteStrategy::Promote => vec![id.clone()],
            DeleteStrategy::Cascade => {
                let node = self.get_node(id.clone()).unwrap();
                let mut to_delete = node
                    .traverse(TraversalOrder::BreadthFirst)
                    .map(|n| n.id().clone())
                    .collect::<Vec<_>>();
                to_delete.reverse();
                to_delete
            }
        };

        if strategy == DeleteStrategy::Promote {
            let parent = self
                .get_parent(id)
                .ok_or(TreeError::InvalidTarget(id.clone()))?;
            let children = self.get_children(id);
            for child in children {
                self.update_node(&child, &parent, None)?;
            }
        }

        self.delete_nodes(&to_delete)
    }

    pub(crate) fn delete_nodes(self: &Arc<Self>, ids: &[NodeId]) -> Result<()> {
        if let Some(poisioned) = self.poisioned.borrow().as_ref() {
            return Err(TreeError::TreePoisoned(Box::new(poisioned.clone())).into());
        }

        let mut txn = self.doc.transact_mut_with("yrs_tree");
        let map = self.yjs_map.write();
        let lock = self.structure.lock();
        let mut structure = lock.borrow_mut();

        let result = structure.delete_nodes(ids, &map, &mut txn);
        drop(structure);

        result
    }

    pub(crate) fn set_data<V: Prelim + Into<yrs::Any>>(
        self: &Arc<Self>,
        id: &NodeId,
        key: &str,
        value: V,
    ) -> Result<V::Return> {
        if let Some(poisioned) = self.poisioned.borrow().as_ref() {
            return Err(TreeError::TreePoisoned(Box::new(poisioned.clone())).into());
        }

        let mut txn = self.doc.transact_mut_with("yrs_tree_data");
        let map = self.yjs_map.write();
        let result = self
            .structure
            .lock()
            .borrow_mut()
            .set_data(id, key, value, &map, &mut txn);

        if let Err(e) = &result {
            if let TreeError::TreePoisoned(err) = e {
                self.mark_poisoned((**err).clone());
                return result;
            }
        }

        result
    }

    pub(crate) fn get_data(self: &Arc<Self>, id: &NodeId, key: &str) -> Result<Option<yrs::Out>> {
        if let Some(poisioned) = self.poisioned.borrow().as_ref() {
            return Err(TreeError::TreePoisoned(Box::new(poisioned.clone())).into());
        }

        let mut txn = self.doc.transact();
        let map = self.yjs_map.read();
        let result = self
            .structure
            .lock()
            .borrow()
            .get_data(id, key, &map, &mut txn);

        if let Err(e) = &result {
            if let TreeError::TreePoisoned(err) = e {
                self.mark_poisoned((**err).clone());
                return result;
            }
        }

        result
    }

    pub(crate) fn get_data_as<V: serde::de::DeserializeOwned>(
        self: &Arc<Self>,
        id: &NodeId,
        key: &str,
    ) -> Result<V> {
        let result = self.structure.lock().borrow().get_data_as(
            id,
            key,
            &self.yjs_map.read(),
            &mut self.doc.transact(),
        );

        if let Err(e) = &result {
            if let TreeError::TreePoisoned(err) = e {
                self.mark_poisoned((**err).clone());
                return result;
            }
        }

        result
    }

    /// Returns a subscription to the tree's events. When dropped, the subscription
    /// is automatically cancelled.
    pub fn on_change(&self, callback: impl Fn(&TreeEvent) + Send + Sync + 'static) -> Subscription {
        self.observer.subscribe(callback)
    }

    pub(crate) fn traverse_starting_at(
        self: &Arc<Self>,
        start: &NodeId,
        order: TraversalOrder,
    ) -> TreeIter {
        TreeIter::new(self.clone(), start, order)
    }
}

/// `Tree` implements [`NodeApi`], forwarding the calls to the root node of the tree
impl NodeApi for Tree {
    #[inline]
    fn id(self: &Arc<Self>) -> &NodeId {
        &NodeId::Root
    }

    #[inline]
    fn create_child(self: &Arc<Self>) -> Result<Arc<Node>> {
        self.root().create_child()
    }

    #[inline]
    fn create_child_at(self: &Arc<Self>, index: usize) -> Result<Arc<Node>> {
        self.root().create_child_at(index)
    }

    #[inline]
    fn create_child_with_id(self: &Arc<Self>, id: impl Into<NodeId>) -> Result<Arc<Node>> {
        self.root().create_child_with_id(id)
    }

    #[inline]
    fn create_child_with_id_at(
        self: &Arc<Self>,
        id: impl Into<NodeId>,
        index: usize,
    ) -> Result<Arc<Node>> {
        self.root().create_child_with_id_at(id, index)
    }

    #[inline]
    fn move_to(self: &Arc<Self>, _parent: &Node, _index: Option<usize>) -> Result<()> {
        Err(TreeError::UnsupportedOperation("Cannot move the root node".to_string()).into())
    }

    #[inline]
    fn move_before(self: &Arc<Self>, _other: &Arc<Node>) -> Result<()> {
        Err(TreeError::UnsupportedOperation("Cannot move the root node".to_string()).into())
    }

    #[inline]
    fn move_after(self: &Arc<Self>, _other: &Arc<Node>) -> Result<()> {
        Err(TreeError::UnsupportedOperation("Cannot move the root node".to_string()).into())
    }

    #[inline]
    fn children(self: &Arc<Self>) -> Vec<Arc<Node>> {
        self.root().children()
    }

    #[inline]
    fn parent(self: &Arc<Self>) -> Option<Arc<Node>> {
        self.root().parent()
    }

    #[inline]
    fn ancestors(self: &Arc<Self>) -> Vec<Arc<Node>> {
        self.root().ancestors()
    }

    #[inline]
    fn descendants(self: &Arc<Self>, order: TraversalOrder) -> Vec<Arc<Node>> {
        self.root().descendants(order)
    }

    #[inline]
    fn siblings(self: &Arc<Self>) -> Vec<Arc<Node>> {
        self.root().siblings()
    }

    #[inline]
    fn depth(self: &Arc<Self>) -> usize {
        self.root().depth()
    }

    #[inline]
    fn delete(self: &Arc<Self>, strategy: DeleteStrategy) -> Result<()> {
        self.root().delete(strategy)
    }

    #[inline]
    fn traverse(self: &Arc<Self>, order: TraversalOrder) -> TreeIter {
        self.clone().traverse_starting_at(self.root().id(), order)
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
        let iter = tree.traverse(TraversalOrder::DepthFirst);
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
            let parent = if node.id() == &NodeId::Root {
                None
            } else {
                self.get_parent(node.id())
            };

            if let Some(parent_id) = parent {
                let siblings = self.get_children(&parent_id);
                is_last_at_depth[depth] = siblings.last() == Some(node.id());
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

#[cfg(test)]
mod tests {
    use std::error::Error;

    use parking_lot::Mutex;
    use yrs::{updates::decoder::Decode, Map, ReadTxn, Transact, Update};

    use super::*;

    #[test]
    fn it_works() -> Result<()> {
        let doc = yrs::Doc::new();
        let tree = Tree::new(Arc::new(doc), "test")?;
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
            .traverse(TraversalOrder::DepthFirst)
            .map(|n| (n.id().to_string(), n.depth()))
            .collect::<Vec<_>>();

        assert_eq!(
            nodes,
            vec![("<ROOT>", 0), ("2", 1), ("3", 2), ("4", 2), ("1", 1)]
                .iter()
                .map(|(id, depth)| (id.to_string(), *depth as usize))
                .collect::<Vec<_>>()
        );

        Ok(())
    }

    #[test]
    fn test_sync() -> Result<()> {
        let doc1 = Arc::new(yrs::Doc::new());
        let doc2 = Arc::new(yrs::Doc::new());

        let tree1 = Tree::new(doc1.clone(), "test")?;
        let tree2 = Tree::new(doc2.clone(), "test")?;

        let node1 = tree1.create_child_with_id("1")?;
        let node2 = tree1.create_child_with_id("2")?;
        let node3 = node1.create_child_with_id("3")?;
        let node4 = node2.create_child_with_id("4")?;
        node3.move_to(&node2, Some(0))?;
        node1.move_after(&node2)?;
        node4.move_before(&node3)?;

        let txn = doc1.transact();
        let update = txn.encode_state_as_update_v1(&Default::default());
        drop(txn);

        doc2.transact_mut()
            .apply_update(Update::decode_v1(&update).unwrap())
            .unwrap();

        assert_eq!(tree1, tree2);

        Ok(())
    }

    #[test]
    fn handles_moving_after_cycles() -> Result<()> {
        let doc1 = Arc::new(yrs::Doc::new());
        let doc2 = Arc::new(yrs::Doc::new());

        let tree1 = Tree::new(doc1.clone(), "test")?;
        let tree2 = Tree::new(doc2.clone(), "test")?;

        let node_c1 = tree1.create_child_with_id("C")?;
        let node_d1 = tree1.create_child_with_id("D")?;
        let node_a1 = node_c1.create_child_with_id("A")?;
        let node_b1 = node_c1.create_child_with_id("B")?;

        sync_docs(&doc1, &doc2).unwrap();

        // Peer 1 moves A to be a child of B
        node_a1.move_to(&node_b1, None)?;
        // Peer 2 moves B to be a child of A
        let node_b2 = tree2.get_node("B").unwrap();
        let node_a2 = tree2.get_node("A").unwrap();
        node_b2.move_to(&node_a2, None)?;

        sync_docs(&doc1, &doc2).unwrap();

        // Without specially handling the edge map when
        // reparenting, this will unintuitively move A to be a child of D
        // as well as B, because moving B undoes the creation of the cycle that
        // caused it to be parented to C. See
        // https://madebyevan.com/algos/crdt-mutable-tree-hierarchy/
        node_b1.move_to(&node_d1, None)?;

        let nodes = tree1
            .traverse(TraversalOrder::DepthFirst)
            .map(|n| n.id().to_string())
            .collect::<Vec<_>>();

        assert_eq!(nodes, vec!["<ROOT>", "C", "A", "D", "B"]);

        Ok(())
    }

    #[test]
    fn errors_creating_root() -> Result<()> {
        let doc = Arc::new(yrs::Doc::new());
        let tree = Tree::new(doc, "test")?;

        let res = tree.create_child_with_id("<ROOT>");
        assert!(res.is_err());

        Ok(())
    }

    #[test]
    fn handles_poisioning() -> Result<()> {
        let doc = Arc::new(yrs::Doc::new());
        let tree = Tree::new(doc.clone(), "test")?;
        let poisoned = Arc::new(Mutex::new(false));
        let poisoned_clone = poisoned.clone();
        let _sub = tree.on_change(move |e| {
            if let TreeEvent::TreePoisoned(_, _) = e {
                *poisoned_clone.lock() = true;
            }
        });

        let node = tree.create_child_with_id("1")?;
        node.set("test", "test")?;

        let map = doc.get_or_insert_map("test");
        let mut txn = doc.transact_mut();
        let Some(yrs::Out::YMap(map_ref)) = map.get(&txn, node.id().to_string().as_str()) else {
            panic!("Map not found");
        };

        // Change the data map to poison the tree
        map_ref.insert(&mut txn, "data", "asdfasdf");
        drop(txn);

        let _data = node.get("test");

        assert_eq!(tree.is_poisoned(), true);
        assert_eq!(*poisoned.lock(), true);

        Ok(())
    }

    #[test]
    fn test_delete_promote() -> Result<()> {
        let doc = Arc::new(yrs::Doc::new());
        let tree = Tree::new(doc.clone(), "test")?;

        let node1 = tree.create_child_with_id("1")?;
        let node2 = tree.create_child_with_id("2")?;
        let node3 = node1.create_child_with_id("3")?;
        let _node4 = node1.create_child_with_id("4")?;
        let _node5 = node2.create_child_with_id("5")?;
        let _node6 = node2.create_child_with_id("6")?;
        let _node7 = node3.create_child_with_id("7")?;

        node3.delete(DeleteStrategy::Promote)?;

        let nodes = tree
            .traverse(TraversalOrder::BreadthFirst)
            .map(|n| n.id().to_string())
            .collect::<Vec<_>>();

        assert_eq!(nodes, vec!["<ROOT>", "1", "2", "4", "7", "5", "6"]);

        Ok(())
    }

    #[test]
    fn test_delete_cascade() -> Result<()> {
        let doc = Arc::new(yrs::Doc::new());
        let tree = Tree::new(doc.clone(), "test")?;

        let node1 = tree.create_child_with_id("1")?;
        let node2 = tree.create_child_with_id("2")?;
        let node3 = node1.create_child_with_id("3")?;
        let _node4 = node1.create_child_with_id("4")?;
        let _node5 = node2.create_child_with_id("5")?;
        let _node6 = node2.create_child_with_id("6")?;
        let _node7 = node3.create_child_with_id("7")?;

        node3.delete(DeleteStrategy::Cascade)?;

        let nodes = tree
            .traverse(TraversalOrder::BreadthFirst)
            .map(|n| n.id().to_string())
            .collect::<Vec<_>>();

        assert_eq!(nodes, vec!["<ROOT>", "1", "2", "4", "5", "6"]);

        Ok(())
    }

    fn sync_docs(doc1: &yrs::Doc, doc2: &yrs::Doc) -> std::result::Result<(), Box<dyn Error>> {
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
