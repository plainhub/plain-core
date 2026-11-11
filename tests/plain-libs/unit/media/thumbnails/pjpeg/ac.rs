use super::*;

#[test]
fn grouped_lookup_entry_uses_two_bytes() {
    assert_eq!(std::mem::size_of::<AcGroup>(), 2);
    for bits in 0..=16 {
        for advance in 0..64 {
            for end in [false, true] {
                let entry = AcGroup::new(bits, advance, end);
                assert_eq!(entry.bits(), bits);
                assert_eq!(entry.advance(), advance);
                assert_eq!(entry.end(), end);
            }
        }
    }
}

#[test]
fn grouped_lookup_uses_fixed_sixteen_kib_storage() {
    let mut counts = [0; 16];
    counts[1] = 2;
    let table = AcTable::build(&counts, &[0, 1]).unwrap();
    assert_eq!(
        std::mem::size_of_val(&table.groups),
        std::mem::size_of::<usize>()
    );
    assert_eq!(std::mem::size_of_val(&*table.groups), 16 * 1024);
}

fn encoded(counts: &[u8; 16], values: &[u8], symbols: &[(u8, u16)], offset: usize) -> Vec<u8> {
    let mut codes = [(0u16, 0u8); 256];
    let mut code = 0u16;
    let mut position = 0usize;
    for (n, &count) in counts.iter().enumerate() {
        for _ in 0..count {
            codes[usize::from(values[position])] = (code, n as u8 + 1);
            position += 1;
            code += 1;
        }
        code = code.wrapping_mul(2);
    }
    let mut bits = vec![false; offset];
    let mut emit = |value: u16, width: u8| {
        for bit in (0..width).rev() {
            bits.push((value >> bit) & 1 != 0);
        }
    };
    for &(symbol, value) in symbols {
        let (code, width) = codes[usize::from(symbol)];
        emit(code, width);
        emit(value, symbol & 15);
    }
    emit(0xaff3, 16);
    while bits.len() % 8 != 0 {
        bits.push(true);
    }
    let mut data = Vec::new();
    for byte in bits.chunks_exact(8) {
        let value = byte.iter().fold(0u8, |a, &b| (a << 1) | u8::from(b));
        data.push(value);
        if value == 255 {
            data.push(0);
        }
    }
    data.extend_from_slice(&[255, 217]);
    data
}

fn serial_skip(table: &AcTable, bits: &mut Bits) -> Result<()> {
    let mut coefficient = 1usize;
    while coefficient < 64 {
        let symbol = table.codes.symbol(bits)?;
        if symbol == 0 {
            return Ok(());
        }
        let magnitude = symbol & 15;
        bits.receive(u32::from(magnitude))
            .ok_or_else(|| crate::anyhow::anyhow!("truncated AC"))?;
        coefficient += if symbol == 0xf0 {
            16
        } else {
            usize::from(symbol >> 4) + 1
        };
        if coefficient > 64 {
            bail!("AC exceeds block");
        }
    }
    Ok(())
}

#[test]
fn grouped_ac_preserves_block_boundaries_stuffing_and_long_code_consumption() {
    let mut counts = [0; 16];
    counts[1] = 3;
    counts[10] = 1;
    let values = [0, 1, 0xf0, 0x31];
    let table = AcTable::build(&counts, &values).unwrap();
    let mut dense = vec![(1, 1); 14];
    dense.push((0, 0));
    let mut exact = vec![(0xf0, 0); 3];
    exact.extend(vec![(1, 0); 15]);
    let mut long = vec![(0x31, 1); 12];
    long.push((0, 0));
    GROUP_HITS.set(0);
    for offset in 0..8 {
        for symbols in [&dense, &exact, &long] {
            let data = encoded(&counts, &values, symbols, offset);
            let mut fast = Bits::new(&data, 0);
            let mut serial = Bits::new(&data, 0);
            fast.receive(offset as u32).unwrap();
            serial.receive(offset as u32).unwrap();
            table.skip(&mut fast).unwrap();
            serial_skip(&table, &mut serial).unwrap();
            assert_eq!(fast.pos_after(), serial.pos_after());
            assert_eq!(fast.receive(16), Some(0xaff3));
            assert_eq!(serial.receive(16), Some(0xaff3));
        }
    }
    assert!(GROUP_HITS.get() > 0);
}

#[test]
fn shared_expansion_reuses_exact_tables_and_stays_bounded() {
    let mut cache = AcTables { tables: Vec::new() };
    let mut counts = [0; 16];
    counts[7] = 1;
    let first = cache.get(&counts, &[1]).unwrap();
    assert!(Arc::ptr_eq(&first, &cache.get(&counts, &[1]).unwrap()));
    for i in 1..32u8 {
        cache.get(&counts, &[(i / 10) << 4 | (i % 10 + 1)]).unwrap();
    }
    assert_eq!(cache.tables.len(), 16);
    assert!(Arc::ptr_eq(&first, &cache.get(&counts, &[1]).unwrap()));
    let different = cache.get(&counts, &[2]).unwrap();
    assert!(!Arc::ptr_eq(&first, &different));
}

#[test]
fn invalid_tables_and_truncated_or_overflowing_blocks_are_rejected() {
    let mut counts = [0; 16];
    counts[0] = 3;
    assert!(AcTable::build(&counts, &[0, 1, 2]).is_err());
    counts[0] = 1;
    assert!(AcTable::build(&counts, &[0x10]).is_err());
    assert!(AcTable::build(&counts, &[11]).is_err());
    assert!(AcTable::build(&counts, &[]).is_err());
    let table = AcTable::build(&counts, &[1]).unwrap();
    assert!(table.skip(&mut Bits::new(&[0xff, 0xd9], 0)).is_err());
    let mut counts = [0; 16];
    counts[1] = 2;
    let table = AcTable::build(&counts, &[0, 0xf0]).unwrap();
    let data = encoded(&counts, &[0, 0xf0], &[(0xf0, 0); 4], 0);
    assert!(table.skip(&mut Bits::new(&data, 0)).is_err());
}
