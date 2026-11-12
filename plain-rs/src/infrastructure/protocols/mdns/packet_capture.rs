use plain_libs::protocols::mdns::packet_codec;
use plain_libs::protocols::mdns::records::MdnsRecord;
use serde::Serialize;
use std::{
    collections::VecDeque,
    net::SocketAddr,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
pub enum MdnsPacketDirection {
    In,
    Out,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacketLog {
    time: crate::infrastructure::types::Instant,
    direction: MdnsPacketDirection,
    src_ip: String,
    src_port: u16,
    dst_ip: String,
    dst_port: u16,
    size: usize,
    summary: String,
    detail: String,
}
#[derive(Default)]
struct Capture {
    inbound: VecDeque<PacketLog>,
    outbound: VecDeque<PacketLog>,
}
impl Capture {
    fn push(&mut self, row: PacketLog) {
        let buffer = if row.direction == MdnsPacketDirection::In {
            &mut self.inbound
        } else {
            &mut self.outbound
        };
        buffer.push_front(row);
        buffer.truncate(50);
    }
}
static ENABLED: AtomicBool = AtomicBool::new(false);
static CAPTURE: Mutex<Capture> = Mutex::new(Capture {
    inbound: VecDeque::new(),
    outbound: VecDeque::new(),
});

pub fn set_enabled(enabled: bool) {
    let mut state = CAPTURE.lock().unwrap();
    ENABLED.store(enabled, Ordering::SeqCst);
    if !enabled {
        *state = Capture::default();
    }
}
pub fn snapshot() -> (Vec<PacketLog>, Vec<PacketLog>) {
    let state = CAPTURE.lock().unwrap();
    (
        state.inbound.iter().cloned().collect(),
        state.outbound.iter().cloned().collect(),
    )
}
pub(crate) fn record(
    direction: MdnsPacketDirection,
    src: SocketAddr,
    dst: SocketAddr,
    bytes: &[u8],
) {
    if !ENABLED.load(Ordering::SeqCst) {
        return;
    }
    let (summary, detail) = decode(bytes);
    let row = PacketLog {
        time: crate::infrastructure::types::Instant(plain_libs::chrono::Utc::now()),
        direction,
        src_ip: src.ip().to_string(),
        src_port: src.port(),
        dst_ip: dst.ip().to_string(),
        dst_port: dst.port(),
        size: bytes.len(),
        summary,
        detail,
    };
    let mut state = CAPTURE.lock().unwrap();
    if !ENABLED.load(Ordering::SeqCst) {
        return;
    }
    state.push(row);
}
fn type_name(kind: u16) -> String {
    match kind {
        1 => "A".into(),
        12 => "PTR".into(),
        16 => "TXT".into(),
        28 => "AAAA".into(),
        33 => "SRV".into(),
        255 => "ANY".into(),
        _ => format!("TYPE{kind:x}"),
    }
}
fn record_line(row: &MdnsRecord) -> String {
    let value = match row.record_type {
        12 => row.ptr_target().unwrap_or_default(),
        33 => row
            .srv()
            .map(|r| format!("{} : {}", r.target, r.port))
            .unwrap_or_default(),
        16 => row.txt_strings().unwrap_or_default().join(", "),
        1 => row.ip().unwrap_or_default(),
        28 => row.ipv6().unwrap_or_default(),
        _ => String::new(),
    };
    format!(
        "{} {} {} ttl={}",
        row.name,
        type_name(row.record_type),
        value,
        row.ttl
    )
}
fn decode(bytes: &[u8]) -> (String, String) {
    let questions = packet_codec::read_questions(bytes).unwrap_or_default();
    let parsed = packet_codec::parse_response(bytes);
    let summary = match &parsed {
        Some(response) if response.is_response() => format!(
            "response {}ans/{}add",
            response.answers.len(),
            response.additional.len()
        ),
        _ => questions
            .first()
            .map(|q| format!("query {} {}", type_name(q.qtype), q.name))
            .unwrap_or("packet".into()),
    };
    let mut lines = questions
        .iter()
        .map(|q| {
            format!(
                "Q {} {}{}",
                q.name,
                type_name(q.qtype),
                if q.unicast_response_requested {
                    " QU"
                } else {
                    ""
                }
            )
        })
        .collect::<Vec<_>>();
    if let Some(response) = parsed {
        lines.extend(
            response
                .answers
                .iter()
                .chain(&response.additional)
                .map(record_line),
        );
    }
    if lines.is_empty() {
        lines.push("(unparseable)".into());
    }
    (summary, lines.join("\n"))
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/protocols/mdns/packet_capture.rs"]
mod tests;
