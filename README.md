# yjs_tree

A Rust library implementing a CRDT-based tree data structure powered by [Yjs](https://yjs.dev/). This implementation is based on the ["Mutable Tree Hierarchy" algorithm by Evan Wallace](https://madebyevan.com/algos/crdt-mutable-tree-hierarchy/).

## Overview

`yjs_tree` provides a conflict-free replicated data type (CRDT) implementation of a tree structure. It allows multiple users to concurrently modify a tree structure (adding, removing, and moving nodes) while automatically resolving conflicts in a consistent way.

The implementation uses Yjs as the underlying CRDT framework and follows Evan Wallace's algorithm for maintaining a mutable tree hierarchy in a distributed system.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
yjs_tree = "0.1.0"
```

## Usage

```rust
use std::sync::Arc;

use yjs_tree::Tree;
use yrs::Doc;

// Create a new Yjs document
let doc = Arc::new(Doc::new());

// Create a new tree
let tree = Tree::new(doc.clone()), "test");

// Add nodes and manipulate the tree
// (More detailed examples coming soon)
```

## How It Works

The library implements the tree CRDT algorithm described in ["CRDT: Mutable Tree Hierarchy"](https://madebyevan.com/algos/crdt-mutable-tree-hierarchy/) by Evan Wallace. The algorithm ensures that concurrent modifications to the tree structure (such as moving nodes) converge to a consistent state across all peers.

Key concepts from the algorithm:

1. **Parent Pointers and Edge Maps**: Each node maintains an edge map containing its historical parent relationships. The map tracks each parent using a counter system to determine the most recent parent relationship.

2. **Cycle Prevention**: The algorithm includes a sophisticated mechanism to prevent cycles in the tree structure. When concurrent operations might create a cycle (for example, if peer A moves node X under Y while peer B moves Y under X), the algorithm consistently resolves these conflicts across all peers.

3. **Reattachment Strategy**: When nodes become disconnected from the root (forming potential cycles), the algorithm uses a consistent strategy to reattach them to the tree. This process considers the edge counters and ensures all peers arrive at the same final tree structure.

4. **Counter-Based Conflict Resolution**: The algorithm uses a counter system to determine which parent relationships take precedence, ensuring that all peers converge to the same tree state even when faced with concurrent modifications.

For a complete technical explanation of the algorithm, please refer to [Evan Wallace's original article](https://madebyevan.com/algos/crdt-mutable-tree-hierarchy/).

## License

Licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Credits

- Algorithm design by [Evan Wallace](https://madebyevan.com/)
- Built on top of [Yjs](https://yjs.dev/)
