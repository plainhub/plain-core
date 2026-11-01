//! Builds mDNS responses for the published `MdnsServiceInfo`.
//!
//! A PTR query for the service type is answered with the PTR record plus A
//! records (additional), so a browser learns the instance name and its
//! address in one shot. SRV / TXT / A queries are answered with the matching
//! records.

use crate::protocols::mdns::packet_codec;
use crate::protocols::mdns::packet_codec::DNS_CACHE_FLUSH_CLASS_IN;
use crate::protocols::mdns::packet_codec::DNS_CLASS_IN;
use crate::protocols::mdns::packet_codec::MdnsQuestion;
use crate::protocols::mdns::packet_codec::TTL_SECONDS;
use crate::protocols::mdns::packet_codec::TYPE_A;
use crate::protocols::mdns::packet_codec::TYPE_ANY;
use crate::protocols::mdns::packet_codec::TYPE_PTR;
use crate::protocols::mdns::packet_codec::TYPE_SRV;
use crate::protocols::mdns::packet_codec::TYPE_TXT;
use crate::protocols::mdns::records::MdnsServiceInfo;

pub struct MdnsServiceResponse {
    pub bytes: Vec<u8>,
}

pub fn build_response_if_match(
    query: &[u8],
    service: &MdnsServiceInfo,
) -> Option<MdnsServiceResponse> {
    if service.ips.is_empty() {
        return None;
    }
    let questions: Vec<MdnsQuestion> = packet_codec::read_questions(query)?;
    let instance_fqdn = service.instance_fqdn();

    let mut want_ptr = false;
    let mut want_srv = false;
    let mut want_txt = false;
    let mut want_a = false;
    for q in &questions {
        if q.qclass != DNS_CLASS_IN {
            continue;
        }
        let matches_type = q.name.eq_ignore_ascii_case(&service.service_type);
        let matches_instance = q.name.eq_ignore_ascii_case(&instance_fqdn);
        let matches_hostname = q.name.eq_ignore_ascii_case(&service.target_hostname);
        if q.qtype == TYPE_PTR && matches_type {
            want_ptr = true;
        } else if q.qtype == TYPE_SRV && matches_instance {
            want_srv = true;
        } else if q.qtype == TYPE_TXT && matches_instance {
            want_txt = true;
        } else if q.qtype == TYPE_A && matches_hostname {
            want_a = true;
        } else if q.qtype == TYPE_ANY && (matches_type || matches_instance || matches_hostname) {
            // RFC 6762 §6: only answer ANY with records whose name matches
            // the question — otherwise we'd pollute other mDNS stacks'
            // caches with answers unrelated to the queried name.
            want_ptr = want_ptr || matches_type;
            want_srv = want_srv || matches_instance;
            want_txt = want_txt || matches_instance;
            want_a = want_a || matches_hostname;
        }
    }
    if !want_ptr && !want_srv && !want_txt && !want_a {
        return None;
    }

    let mut answers: Vec<u8> = Vec::new();
    let mut additional: Vec<u8> = Vec::new();
    if want_ptr {
        answers.extend_from_slice(&ptr_record(service, TTL_SECONDS));
    }
    if want_srv {
        answers.extend_from_slice(&srv_record(service, TTL_SECONDS));
    }
    if want_txt {
        answers.extend_from_slice(&txt_record(service, TTL_SECONDS));
    }
    if want_a {
        answers.extend_from_slice(&a_records(service));
    }
    // RFC 6763 §12: a PTR answer carries SRV/TXT/A in the additional section
    // so a single PTR query resolves the full service — the port is learned
    // in one round trip instead of a follow-up SRV query.
    let mut additional_count = 0usize;
    if want_ptr {
        if !want_srv {
            additional.extend_from_slice(&srv_record(service, TTL_SECONDS));
            additional_count += 1;
        }
        if !want_txt {
            additional.extend_from_slice(&txt_record(service, TTL_SECONDS));
            additional_count += 1;
        }
        if !want_a {
            additional.extend_from_slice(&a_records(service));
            additional_count += service.ips.len();
        }
    } else if (want_srv || want_txt) && !want_a {
        additional.extend_from_slice(&a_records(service));
        additional_count += service.ips.len();
    }
    if answers.is_empty() {
        return None;
    }

    // Each PTR/SRV/TXT is a single record; A records are one per IP.
    let answer_count = (want_ptr as usize)
        + (want_srv as usize)
        + (want_txt as usize)
        + if want_a { service.ips.len() } else { 0 };

    let mut out = Vec::new();
    packet_codec::write_header(&mut out, answer_count, additional_count);
    out.extend_from_slice(&answers);
    out.extend_from_slice(&additional);
    Some(MdnsServiceResponse { bytes: out })
}

fn ptr_record(service: &MdnsServiceInfo, ttl: u32) -> Vec<u8> {
    let mut out = Vec::new();
    packet_codec::write_record(
        &mut out,
        &packet_codec::encode_name(&service.service_type),
        TYPE_PTR,
        // RFC 6762 §10.2: the cache-flush bit is only for unique records
        // (SRV/TXT/A). PTR rnames are shared by all instances of the type,
        // so flushing would evict other devices' PTR entries from peers.
        DNS_CLASS_IN,
        ttl,
        &packet_codec::encode_name(&service.instance_fqdn()),
    );
    out
}

fn srv_record(service: &MdnsServiceInfo, ttl: u32) -> Vec<u8> {
    let mut rdata = Vec::new();
    packet_codec::write_u16(&mut rdata, 0); // priority
    packet_codec::write_u16(&mut rdata, 0); // weight
    packet_codec::write_u16(&mut rdata, service.port);
    rdata.extend_from_slice(&packet_codec::encode_name(&service.target_hostname));
    let mut out = Vec::new();
    packet_codec::write_record(
        &mut out,
        &packet_codec::encode_name(&service.instance_fqdn()),
        TYPE_SRV,
        DNS_CACHE_FLUSH_CLASS_IN,
        ttl,
        &rdata,
    );
    out
}

fn txt_record(service: &MdnsServiceInfo, ttl: u32) -> Vec<u8> {
    let mut rdata = Vec::new();
    for value in &service.txt_records {
        let bytes = value.as_bytes();
        rdata.push(bytes.len() as u8);
        rdata.extend_from_slice(bytes);
    }
    let mut out = Vec::new();
    packet_codec::write_record(
        &mut out,
        &packet_codec::encode_name(&service.instance_fqdn()),
        TYPE_TXT,
        DNS_CACHE_FLUSH_CLASS_IN,
        ttl,
        &rdata,
    );
    out
}

/// Builds the RFC 6762 §8.4 goodbye for one instance: the same PTR/SRV/TXT
/// records with TTL=0, so every resolver on the link drops the cached entry
/// at once. Needed when a published instance is replaced by one with a
/// different instance FQDN (device renamed) — without it peers keep listing
/// the old name until the 120s TTL expires. Mirrors the Kotlin
/// `MdnsServiceResponseBuilder.buildGoodbye`.
pub fn build_goodbye(service: &MdnsServiceInfo) -> Vec<u8> {
    let mut out = Vec::new();
    packet_codec::write_header(&mut out, 3, 0);
    out.extend_from_slice(&ptr_record(service, 0));
    out.extend_from_slice(&srv_record(service, 0));
    out.extend_from_slice(&txt_record(service, 0));
    out
}

fn a_records(service: &MdnsServiceInfo) -> Vec<u8> {
    let name_bytes = packet_codec::encode_name(&service.target_hostname);
    let mut out = Vec::new();
    for ip in &service.ips {
        packet_codec::write_record(
            &mut out,
            &name_bytes,
            TYPE_A,
            DNS_CACHE_FLUSH_CLASS_IN,
            TTL_SECONDS,
            &packet_codec::ip_to_bytes(ip),
        );
    }
    out
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/protocols/mdns/service_response_builder.rs"]
mod tests;
