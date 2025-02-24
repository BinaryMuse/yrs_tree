# yrs_tree

[![yrs_tree](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2FBinaryMuse%2Fyrs_tree%2Frefs%2Fheads%2Fmain%2FCargo.toml&query=%24.package.version&prefix=v&label=yrs_tree)](https://crates.io/crates/yrs_tree)
[![docs.rs](https://img.shields.io/docsrs/yrs_tree)](https://docs.rs/yrs_tree)

A tree CRDT for Yrs, a Rust implementation of Yjs, based on the algorithm described in [Evan Wallace's article on CRDT Mutable Tree Hierarchies](https://madebyevan.com/algos/crdt-mutable-tree-hierarchy/). Changes among clients are guaranteed to converge to a consistent state, and the tree ensures that conflicts and cycles are handled correctly.

Each node in the tree has an ID. The root node is always `NodeId::Root`; user-created nodes have IDs of the form `NodeId::Id(String)`. Nodes can be accessed by their ID using the [`Tree::get_node`] method.

Each node can also store and retrieve arbitrary data associated with that node. See the [`Node::set`] and [`Node::get`]/[`Node::get_as`] methods for more information.

## Installation

```bash
cargo add yrs_tree
```

## Documentation

You can [find the complete documentation on Docs.rs](https://docs.rs/yrs_tree/).

Quick links:

* [`Tree` API Documentation](https://docs.rs/yrs_tree/latest/yrs_tree/struct.Tree.html)
* [`Node` API Documentation](https://docs.rs/yrs_tree/latest/yrs_tree/struct.Node.html)
* [`NodeApi` Trait Documentation](https://docs.rs/yrs_tree/latest/yrs_tree/trait.NodeApi.html)

## Tree Poisoning

When the underlying Yrs document is updated, the tree automatically updates its state in response. If the library detects that the Yrs document is malformed in a way that cannot be reconciled, it will mark the tree as "poisoned."

When a tree is poisoned, any operations on the tree that rely on the Yrs document will fail with a `TreePoisoned` error. Operations that only rely on the tree's cached state will continue to succeed, but will not reflect the latest state of the Yrs document.

## Example

```rust
use std::{error::Error, sync::Arc};
use yrs_tree::{NodeApi, Tree, TreeEvent, TraversalOrder};

fn main() -> Result<(), Box<dyn Error>> {
    // Create a new Yjs document and tree
    let doc = Arc::new(yrs::Doc::new());
    let tree = Tree::new(doc.clone(), "test")?;

    // Subscribe to tree changes
    let sub = tree.on_change(|e| {
        match e {
            TreeEvent::TreeUpdated(tree) => {
                // Print a textual representation of the tree
                println!("{}", tree);
            }
            TreeEvent::TreePoisoned(_tree, err) => {
                println!("Tree was poisoned! {}", err);
            }
        }
    });

    // Create and manipulate nodes
    let node1 = tree.create_child_with_id("1")?;
    let node2 = tree.create_child_with_id("2")?;
    let node3 = node1.create_child_with_id("3")?;
    let node4 = node2.create_child_with_id("4")?;

    // Move nodes around
    node3.move_to(&node2, Some(0))?; // Move node3 to be first child of node2
    node1.move_after(&node2)?;       // Move node1 to be after node2
    node4.move_before(&node3)?;      // Move node4 to be before node3

    // Store data on a node
    node1.set("my_key", "my_value")?;
    // Get data from a node
    let val = node1.get_as::<String>("my_key")?;
    assert_eq!(val.as_str(), "my_value");

    // Iterate over the tree in depth-first order
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
```

See the [examples folder in the repository](https://github.com/BinaryMuse/yrs_tree/tree/main/examples) for more.
