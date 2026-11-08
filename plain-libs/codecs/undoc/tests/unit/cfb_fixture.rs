const FREE: u32 = u32::MAX;
const END: u32 = FREE - 1;
const FAT: u32 = FREE - 2;
const SECTOR: usize = 512;
const MINI: usize = 64;

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn append_chain(data: &mut Vec<u8>, links: &mut Vec<u32>, payload: &[u8], block: usize) -> u32 {
    let count = payload.len().div_ceil(block);
    if count == 0 {
        return END;
    }
    let start = links.len() as u32;
    for offset in 1..count {
        links.push(start + offset as u32);
    }
    links.push(END);
    data.extend_from_slice(payload);
    data.resize(links.len() * block, 0);
    start
}

fn write_entry(
    entry: &mut [u8],
    name: &str,
    kind: u8,
    right: u32,
    child: u32,
    start: u32,
    len: usize,
) {
    let mut size = 0;
    for unit in name.encode_utf16().chain([0]) {
        write_u16(entry, size, unit);
        size += 2;
    }
    assert!(size <= 64);
    write_u16(entry, 64, size as u16);
    entry[66] = kind;
    entry[67] = 1;
    write_u32(entry, 68, FREE);
    write_u32(entry, 72, right);
    write_u32(entry, 76, child);
    write_u32(entry, 116, start);
    write_u32(entry, 120, u32::try_from(len).unwrap());
}

pub(crate) fn compound<'a>(streams: impl IntoIterator<Item = (&'a str, &'a [u8])>) -> Vec<u8> {
    let mut streams: Vec<_> = streams.into_iter().collect();
    for (name, _) in &mut streams {
        *name = name.trim_start_matches('/');
        assert!(name.is_ascii());
    }
    streams.sort_unstable_by_key(|(name, _)| (name.len(), name.to_ascii_uppercase()));

    let mut data = Vec::new();
    let mut links = Vec::new();
    let mut mini_data = Vec::new();
    let mut mini_links = Vec::new();
    let mut directory = vec![0; (streams.len() + 1).div_ceil(4) * SECTOR];
    for (index, (name, bytes)) in streams.iter().enumerate() {
        let start = if bytes.len() < 4096 {
            append_chain(&mut mini_data, &mut mini_links, bytes, MINI)
        } else {
            append_chain(&mut data, &mut links, bytes, SECTOR)
        };
        let right = if index + 1 < streams.len() {
            index as u32 + 2
        } else {
            FREE
        };
        let offset = 128 * (index + 1);
        write_entry(
            &mut directory[offset..][..128],
            name,
            2,
            right,
            FREE,
            start,
            bytes.len(),
        );
    }
    let mini_start = append_chain(&mut data, &mut links, &mini_data, SECTOR);
    let mini_fat_count = mini_links.len().div_ceil(SECTOR / 4);
    mini_links.resize(mini_fat_count * (SECTOR / 4), FREE);
    let mini_fat: Vec<u8> = mini_links.into_iter().flat_map(u32::to_le_bytes).collect();
    let mini_fat_start = append_chain(&mut data, &mut links, &mini_fat, SECTOR);
    write_entry(
        &mut directory[..128],
        "Root Entry",
        5,
        FREE,
        if streams.is_empty() { FREE } else { 1 },
        mini_start,
        mini_data.len(),
    );
    let directory_start = append_chain(&mut data, &mut links, &directory, SECTOR);

    let fat_start = links.len();
    let fat_count = links.len().div_ceil(SECTOR / 4 - 1);
    assert!(fat_count <= 109);
    links.resize(fat_start + fat_count, FAT);
    links.resize(fat_count * (SECTOR / 4), FREE);
    let mut header = [0; SECTOR];
    header[..8].copy_from_slice(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
    write_u16(&mut header, 24, 0x3e);
    write_u16(&mut header, 26, 3);
    write_u16(&mut header, 28, 0xfffe);
    write_u16(&mut header, 30, 9);
    write_u16(&mut header, 32, 6);
    write_u32(&mut header, 44, fat_count as u32);
    write_u32(&mut header, 48, directory_start);
    write_u32(&mut header, 56, 4096);
    write_u32(&mut header, 60, mini_fat_start);
    write_u32(&mut header, 64, mini_fat_count as u32);
    write_u32(&mut header, 68, END);
    for index in 0..109 {
        let sector = if index < fat_count {
            (fat_start + index) as u32
        } else {
            FREE
        };
        write_u32(&mut header, 76 + index * 4, sector);
    }
    let mut bytes = header.to_vec();
    bytes.extend_from_slice(&data);
    bytes.extend(links.into_iter().flat_map(u32::to_le_bytes));
    bytes
}
