use std::{
    collections::{BTreeSet, HashMap},
    error::Error,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use fractional_index::FractionalIndex;
use parking_lot::RwLock;
use yrs::{Map, MapPrelim, MapRef, Out};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EdgeMap(HashMap<String, i64>);

impl EdgeMap {
    pub fn from_map(map: HashMap<String, i64>) -> Self {
        Self(map)
    }

    pub fn max_edge(&self) -> Option<(String, i64)> {
        self.iter()
            .max_by(|(_, a), (_, b)| a.cmp(b))
            .map(|(id, edge)| (id.clone(), *edge))
    }

    pub fn edges_desc(&self) -> Vec<(String, i64)> {
        let mut edges = self
            .iter()
            .map(|(id, edge)| (id.clone(), *edge))
            .collect::<Vec<_>>();
        edges.sort_by(|(_, a), (_, b)| b.cmp(a));
        edges
    }

    pub fn add_edge(&mut self, id: &str) -> (String, i64) {
        if let Some((_, edge)) = self.max_edge() {
            let new_edge_val = edge + 1;
            self.entry(id.to_string())
                .and_modify(|v| *v = new_edge_val)
                .or_insert(new_edge_val);
            (id.to_string(), new_edge_val)
        } else {
            self.insert(id.to_string(), 0);
            (id.to_string(), 0)
        }
    }
}

impl Deref for EdgeMap {
    type Target = HashMap<String, i64>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for EdgeMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<HashMap<String, i64>> for EdgeMap {
    fn from(map: HashMap<String, i64>) -> Self {
        Self(map)
    }
}

pub struct NodeContainer {
    pub id: String,
    pub edge_map: EdgeMap,
    pub fi: FractionalIndex,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub fi: FractionalIndex,
    pub edge_map: EdgeMap,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TreeStructure {
    pub nodes: HashMap<String, TreeNode>,
    pending_edge_map_updates: Vec<(String, String, i64)>,
}

impl TreeStructure {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            pending_edge_map_updates: Vec::new(),
        }
    }

    pub fn get_children(&self, id: &str) -> Option<&[String]> {
        self.nodes.get(id).map(|node| node.children.as_slice())
    }

    pub fn get_parent(&self, id: &str) -> Option<&str> {
        self.nodes.get(id)?.parent_id.as_deref()
    }

    pub(crate) fn get_node(&self, id: &str) -> Option<&TreeNode> {
        self.nodes.get(id)
    }

    pub(crate) fn init_from_yjs(&mut self, map: &MapRef, txn: &yrs::TransactionMut) {
        // Clear nodes in case of re-initialization due to large Yjs updates
        self.nodes.clear();

        let containers = Self::collect_node_containers(map, txn);
        self.create_initial_nodes(&containers);
        let non_attached_nodes = self.process_parent_relationships(&containers);
        self.reattach_nodes(non_attached_nodes, map, txn);
        self.update_children_order();
    }

    pub(crate) fn apply_yjs_update(&mut self, map: Arc<RwLock<MapRef>>, txn: &yrs::TransactionMut) {
        let map = map.read();
        self.init_from_yjs(&map, txn);
    }

    fn collect_node_containers(map: &MapRef, txn: &yrs::TransactionMut) -> Vec<NodeContainer> {
        let mut containers = Vec::new();
        for (id, out) in map.iter(txn) {
            if let yrs::Out::YMap(container) = out {
                let edge_map: HashMap<String, i64> =
                    container.get_as(txn, "em").unwrap_or_default();
                let fi_str: String = container.get_as(txn, "fi").unwrap_or_default();
                let fi = FractionalIndex::from_string(&fi_str).unwrap_or_default();
                containers.push(NodeContainer {
                    id: id.to_string(),
                    edge_map: edge_map.into(),
                    fi,
                });
            }
        }
        containers
    }

    fn create_initial_nodes(&mut self, containers: &[NodeContainer]) {
        let root = TreeNode {
            id: "__ROOT__".to_string(),
            parent_id: None,
            children: vec![],
            fi: FractionalIndex::default(),
            edge_map: EdgeMap::default(),
        };
        self.nodes.insert("__ROOT__".to_string(), root);

        for container in containers {
            let id = &container.id;
            let fi = &container.fi;
            let node = TreeNode {
                id: id.clone(),
                parent_id: None,
                children: vec![],
                fi: fi.clone(),
                edge_map: container.edge_map.clone().into(),
            };
            self.nodes.insert(id.clone(), node);
        }
    }

    fn process_parent_relationships(
        &mut self,
        containers: &Vec<NodeContainer>,
    ) -> BTreeSet<String> {
        let nodes_to_process = containers;

        for node in nodes_to_process {
            let parent_id = node.edge_map.max_edge().map(|(id, _)| id);
            if let Some(parent_id) = parent_id {
                let node = self.nodes.get_mut(&node.id).unwrap();
                node.parent_id = Some(parent_id.clone());
            } else {
                panic!("No parent found for node: {}", node.id);
            }
        }

        let mut non_attached_nodes = BTreeSet::new();
        for node in self.nodes.values() {
            if !self.can_reach(&node.id, "__ROOT__") {
                non_attached_nodes.insert(node.id.clone());
            }
        }

        non_attached_nodes
    }

    fn reattach_nodes(
        &mut self,
        mut non_attached_nodes: BTreeSet<String>,
        map: &MapRef,
        txn: &yrs::TransactionMut,
    ) {
        while !non_attached_nodes.is_empty() {
            // find the historical parent with the highest edge value
            // that is also not inside the non_attached_nodes set
            let next = non_attached_nodes.first().unwrap().clone();
            if self.can_reach(&next, "__ROOT__") {
                non_attached_nodes.remove(&next);
                continue;
            }

            let node = self.nodes.get_mut(&next).unwrap();
            let edges_desc = &node.edge_map.edges_desc();
            let first_valid_parent = edges_desc
                .iter()
                .find(|(id, _)| !non_attached_nodes.contains(id));

            if let Some((parent_id, _)) = first_valid_parent {
                node.parent_id = Some(parent_id.clone());
                let (edge_id, edge_val) = node.edge_map.add_edge(parent_id);
                let yrs::Out::YMap(container) = map.get(txn, &node.id).unwrap() else {
                    panic!("Node is not a container: {}", node.id);
                };
                let yrs::Out::YMap(em) = container.get(txn, "em").unwrap() else {
                    panic!("Edge map is not a container: {}", node.id);
                };
                self.pending_edge_map_updates
                    .push((node.id.clone(), edge_id, edge_val));
                non_attached_nodes.remove(&next);
            } else {
                panic!("No valid parent found for detached node: {}", next);
            }
        }
    }

    fn update_children_order(&mut self) {
        let all_node_ids = self
            .nodes
            .values()
            .map(|n| n.id.clone())
            .collect::<Vec<_>>();

        for node_id in all_node_ids.iter() {
            let parent = self.nodes.get(node_id).unwrap().parent_id.clone();
            if let Some(parent_id) = parent {
                let parent = self.nodes.get_mut(&parent_id).unwrap();
                parent.children.push(node_id.clone());
            }
        }

        // Now that the children are set, we need to order them based on the FI
        for node_id in all_node_ids.iter() {
            let node = self.nodes.get(node_id).unwrap();
            let mut children = node.children.clone();
            children.sort_by(|a, b| {
                let node_a = self.nodes.get(a).unwrap();
                let node_b = self.nodes.get(b).unwrap();

                match node_a.fi.cmp(&node_b.fi) {
                    std::cmp::Ordering::Equal => node_a.id.cmp(&node_b.id),
                    ordering => ordering,
                }
            });
            let node = self.nodes.get_mut(node_id).unwrap();
            node.children = children;
        }
    }

    fn can_reach(&self, id: &str, target: &str) -> bool {
        let mut tortoise = id;
        let mut hare = match self.nodes.get(id).and_then(|n| n.parent_id.as_deref()) {
            Some(parent) => parent,
            None => return id == target,
        };

        while hare != target {
            for _ in 0..2 {
                match self.nodes.get(hare).and_then(|n| n.parent_id.as_deref()) {
                    Some(parent) => hare = parent,
                    None => return false,
                }
                if hare == target {
                    return true;
                }
            }

            tortoise = self
                .nodes
                .get(tortoise)
                .and_then(|n| n.parent_id.as_deref())
                .unwrap_or(tortoise);

            if tortoise == hare {
                return false;
            }
        }

        true
    }

    pub(crate) fn update_node(
        &mut self,
        id: &str,
        parent: &str,
        index: Option<usize>,
        map: &MapRef,
        txn: &mut yrs::TransactionMut,
    ) -> Result<(), Box<dyn Error>> {
        // To determine the new fractional index, we need to know the parent's childrens' fractional indices
        let parent_children = self
            .get_node(parent)
            .map(|node| node.children.as_slice())
            .unwrap_or_default();

        // Calculate the new fractional index based on the insertion position
        let mut index = index.unwrap_or(parent_children.len());
        if index > parent_children.len() {
            index = parent_children.len();
        }

        let new_fi = if index == parent_children.len() {
            if let Some(last_id) = parent_children.last() {
                if let Some(last) = self.nodes.get(last_id) {
                    FractionalIndex::new_after(&last.fi)
                } else {
                    FractionalIndex::default()
                }
            } else {
                FractionalIndex::default()
            }
        } else {
            self.nodes
                .get(&parent_children[index])
                .map(|node| node.fi.clone())
                .unwrap_or(FractionalIndex::default())
        };

        if let Some(node) = self.nodes.get_mut(id) {
            // We should calculate our updated edge value from the node's edge map
            // since we might have updated it during the node reattachment phase
            // without updating the backing Yjs map
            let node_edge_map = &mut node.edge_map;
            let (_, new_edge) = node_edge_map.add_edge(parent);
            node.fi = new_fi.clone();

            let Some(Out::YMap(container)) = map.get(txn, id) else {
                panic!("Node {} not found", id);
            };

            let Some(Out::YMap(edge_map)) = container.get(txn, "em") else {
                panic!("Edge map for node {} not found", id);
            };

            edge_map.insert(txn, parent, new_edge);
            container.insert(txn, "fi", new_fi.to_string());
        } else {
            // No existing node; we need to create the container and the node data
            let container = map.insert(txn, id, MapPrelim::default());
            let edge_map = container.insert(txn, "em", MapPrelim::default());

            edge_map.insert(txn, parent, 0);
            container.insert(txn, "fi", new_fi.to_string());
        }

        Ok(())
    }

    pub(crate) fn has_pending_edge_map_updates(&self) -> bool {
        !self.pending_edge_map_updates.is_empty()
    }

    pub(crate) fn apply_pending_edge_map_updates(
        &mut self,
        map: &MapRef,
        txn: &mut yrs::TransactionMut,
    ) {
        for (node_id, edge_id, edge_val) in self.pending_edge_map_updates.iter() {
            let yrs::Out::YMap(container) = map.get(txn, node_id).unwrap() else {
                panic!("Node is not a container: {}", node_id);
            };
            let yrs::Out::YMap(edge_map) = container.get(txn, "em").unwrap() else {
                panic!("Edge map is not a container: {}", node_id);
            };
            edge_map.insert(txn, edge_id.clone(), *edge_val);
        }
        self.pending_edge_map_updates.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{Doc, Map, MapPrelim, Transact};

    fn create_container(
        map: &MapRef,
        txn: &mut yrs::TransactionMut,
        id: &str,
        fi: &FractionalIndex,
        parent_history: Vec<(String, u32)>,
    ) {
        let node = map.insert(txn, id, MapPrelim::default());
        let em = node.insert(txn, "em", MapPrelim::default());
        for (parent, edge) in parent_history {
            em.insert(txn, parent, edge);
        }
        node.insert(txn, "fi", fi.to_string());
    }

    #[test]
    fn handles_initial_data() {
        let doc = Doc::new();
        let map = doc.get_or_insert_map("test");
        let mut txn = doc.transact_mut();

        let fi1 = FractionalIndex::default();
        let fi2 = FractionalIndex::new_after(&fi1);
        let fi3 = FractionalIndex::new_after(&fi2);
        let fi4 = FractionalIndex::new_after(&fi3);

        // using fi2 first to test ordering
        create_container(&map, &mut txn, "1", &fi2, vec![("__ROOT__".to_string(), 0)]);
        create_container(&map, &mut txn, "2", &fi1, vec![("__ROOT__".to_string(), 1)]);
        create_container(&map, &mut txn, "3", &fi3, vec![("1".to_string(), 0)]);
        create_container(&map, &mut txn, "4", &fi4, vec![("2".to_string(), 0)]);
        drop(txn);

        let mut txn = doc.transact_mut();
        let mut tree = TreeStructure::new();
        tree.init_from_yjs(&map, &mut txn);

        assert_eq!(tree.nodes.len(), 5); // 4 nodes + ROOT

        let root = tree.nodes.get("__ROOT__").unwrap();
        assert_eq!(root.children, vec!["2", "1"]);

        let node1 = tree.nodes.get("1").unwrap();
        assert_eq!(node1.parent_id, Some("__ROOT__".to_string()));
        assert_eq!(node1.children, vec!["3"]);
        assert_eq!(node1.fi.to_string(), fi2.to_string());

        let node2 = tree.nodes.get("2").unwrap();
        assert_eq!(node2.parent_id, Some("__ROOT__".to_string()));
        assert_eq!(node2.children, vec!["4"]);
        assert_eq!(node2.fi.to_string(), fi1.to_string());

        let node3 = tree.nodes.get("3").unwrap();
        assert_eq!(node3.parent_id, Some("1".to_string()));
        assert!(node3.children.is_empty());
        assert_eq!(node3.fi.to_string(), fi3.to_string());

        let node4 = tree.nodes.get("4").unwrap();
        assert_eq!(node4.parent_id, Some("2".to_string()));
        assert!(node4.children.is_empty());
        assert_eq!(node4.fi.to_string(), fi4.to_string());
    }

    #[test]
    fn handles_cycles() {
        let doc = Doc::new();
        let map = doc.get_or_insert_map("test");
        let mut txn = doc.transact_mut();

        let fi1 = FractionalIndex::default();
        let fi2 = FractionalIndex::new_after(&fi1);
        let fi3 = FractionalIndex::new_after(&fi2);
        let fi4 = FractionalIndex::new_after(&fi3);

        // using fi2 first to test ordering
        create_container(&map, &mut txn, "1", &fi2, vec![("__ROOT__".to_string(), 0)]);
        create_container(&map, &mut txn, "2", &fi1, vec![("__ROOT__".to_string(), 1)]);
        create_container(
            &map,
            &mut txn,
            "3",
            &fi3,
            vec![("4".to_string(), 1), ("2".to_string(), 0)],
        );
        create_container(
            &map,
            &mut txn,
            "4",
            &fi4,
            vec![("3".to_string(), 1), ("__ROOT__".to_string(), 0)],
        );
        drop(txn);

        let mut txn = doc.transact_mut();
        let mut tree = TreeStructure::new();
        tree.init_from_yjs(&map, &mut txn);

        assert_eq!(tree.nodes.len(), 5); // 4 nodes + ROOT

        let root = tree.nodes.get("__ROOT__").unwrap();
        assert_eq!(root.children, vec!["2", "1"]);

        let node1 = tree.nodes.get("1").unwrap();
        assert_eq!(node1.parent_id, Some("__ROOT__".to_string()));
        assert!(node1.children.is_empty());
        assert_eq!(node1.fi.to_string(), fi2.to_string());

        let node2 = tree.nodes.get("2").unwrap();
        assert_eq!(node2.parent_id, Some("__ROOT__".to_string()));
        assert_eq!(node2.children, vec!["3"]);
        assert_eq!(node2.fi.to_string(), fi1.to_string());

        let node3 = tree.nodes.get("3").unwrap();
        assert_eq!(node3.parent_id, Some("2".to_string()));
        assert_eq!(node3.children, vec!["4"]);
        assert_eq!(node3.fi.to_string(), fi3.to_string());

        let node4 = tree.nodes.get("4").unwrap();
        assert_eq!(node4.parent_id, Some("3".to_string()));
        assert!(node4.children.is_empty());
        assert_eq!(node4.fi.to_string(), fi4.to_string());
    }
}
