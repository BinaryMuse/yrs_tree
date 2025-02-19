use std::{
    collections::HashMap,
    error::Error,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

use yrs::{In, Map, MapPrelim, Out, Transact};

struct EdgeMap(HashMap<String, i64>);

impl EdgeMap {
    fn new() -> Self {
        Self(HashMap::new())
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

impl Into<yrs::Any> for EdgeMap {
    fn into(self) -> yrs::Any {
        self.0.into()
    }
}

pub(crate) struct YrsTree {
    pub doc: Arc<yrs::Doc>,
    pub map: Arc<RwLock<yrs::MapRef>>,
}

impl YrsTree {
    pub fn new(doc: Arc<yrs::Doc>, map: Arc<RwLock<yrs::MapRef>>) -> Self {
        {
            let mut txn = doc.transact_mut();
            if map.read().unwrap().get(&mut txn, "__ROOT__").is_none() {
                map.write()
                    .unwrap()
                    .insert(&mut txn, "__ROOT__", EdgeMap::new());
            }
        }

        Self { doc, map }
    }

    pub fn set_parent(&self, id: &str, parent: Option<&str>) -> Result<(), Box<dyn Error>> {
        let parent = parent.unwrap_or("__ROOT__");
        {
            let mut txn = self.doc.transact();
            if self.map.read().unwrap().get(&mut txn, parent).is_none() {
                return Err("Parent node does not exist".into());
            }
        }

        let mut txn = self.doc.transact_mut();
        let out = self.map.read().unwrap().get(&mut txn, id);

        if let Some(Out::YMap(edge_map)) = out {
            let edge_values = edge_map.values(&mut txn).flatten().collect::<Vec<_>>();
            println!("edge_values: {:?}", edge_values);

            let max_edge: Vec<i64> = edge_values
                .iter()
                .map(|v| v.clone().cast::<i64>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "Failed to cast edge values to i64".to_string())?;
            let new_edge = max_edge.iter().max().unwrap() + 1;

            if !edge_map.try_update(&mut txn, parent.to_string(), new_edge) {
                println!("UNEXPECTED: Failed to update edge map");
                edge_map.insert(&mut txn, parent.to_string(), new_edge);
            }
        } else {
            let mut edge_map = EdgeMap::new();
            edge_map.insert(parent.to_string(), 0);
            self.map.write().unwrap().insert(
                &mut txn,
                id,
                In::Map(MapPrelim::from_iter(
                    edge_map.iter().map(|(k, v)| (k.clone(), v.clone())),
                )),
            );
        }

        Ok(())
    }
}
