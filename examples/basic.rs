use std::{error::Error, sync::Arc};

use yrs_tree::{NodeApi, Tree, TreeUpdateEvent};

fn main() -> Result<(), Box<dyn Error>> {
    let doc = Arc::new(yrs::Doc::new());
    let tree = Tree::new(doc.clone(), "test");

    let _sub = tree.on_change(|e| {
        let TreeUpdateEvent(tree) = e;
        println!("{}", tree);
    });

    println!("Add 1 to ROOT");
    let node1 = tree.create_child_with_id("1")?;
    println!("Add 2 to ROOT");
    let node2 = tree.create_child_with_id("2")?;
    println!("Add 3 to 1");
    let node3 = node1.create_child_with_id("3")?;
    println!("Add 4 to 2");
    let node4 = node2.create_child_with_id("4")?;

    println!("Move 3 to 2, index 0");
    node3.move_to(&node2, Some(0))?;

    println!("Move 1 after 2");
    node1.move_after(&node2)?;

    println!("Move 4 before 3");
    node4.move_before(&node3)?;

    Ok(())
}
