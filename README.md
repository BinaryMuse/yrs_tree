# yjs-tree

A CRDT-based tree hierarchy implementation for Yjs, based on the algorithm described in [Evan Wallace's article on CRDT Mutable Tree Hierarchies](https://madebyevan.com/algos/crdt-mutable-tree-hierarchy/).

This library provides a conflict-free replicated data type (CRDT) for managing tree hierarchies that can be modified concurrently by multiple users while maintaining consistency. It's built on top of [Yjs](https://github.com/yjs/yjs).

## Installation

```bash
cargo add yjs_tree
```

## Usage

### Basic Example

```rust
use std::{error::Error, sync::Arc};
use yjs_tree::{Tree, TreeUpdateEvent};

fn main() -> Result<(), Box<dyn Error>> {
    // Create a new Yjs document and tree
    let doc = Arc::new(yrs::Doc::new());
    let tree = Tree::new(doc.clone(), "test");
    let root = tree.root();

    // Subscribe to tree changes
    let sub = tree.on_change(|e| {
        let TreeUpdateEvent(tree) = e;
        // Print a textual representation of the tree
        println!("{}", tree);
    });

    // Create and manipulate nodes
    let node1 = root.create_child_with_id("1")?;
    let node2 = root.create_child_with_id("2")?;
    let node3 = node1.create_child_with_id("3")?;
    let node4 = node2.create_child_with_id("4")?;

    // Move nodes around
    node3.move_to(&node2, Some(0))?;     // Move node3 as first child of node2
    node1.move_after(&node2)?;           // Move node1 after node2
    node4.move_before(&node3)?;          // Move node4 before node3

    Ok(())
}
```

### Synchronization Between Clients

```rust
use std::{error::Error, sync::Arc};
use yjs_tree::Tree;
use yrs::{updates::decoder::Decode, ReadTxn, Transact, Update};

fn main() -> Result<(), Box<dyn Error>> {
    // Create two Yjs documents and trees
    let doc1 = Arc::new(yrs::Doc::new());
    let doc2 = Arc::new(yrs::Doc::new());
    
    let tree1 = Tree::new(doc1.clone(), "test");
    let tree2 = Tree::new(doc2.clone(), "test");

    // Make changes to tree1
    let root1 = tree1.root();
    let node1 = root1.create_child_with_id("1")?;
    let node2 = root1.create_child_with_id("2")?;
    
    // Sync changes to tree2
    let txn = doc1.transact();
    let update = txn.encode_state_as_update_v1(&Default::default());
    drop(txn);

    doc2.transact_mut()
        .apply_update(Update::decode_v1(&update).unwrap())?;

    Ok(())
}
```

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.
