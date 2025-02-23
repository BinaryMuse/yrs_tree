use std::{error::Error, sync::Arc};

use yrs_tree::{NodeApi, Tree, TreeEvent};

fn main() -> Result<(), Box<dyn Error>> {
    let doc = Arc::new(yrs::Doc::new());
    let tree = Tree::new(doc.clone(), "test")?;

    let _sub = tree.on_change(|e| match e {
        TreeEvent::TreeUpdated(tree) => {
            println!("{}", tree);
        }
        TreeEvent::TreePoisoned(_tree, err) => {
            println!("Tree is poisoned: {}", err);
        }
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

    println!("Set data on 1: my_key = my_value");
    node1.set("my_key", "my_value")?;

    let val = node1.get_as::<String>("my_key")?;
    println!("Get data from 1: my_key = {}", val);

    Ok(())
}
