use super::*;
#[test]
fn set_sentence() {
    let lattice = Lattice::from("", 1, 2);

    assert_eq!(lattice.len(), 0);

    let lattice = Lattice::from("", 1, 2);
    assert_eq!(lattice.len(), 0);
    assert_eq!(lattice.sentence(), "");
    assert_eq!(lattice.surface(0), "");

    let lattice = Lattice::from("test", 1, 2);
    assert_eq!(lattice.len(), 4);
    assert_eq!(lattice.sentence(), "test");
    assert_eq!(lattice.surface(0), "test");
    assert_eq!(lattice.surface(1), "est");
    assert_eq!(lattice.surface(2), "st");
    assert_eq!(lattice.surface(3), "t");

    let bos = lattice.bos_node();
    let eos = lattice.eos_node();

    assert_eq!(bos.borrow().id, 1);
    assert_eq!(eos.borrow().id, 2);
    assert_eq!(
        lattice.end_nodes[0].first().unwrap().borrow().id,
        bos.borrow().id
    );
    assert_eq!(
        lattice.begin_nodes[4].first().unwrap().borrow().id,
        eos.borrow().id
    );

    let lattice = Lattice::from("テストab", 1, 2);
    assert_eq!(lattice.len(), 11);
    assert_eq!(lattice.sentence(), "テストab");
    assert_eq!(lattice.surface(0), "テストab");
    assert_eq!(lattice.surface(1), "ストab");
    assert_eq!(lattice.surface(2), "トab");
    assert_eq!(lattice.surface(3), "ab");
    assert_eq!(lattice.surface(4), "b");
}

#[test]
fn insert_test() {
    let mut lattice = Lattice::from("ABあい", 1, 2);

    lattice.insert(0, 1, 0.0, 3);
    lattice.insert(1, 1, 0.0, 4);
    lattice.insert(2, 3, 0.0, 5);
    lattice.insert(5, 3, 0.0, 6);
    lattice.insert(0, 2, 0.0, 7);
    lattice.insert(1, 4, 0.0, 8);
    lattice.insert(2, 6, 0.0, 9);
    let node0 = lattice.begin_nodes[0][0].borrow();
    let node1 = lattice.begin_nodes[1][0].borrow();
    let node2 = lattice.begin_nodes[2][0].borrow();
    let node3 = lattice.begin_nodes[5][0].borrow();
    let node4 = lattice.begin_nodes[0][1].borrow();
    let node5 = lattice.begin_nodes[1][1].borrow();
    let node6 = lattice.begin_nodes[2][1].borrow();

    assert_eq!(lattice.piece(&node0), "A");
    assert_eq!(lattice.piece(&node1), "B");
    assert_eq!(lattice.piece(&node2), "あ");
    assert_eq!(lattice.piece(&node3), "い");
    assert_eq!(lattice.piece(&node4), "AB");
    assert_eq!(lattice.piece(&node5), "Bあ");
    assert_eq!(lattice.piece(&node6), "あい");

    assert_eq!(node0.pos, 0);
    assert_eq!(node1.pos, 1);
    assert_eq!(node2.pos, 2);
    assert_eq!(node3.pos, 5);
    assert_eq!(node4.pos, 0);
    assert_eq!(node5.pos, 1);
    assert_eq!(node6.pos, 2);

    assert_eq!(node0.length, 1);
    assert_eq!(node1.length, 1);
    assert_eq!(node2.length, 3);
    assert_eq!(node3.length, 3);
    assert_eq!(node4.length, 2);
    assert_eq!(node5.length, 4);
    assert_eq!(node6.length, 6);

    assert_eq!(lattice.bos_node().borrow().id, 1);
    assert_eq!(lattice.eos_node().borrow().id, 2);
    assert_eq!(node0.id, 3);
    assert_eq!(node1.id, 4);
    assert_eq!(node2.id, 5);
    assert_eq!(node3.id, 6);
    assert_eq!(node4.id, 7);
    assert_eq!(node5.id, 8);
    assert_eq!(node6.id, 9);

    assert_eq!(lattice.begin_nodes[0].len(), 2);
    assert_eq!(lattice.begin_nodes[1].len(), 2);
    assert_eq!(lattice.begin_nodes[2].len(), 2);
    assert_eq!(lattice.begin_nodes[5].len(), 1);
    assert_eq!(lattice.begin_nodes[8].len(), 1);

    assert_eq!(lattice.end_nodes[0].len(), 1);
    assert_eq!(lattice.end_nodes[1].len(), 1);
    assert_eq!(lattice.end_nodes[2].len(), 2);
    assert_eq!(lattice.end_nodes[5].len(), 2);
    assert_eq!(lattice.end_nodes[8].len(), 2);

    assert_eq!(lattice.begin_nodes[0][0].borrow().id, node0.id);
    assert_eq!(lattice.begin_nodes[0][1].borrow().id, node4.id);
    assert_eq!(lattice.begin_nodes[1][0].borrow().id, node1.id);
    assert_eq!(lattice.begin_nodes[1][1].borrow().id, node5.id);
    assert_eq!(lattice.begin_nodes[2][0].borrow().id, node2.id);
    assert_eq!(lattice.begin_nodes[2][1].borrow().id, node6.id);
    assert_eq!(lattice.begin_nodes[5][0].borrow().id, node3.id);
    assert_eq!(
        lattice.eos_node().borrow().id,
        lattice.begin_nodes[8][0].borrow().id
    );

    assert_eq!(
        lattice.bos_node().borrow().id,
        lattice.end_nodes[0][0].borrow().id
    );
    assert_eq!(node0.id, lattice.end_nodes[1][0].borrow().id);
    assert_eq!(node1.id, lattice.end_nodes[2][0].borrow().id);
    assert_eq!(node4.id, lattice.end_nodes[2][1].borrow().id);
    assert_eq!(node2.id, lattice.end_nodes[5][0].borrow().id);
    assert_eq!(node5.id, lattice.end_nodes[5][1].borrow().id);
    assert_eq!(node3.id, lattice.end_nodes[8][0].borrow().id);
    assert_eq!(node6.id, lattice.end_nodes[8][1].borrow().id);
}

#[test]
fn test_viterbi() {
    let mut lattice = Lattice::from("ABC", 1, 2);
    assert_eq!(lattice.viterbi(), vec![]);
    // Still incomplete
    lattice.insert(0, 1, 0.0, 3);
    assert_eq!(lattice.viterbi(), vec![]);
    lattice.insert(1, 1, 0.0, 4);
    lattice.insert(2, 1, 0.0, 5);
    // XXX: In sentence piece this is not tested, still incomplete ?
    assert_eq!(lattice.viterbi().len(), 3);
}

#[test]
fn test_viterbi2() {
    let mut lattice = Lattice::from("ABC", 1, 2);

    lattice.insert(0, 1, 0.0, 3);
    lattice.insert(1, 1, 0.0, 4);
    lattice.insert(2, 1, 0.0, 5);

    assert_eq!(lattice.tokens(), ["A", "B", "C"]);

    lattice.insert(0, 2, 2.0, 6);
    assert_eq!(lattice.tokens(), ["AB", "C"]);

    lattice.insert(1, 2, 5.0, 7);
    assert_eq!(lattice.tokens(), ["A", "BC"]);

    lattice.insert(0, 3, 10.0, 8);
    assert_eq!(lattice.tokens(), ["ABC"]);
}
