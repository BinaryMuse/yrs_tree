use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct Node {
    pub id: String,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub edges: HashMap<String, i64>,
    pub cycle: Option<bool>, // todo
}

impl Node {
    pub fn new(id: String) -> Self {
        Self {
            id,
            parent: None,
            children: Vec::new(),
            edges: HashMap::new(),
            cycle: None,
        }
    }
}
