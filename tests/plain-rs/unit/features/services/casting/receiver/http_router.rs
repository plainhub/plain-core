use super::*;
use std::collections::HashMap;

async fn request(method: &str, path: &str) -> DlnaHttpResponse {
    let state = Arc::new(RwLock::new(DlnaRendererState::default()));
    let (commands, _) = tokio::sync::mpsc::unbounded_channel();
    route(
        &state,
        method,
        path,
        &HashMap::new(),
        "",
        "test-device",
        "Test Renderer",
        "192.0.2.1",
        "192.0.2.2",
        &commands,
        &[],
        &[],
    )
    .await
}

#[tokio::test]
async fn description_advertises_only_the_unified_service_paths() {
    let response = request("GET", "/dlna/description.xml?probe=1").await;
    assert_eq!(response.status, 200);
    for service in ["av-transport", "rendering-control"] {
        for (tag, endpoint) in [
            ("controlURL", "control"),
            ("eventSubURL", "event"),
            ("SCPDURL", "scpd.xml"),
        ] {
            assert!(
                response
                    .body
                    .contains(&format!("<{tag}>/dlna/{service}/{endpoint}</{tag}>"))
            );
        }
        assert_eq!(
            request("GET", &format!("/dlna/{service}/scpd.xml"))
                .await
                .status,
            200
        );
        let subscribe = request("SUBSCRIBE", &format!("/dlna/{service}/event")).await;
        assert_eq!(subscribe.status, 200);
        assert!(subscribe.headers.iter().any(|(key, _)| key == "SID"));
        assert_eq!(
            request("UNSUBSCRIBE", &format!("/dlna/{service}/event"))
                .await
                .status,
            200
        );
    }
    assert!(!response.body.contains("/AVTransport/"));
    assert!(!response.body.contains("/RenderingControl/"));
}

#[tokio::test]
async fn rejects_old_unknown_and_wrong_method_routes() {
    for (method, path) in [
        ("GET", "/description.xml"),
        ("GET", "/AVTransport/scpd.xml"),
        ("POST", "/AVTransport/control"),
        ("SUBSCRIBE", "/RenderingControl/event"),
        ("POST", "/RenderingControl/control"),
        ("GET", "/dlna/unknown/description.xml"),
        ("GET", "/dlna/av-transport/unknown/scpd.xml"),
        ("POST", "/dlna/av-transport/unknown/control"),
        ("POST", "/dlna/av-transport/event"),
        ("GET", "/dlna/av-transport/control"),
        ("POST", "/dlna/description.xml"),
        ("POST", "/dlna/av-transport/scpd.xml"),
        ("SUBSCRIBE", "/dlna/description.xml"),
        ("SUBSCRIBE", "/dlna/rendering-control/control"),
        ("UNSUBSCRIBE", "/dlna/nope"),
    ] {
        assert!(
            !super::super::is_receiver_path(method, path),
            "{method} {path}"
        );
        assert_eq!(request(method, path).await.status, 404, "{method} {path}");
    }
    for path in [
        "/dlna/description.xml",
        "/dlna/av-transport/scpd.xml",
        "/dlna/rendering-control/scpd.xml",
    ] {
        assert_eq!(request("HEAD", path).await.status, 200);
    }
}

#[tokio::test]
async fn rendering_control_does_not_enter_the_transport_handler() {
    let state = Arc::new(RwLock::new(DlnaRendererState::default()));
    let (commands, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let headers = HashMap::from([(
        "soapaction".to_string(),
        "\"urn:schemas-upnp-org:service:RenderingControl:1#GetVolume\"".to_string(),
    )]);
    let response = route(
        &state,
        "POST",
        "/dlna/rendering-control/control",
        &headers,
        "",
        "test-device",
        "Test Renderer",
        "192.0.2.1",
        "192.0.2.2",
        &commands,
        &[],
        &[],
    )
    .await;
    assert_eq!(response.status, 200);
    assert!(response.body.contains("<CurrentVolume>100</CurrentVolume>"));
    assert!(
        response
            .body
            .contains("urn:schemas-upnp-org:service:RenderingControl:1")
    );
    assert!(
        !response
            .body
            .contains("urn:schemas-upnp-org:service:AVTransport:1")
    );
    assert!(receiver.try_recv().is_err());
    assert_eq!(
        request("POST", "/dlna/av-transport/control").await.status,
        500
    );
}
