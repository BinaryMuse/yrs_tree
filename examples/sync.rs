use std::{error::Error, sync::Arc};

use yrs::{updates::decoder::Decode, ReadTxn, Transact, Update};
use yrs_tree::{NodeApi, Tree};

fn main() -> Result<(), Box<dyn Error>> {
    let doc1 = Arc::new(yrs::Doc::new());
    let doc2 = Arc::new(yrs::Doc::new());

    let tree1 = Tree::new(doc1.clone(), "test");
    let tree2 = Tree::new(doc2.clone(), "test");

    let node1 = tree1.create_child_with_id("1")?;
    let node2 = tree1.create_child_with_id("2")?;
    let node3 = node1.create_child_with_id("3")?;
    let node4 = node2.create_child_with_id("4")?;
    node3.move_to(&node2, Some(0))?;
    node1.move_after(&node2)?;
    node4.move_before(&node3)?;

    println!("Tree 1: \n{}", tree1);
    println!("Syncing Yjs documents...\n");

    let txn = doc1.transact();
    let update = txn.encode_state_as_update_v1(&Default::default());
    drop(txn);

    doc2.transact_mut()
        .apply_update(Update::decode_v1(&update).unwrap())?;

    println!("Tree 2: \n{}", tree2);

    Ok(())
}
