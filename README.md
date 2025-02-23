# yrs_tree

A tree CRDT for Yrs, a Rust implementation of Yjs, based on the algorithm described in [Evan Wallace's article on CRDT Mutable Tree Hierarchies](https://madebyevan.com/algos/crdt-mutable-tree-hierarchy/). Changes among clients are guaranteed to converge to a consistent state, and the tree ensures that conflicts and cycles are handled correctly.

## Installation

```bash
cargo add yrs_tree
```

## Usage

[Documentation](https://docs.rs/yrs_tree/)

### Basic Example

```rust
use std::{error::Error, sync::Arc};
use yrs_tree::{Tree, TreeUpdateEvent, NodeApi};

fn main() -> Result<(), Box<dyn Error>> {
    // Create a new Yjs document and tree
    let doc = Arc::new(yrs::Doc::new());
    let tree = Tree::new(doc.clone(), "test");

    // Subscribe to tree changes
    let sub = tree.on_change(|e| {
        let TreeUpdateEvent(tree) = e;
        // Print a textual representation of the tree
        println!("{}", tree);
    });

    // Create and manipulate nodes
    let node1 = tree.create_child_with_id("1")?;
    let node2 = tree.create_child_with_id("2")?;
    let node3 = node1.create_child_with_id("3")?;
    let node4 = node2.create_child_with_id("4")?;

    // Move nodes around
    node3.move_to(&node2, Some(0))?;     // Move node3 as first child of node2
    node1.move_after(&node2)?;           // Move node1 after node2
    node4.move_before(&node3)?;          // Move node4 before node3

    // Iterate over the tree in depth-first order
    let nodes = tree
        .traverse_dfs()
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
```

### Synchronization Between Clients

```rust
use std::{error::Error, sync::Arc};
use yrs_tree::{NodeApi, Tree};
use yrs::{updates::decoder::Decode, ReadTxn, Transact, Update};

fn main() -> Result<(), Box<dyn Error>> {
    // Create two Yjs documents and trees
    let doc1 = Arc::new(yrs::Doc::new());
    let doc2 = Arc::new(yrs::Doc::new());
    
    let tree1 = Tree::new(doc1.clone(), "test");
    let tree2 = Tree::new(doc2.clone(), "test");

    // Make changes to tree1
    let node1 = tree1.create_child_with_id("1")?;
    let node2 = tree1.create_child_with_id("2")?;
    
    // Sync changes to tree2
    let txn = doc1.transact();
    let update = txn.encode_state_as_update_v1(&Default::default());
    drop(txn);

    doc2.transact_mut()
        .apply_update(Update::decode_v1(&update).unwrap())?;

    assert_eq!(tree1, tree2);

    Ok(())
}
```

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.
