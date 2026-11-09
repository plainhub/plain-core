pub fn device_description(ip: &str, port: u16, uuid: &str, device_name: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <specVersion><major>1</major><minor>0</minor></specVersion>
  <URLBase>http://{ip}:{port}</URLBase>
  <device>
    <deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>
    <friendlyName>{device_name}</friendlyName>
    <manufacturer>PlainApp</manufacturer>
    <modelName>PlainApp MediaRenderer</modelName>
    <modelNumber>1</modelNumber>
    <UDN>uuid:{uuid}</UDN>
    <serviceList>
      <service>
        <serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:AVTransport</serviceId>
        <controlURL>/dlna/av-transport/control</controlURL>
        <eventSubURL>/dlna/av-transport/event</eventSubURL>
        <SCPDURL>/dlna/av-transport/scpd.xml</SCPDURL>
      </service>
      <service>
        <serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:RenderingControl</serviceId>
        <controlURL>/dlna/rendering-control/control</controlURL>
        <eventSubURL>/dlna/rendering-control/event</eventSubURL>
        <SCPDURL>/dlna/rendering-control/scpd.xml</SCPDURL>
      </service>
    </serviceList>
  </device>
</root>"#
    )
}

pub const SCPD_XML: &str = r#"<?xml version="1.0"?>
<scpd xmlns="urn:schemas-upnp-org:service-1-0">
  <specVersion><major>1</major><minor>0</minor></specVersion>
  <actionList></actionList>
  <serviceStateTable></serviceStateTable>
</scpd>"#;

pub const VOLUME_RESPONSE_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:GetVolumeResponse xmlns:u="urn:schemas-upnp-org:service:RenderingControl:1">
      <CurrentVolume>100</CurrentVolume>
    </u:GetVolumeResponse>
  </s:Body>
</s:Envelope>"#;
