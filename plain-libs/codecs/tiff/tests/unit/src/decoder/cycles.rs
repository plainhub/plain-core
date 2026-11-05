use super::*;

#[test]
fn cycles_are_detected() {
    let mut cycles = IfdCycles::new();

    cycles
        .insert_next(IfdPointer(0x20), Some(IfdPointer(0x800)))
        .expect("non-existing link is valid");

    cycles
        .insert_next(IfdPointer(0x800), Some(IfdPointer(0x20)))
        .expect_err("cycle must be detected");
}

#[test]
fn reflective_cycle() {
    let mut cycles = IfdCycles::new();

    cycles
        .insert_next(IfdPointer(0x20), Some(IfdPointer(0x20)))
        .expect_err("self-referential cycle must be detected");
}

#[test]
fn late_cycle() {
    let mut cycles = IfdCycles::new();

    cycles
        .insert_next(IfdPointer(0x20), Some(IfdPointer(0x40)))
        .expect("non-existing link is valid");

    cycles
        .insert_next(IfdPointer(0x60), Some(IfdPointer(0x80)))
        .expect("non-existing link is valid");
    cycles
        .insert_next(IfdPointer(0x80), Some(IfdPointer(0x20)))
        .expect("non-existing link is valid");

    cycles
        .insert_next(IfdPointer(0x40), Some(IfdPointer(0x60)))
        .expect_err("non-existing link is valid");
}

#[test]
fn odd_cycle() {
    let mut cycles = IfdCycles::new();

    cycles
        .insert_next(IfdPointer(0x20), Some(IfdPointer(0x40)))
        .expect("non-existing link is valid");

    cycles
        .insert_next(IfdPointer(0x60), Some(IfdPointer(0x80)))
        .expect("non-existing link is valid");
    cycles
        .insert_next(IfdPointer(0x80), Some(IfdPointer(0x20)))
        .expect("non-existing link is valid");

    cycles
        .insert_next(IfdPointer(0x40), Some(IfdPointer(0x80)))
        .expect_err("non-existing link is valid");
}
