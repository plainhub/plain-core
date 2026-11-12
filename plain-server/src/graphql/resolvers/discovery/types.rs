use plain_graphql::{ComplexObject, SimpleObject};
use plain_rs::features::services::device::types::DeviceType;
use plain_rs::infrastructure::types::Instant;

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct NearbyDevice {
    pub id: plain_graphql::ID,
    pub name: String,
    pub ips: Vec<String>,
    pub port: i32,
    pub remote_access: Option<bool>,
    pub signature_public_key: String,
    pub device_type: DeviceType,
    pub version: String,
    pub platform: String,
    #[graphql(skip)]
    pub last_seen: String,
    pub discovery_methods: Vec<plain_rs::features::discovery::nearby_devices::DiscoveryMethod>,
    pub best_ip: String,
    pub status: plain_rs::features::chat::enums::PeerStatus,
}

impl NearbyDevice {
    pub fn from_snapshot_device(
        device: plain_rs::features::discovery::nearby_devices::Device,
    ) -> Self {
        let interfaces = plain_rs::infrastructure::transport::lan::local_interfaces();
        let best_ip = plain_rs::infrastructure::transport::lan::best(&device.ips, &interfaces);
        Self {
            id: device.id.into(),
            name: device.name,
            ips: device.ips,
            port: device.port as i32,
            device_type: device.device_type.into(),
            version: device.version,
            platform: device.platform,
            last_seen: device.last_seen_at,
            remote_access: device.remote_access,
            signature_public_key: device.signature_public_key,
            discovery_methods: device.discovery_methods,
            best_ip,
            status: plain_rs::features::chat::enums::PeerStatus::Unpaired,
        }
    }
}

#[ComplexObject]
impl NearbyDevice {
    fn last_seen(&self) -> plain_graphql::Result<Instant> {
        crate::graphql::types::parse_instant(&self.last_seen)
    }
}
