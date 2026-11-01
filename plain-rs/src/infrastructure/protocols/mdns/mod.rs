//! mDNS (RFC 6762) protocol stack for `_plainapp._tcp.local` discovery.
//!
//! - [`service_info`]: service model + parsed record accessors
//! - [`host_responder`]: shared 5353 socket, multicast join/send, A-record
//!   hostname responder
//! - [`service_browser`]: browsing state machine that resolves instances into
//!   complete devices

pub mod host_responder;
pub mod service_browser;
pub mod service_info;

pub mod packet_capture;
