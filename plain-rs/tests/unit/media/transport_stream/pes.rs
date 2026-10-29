use super::*;

fn packet(counter: u8, start: bool, payload: &[u8]) -> [u8; 188] {
    let mut bytes = [0xff; 188];
    bytes[..4].copy_from_slice(&[0x47, if start { 0x40 } else { 0 }, 1, 0x30 | counter]);
    let padding = 183 - payload.len();
    bytes[4] = padding as u8;
    if padding > 0 {
        bytes[5] = 0;
    }
    bytes[188 - payload.len()..].copy_from_slice(payload);
    bytes
}

fn send(consumer: &mut Consumer, counter: u8, start: bool, bytes: &[u8]) {
    let bytes = packet(counter, start, bytes);
    consumer.packet(Packet::read(&bytes).unwrap(), &bytes);
}

const PES: &[u8] = &[
    0, 0, 1, 0xe0, 0, 11, 0x80, 0, 0, 0, 0, 1, 0xb3, 4, 0, 48, 0x23,
];

#[test]
fn split_pes_headers_and_identical_continuations_preserve_the_elementary_stream() {
    let mut consumer = Consumer::new(Codec::Mpeg);
    send(&mut consumer, 0, true, &PES[..10]);
    send(&mut consumer, 1, false, &PES[10..12]);
    send(&mut consumer, 1, false, &PES[10..12]);
    send(&mut consumer, 2, false, &PES[12..]);
    assert_eq!(consumer.video.unwrap().width, 64);
    let mut consumer = Consumer::new(Codec::Mpeg);
    send(&mut consumer, 0, true, &PES[..7]);
    send(&mut consumer, 1, false, &PES[7..]);
    assert_eq!(consumer.video.unwrap().height, 48);
}

#[test]
fn continuity_loss_discards_partial_headers_and_a_new_pes_recovers() {
    let mut consumer = Consumer::new(Codec::Mpeg);
    send(&mut consumer, 0, true, &PES[..7]);
    send(&mut consumer, 2, false, &PES[7..]);
    assert!(consumer.video.is_none());
    assert!(consumer.prefix.is_empty());
    send(&mut consumer, 3, true, PES);
    assert_eq!(consumer.video.unwrap().width, 64);
}

#[test]
fn pes_lengths_exclude_trailing_payload_and_malformed_headers_stay_unknown() {
    let mut truncated = PES.to_vec();
    truncated[5] = 3;
    let mut consumer = Consumer::new(Codec::Mpeg);
    send(&mut consumer, 0, true, &truncated);
    assert!(consumer.video.is_none());
    for change in [3, 7, 8] {
        let mut bytes = PES.to_vec();
        bytes[change] = if change == 3 {
            0xe0
        } else if change == 7 {
            0x40
        } else {
            255
        };
        if change == 3 {
            bytes[0] = 1;
        }
        let mut consumer = Consumer::new(Codec::Mpeg);
        send(&mut consumer, 0, true, &bytes);
        assert!(consumer.video.is_none());
    }
}

#[test]
fn adaptation_only_packets_preserve_counts_but_discontinuity_resets_partial_data() {
    let mut consumer = Consumer::new(Codec::Mpeg);
    send(&mut consumer, 0, true, &PES[..11]);
    let mut adaptation = packet(0, false, &[]);
    adaptation[3] = 0x20;
    consumer.packet(Packet::read(&adaptation).unwrap(), &adaptation);
    send(&mut consumer, 1, false, &PES[11..]);
    assert_eq!(consumer.video.take().unwrap().width, 64);
    send(&mut consumer, 2, true, &PES[..11]);
    adaptation[5] = 0x80;
    consumer.packet(Packet::read(&adaptation).unwrap(), &adaptation);
    send(&mut consumer, 3, false, &PES[11..]);
    assert!(consumer.video.is_none());
}

#[test]
fn empty_or_saturated_prefixes_do_not_rescan_elementary_bytes() {
    let mut consumer = Consumer::new(Codec::Mpeg);
    consumer.push(&[]);
    assert_eq!(consumer.scans, 0);
    consumer.push(&vec![0xff; MAX_ELEMENTARY_BYTES * 2]);
    assert_eq!(consumer.prefix.len(), MAX_ELEMENTARY_BYTES);
    assert_eq!(consumer.scans, 1);
    consumer.push(&[0, 0, 1, 0xb3]);
    consumer.push(&[]);
    assert_eq!(consumer.scans, 1);
}

#[test]
fn maximum_pes_headers_and_unbounded_pes_payloads_keep_both_byte_caps() {
    let mut bytes = vec![0, 0, 1, 0xe0, 0, 0, 0x80, 0, 255];
    bytes.extend(vec![0xff; 255 + MAX_ELEMENTARY_BYTES * 2]);
    let mut consumer = Consumer::new(Codec::Mpeg);
    for (index, bytes) in bytes.chunks(183).enumerate() {
        send(&mut consumer, (index % 16) as u8, index == 0, bytes);
        if let Pes::Header(bytes) = &consumer.pes {
            assert!(bytes.len() <= MAX_HEADER_BYTES);
        }
    }
    assert!(consumer.video.is_none());
    assert_eq!(consumer.prefix.len(), MAX_ELEMENTARY_BYTES);
    assert!(matches!(consumer.pes, Pes::Payload(None)));
}
