use std::{
    error::Error,
    sync::{Arc, RwLock},
};

use yrs::DeepObservable;

use crate::tree_state::TreeState;

pub struct Tree {
    pub(crate) state: Arc<RwLock<TreeState>>,
    pub(crate) _subscription: yrs::Subscription,
}

impl Tree {
    pub fn new(doc: Arc<yrs::Doc>, tree_name: &str) -> Self {
        let state = Arc::new(RwLock::new(TreeState::new(doc, tree_name)));

        let map = state.read().unwrap().inner.map.clone();

        let state_clone = state.clone();
        let subscription = map.read().unwrap().observe_deep(move |txn, evts| {
            state_clone.read().unwrap().handle_events(txn, evts);
        });

        Self {
            state,
            _subscription: subscription,
        }
    }

    pub fn add_node(&self, id: &str, parent: Option<&str>) -> Result<(), Box<dyn Error>> {
        self.state.read().unwrap().set_parent(id, parent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() -> Result<(), Box<dyn Error>> {
        let doc = yrs::Doc::new();
        let tree = Tree::new(Arc::new(doc), "test");
        tree.add_node("a", None)?;
        tree.add_node("b", Some("a"))?;
        tree.add_node("c", Some("a"))?;
        tree.add_node("d", Some("a"))?;
        tree.add_node("d", Some("b"))?;
        tree.add_node("d", Some("c"))?;
        tree.add_node("d", Some("b"))?;
        tree.add_node("d", Some("a"))?;

        println!("{:?}", tree.state.read().unwrap().nodes.read().unwrap());

        // let ser = tree.structure();
        // assert_eq!(ser, ("__ROOT__", vec![("a", vec!["b", "c"])]));

        Ok(())
    }
}
