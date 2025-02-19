use std::{
    collections::{HashMap, VecDeque},
    error::Error,
    sync::{Arc, RwLock},
};

use yrs::{
    types::{EntryChange, PathSegment},
    Map, Out,
};

use crate::{node::Node, yrs_tree::YrsTree};

pub struct TreeState {
    pub(crate) inner: YrsTree,
    pub(crate) nodes: Arc<RwLock<HashMap<String, Node>>>,
}

impl TreeState {
    pub fn new(doc: Arc<yrs::Doc>, tree_name: &str) -> Self {
        let map = Arc::new(RwLock::new(doc.get_or_insert_map(tree_name)));
        let inner = YrsTree::new(doc.clone(), map.clone());

        let nodes = Arc::new(RwLock::new(HashMap::new()));
        let root = Node::new("__ROOT__".to_string());
        nodes.write().unwrap().insert(root.id.clone(), root);

        Self { inner, nodes }
    }

    pub fn handle_events(&self, txn: &yrs::TransactionMut, evts: &yrs::types::Events) {
        for event in evts.iter() {
            if let yrs::types::Event::Map(evt) = event {
                let changes = evt.keys(txn);
                for (key, change) in changes {
                    let path = evt.path();
                    let (child, parent, edge_value) = self.get_edge_change(txn, key, path, change);

                    let mut nodes = self.nodes.write().unwrap();
                    nodes
                        .entry(child.clone())
                        .or_insert(Node::new(child.clone()));

                    nodes
                        .entry(parent.clone())
                        .or_insert(Node::new(parent.clone()));

                    let child_node = nodes.get_mut(&child).unwrap();

                    // TODO: handle undo????

                    child_node
                        .edges
                        .entry(parent.clone())
                        .and_modify(|v| *v = edge_value)
                        .or_insert(edge_value);
                }
            }
        }

        self.rebalance();
    }

    fn get_edge_change(
        &self,
        txn: &yrs::TransactionMut,
        key: &str,
        path: VecDeque<PathSegment>,
        change: &EntryChange,
    ) -> (String, String, i64) {
        match change {
            EntryChange::Inserted(value) => {
                println!("Insert: {:?}", value);
                if path.is_empty() {
                    let (edge, value) = TreeState::get_max_edge(value, txn).unwrap();
                    (key.to_string(), edge, value)
                } else if let PathSegment::Key(inner) = path.back().unwrap() {
                    (
                        inner.to_string(),
                        key.to_string(),
                        value.clone().cast::<i64>().unwrap(),
                    )
                } else {
                    panic!("Unexpected path segment!");
                }
            }
            EntryChange::Updated(old, new) => {
                println!("Update: {:?}, {:?}", old, new);
                let PathSegment::Key(child) = path.back().unwrap() else {
                    panic!("Unexpected path segment!");
                };
                (
                    child.to_string(),
                    key.to_string(),
                    new.clone().cast::<i64>().unwrap(),
                )
            }
            EntryChange::Removed(value) => {
                println!("Remove: {:?}", value);
                ("asdf".to_string(), "asdf".to_string(), 0)
            }
        }
    }

    fn get_max_edge(out: &Out, txn: &yrs::TransactionMut) -> Result<(String, i64), Box<dyn Error>> {
        if let Out::YMap(map) = out {
            let mut max = 0;
            let mut max_key = "".to_string();
            for (key, val) in map.iter(txn) {
                let val = val.cast::<i64>().unwrap();
                if val >= max {
                    max = val;
                    max_key = key.to_string();
                }
            }
            Ok((max_key, max))
        } else {
            Err("Not a map".into())
        }
    }

    pub fn set_parent(&self, id: &str, parent: Option<&str>) -> Result<(), Box<dyn Error>> {
        self.inner.set_parent(id, parent)
    }

    pub fn rebalance(&self) {
        // todo
    }
}
