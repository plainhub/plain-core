use crate::server::ServerState;
use anyhow::Result;
use plain_rs::discovery::nearby_wire::Message;
use plain_rs::protocols::ble;
use plain_rs::services::discovery::discovery_advertisement;
use plain_rs::services::discovery::pairing_runtime;

pub(super) async fn dispatch(
    state: &ServerState,
    address: String,
    bytes: &[u8],
) -> Result<Vec<u8>> {
    let body = std::str::from_utf8(ble::nearby_body(bytes)?)?;
    let message = match Message::parse(body) {
        Ok(message) => message,
        Err(_) => return ble::message(&[], &[]),
    };
    let request = match message {
        Message::Discover => {
            let reply =
                discovery_advertisement::execute(state, discovery_advertisement::Request::Reply {})
                    .await?;
            return ble::message(&[], reply.to_string().as_bytes());
        }
        Message::DiscoverReply(_) => return ble::message(&[], &[]),
        Message::PairVerification(verification) => {
            plain_rs::services::discovery::pairing_confirmation::handle(state, verification)
                .await?;
            return ble::message(&[], &[]);
        }
        Message::PairRequest(request) => pairing_runtime::Request::ReceiveRequest {
            request,
            address,
            ble: true,
        },
        Message::PairResponse(response) => pairing_runtime::Request::Complete {
            response,
            sender_ip: address,
        },
        Message::PairCancel(cancel) => pairing_runtime::Request::ReceiveCancel { cancel },
    };
    pairing_runtime::execute(state, request).await?;
    ble::message(&[], b"1")
}
