use crate::server::ServerState;
use anyhow::Result;
use plain_rs::features::discovery::nearby_wire::Message;
use plain_rs::features::services::discovery::discovery_advertisement;
use plain_rs::features::services::discovery::pairing_runtime;
use plain_rs::infrastructure::protocols::ble;

pub(super) async fn dispatch(
    state: &ServerState,
    address: String,
    bytes: &[u8],
) -> Result<Vec<u8>> {
    let (metadata, body) = ble::parts(bytes)?;
    let body = std::str::from_utf8(body)?;
    let message = match Message::parse(body) {
        Ok(message) => message,
        Err(error) if !metadata.is_empty() => return Err(error),
        Err(_) => return ble::message(&[], &[]),
    };
    anyhow::ensure!(
        metadata.is_empty() || matches!(&message, Message::PairRequest(_)),
        "Invalid Nearby metadata"
    );
    let request = match message {
        Message::Discover => {
            let reply =
                discovery_advertisement::execute(state, discovery_advertisement::Request::Reply {})
                    .await?;
            return ble::message(&[], reply.to_string().as_bytes());
        }
        Message::DiscoverReply(_) => return ble::message(&[], &[]),
        Message::PairVerification(verification) => {
            plain_rs::features::services::discovery::pairing_confirmation::handle(
                state,
                verification,
            )
            .await?;
            return ble::message(&[], &[]);
        }
        Message::PairRequest(payload) => pairing_runtime::Request::ReceiveRequest {
            payload,
            client_id: std::str::from_utf8(metadata)?.to_owned(),
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
