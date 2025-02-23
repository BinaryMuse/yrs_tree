use std::{error::Error, sync::Arc};

use yrs::{updates::decoder::Decode, ReadTxn, Transact, Update};
use yrs_tree::{NodeApi, Tree};

fn main() -> Result<(), Box<dyn Error>> {
    let doc1 = Arc::new(yrs::Doc::new());
    let doc2 = Arc::new(yrs::Doc::new());

    let tree1 = Tree::new(doc1.clone(), "test")?;
    let tree2 = Tree::new(doc2.clone(), "test")?;

    let node1 = tree1.create_child_with_id("1")?;
    let node2 = tree1.create_child_with_id("2")?;
    let node3 = node1.create_child_with_id("3")?;
    let node4 = node2.create_child_with_id("4")?;
    node3.move_to(&node2, Some(0))?;
    node1.move_after(&node2)?;
    node4.move_before(&node3)?;

    sync_docs(&doc1, &doc2)?;

    // Simulate a cycle created by disparate clients
    let node3_left = tree1.get_node("3").unwrap();
    let node4_left = tree1.get_node("4").unwrap();
    node3_left.move_to(&node4_left, None)?;

    let node3_right = tree2.get_node("3").unwrap();
    let node4_right = tree2.get_node("4").unwrap();
    node4_right.move_to(&node3_right, None)?;

    println!("Tree 1: \n{}", tree1);
    println!("Tree 2: \n{}", tree2);

    println!("Syncing docs...\n");
    sync_docs(&doc1, &doc2)?;

    println!("Tree 1: \n{}", tree1);
    println!("Tree 2: \n{}", tree2);

    Ok(())
}

fn sync_docs(doc1: &yrs::Doc, doc2: &yrs::Doc) -> Result<(), Box<dyn Error>> {
    let mut txn1 = doc1.transact_mut();
    let sv1 = txn1.state_vector();

    let mut txn2 = doc2.transact_mut();
    let sv2 = txn2.state_vector();

    let update1 = txn1.encode_diff_v1(&sv2);
    let update2 = txn2.encode_diff_v1(&sv1);

    txn1.apply_update(Update::decode_v1(&update2).unwrap())?;
    txn2.apply_update(Update::decode_v1(&update1).unwrap())?;

    Ok(())
}
