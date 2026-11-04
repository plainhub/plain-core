use plain_graphql_macros::SimpleObject;

#[derive(SimpleObject, Clone, Debug)]
#[graphql(getters = false)]
pub struct NearbyDevice {
    pub id: async_graphql::ID,
    pub name: String,
    pub ips: Vec<String>,
    pub port: i32,
    pub device_type: String,
    pub version: String,
    pub platform: String,
    pub last_seen: String,
    pub discovery_methods: Vec<String>,
    pub best_ip: String,
    pub status: String,
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
            device_type: device.device_type,
            version: device.version,
            platform: device.platform,
            last_seen: device.last_seen_at,
            discovery_methods: device.discovery_methods,
            best_ip,
            status: "UNPAIRED".to_string(),
        }
    }
}
