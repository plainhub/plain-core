use super::*;

#[test]
fn events_keep_their_own_named_type_and_closed_status_values() {
    let sdl = sdl();
    assert!(sdl.contains("events(query: String!, offset: Int!, limit: Int!): [Event!]!"));
    assert!(sdl.contains("type Event {"));
    let object = sdl
        .split("type Event {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    for field in [
        "id: ID!",
        "type: EventType!",
        "data: EventData!",
        "clientId: ID",
        "createdAt: Instant!",
    ] {
        assert!(object.lines().any(|line| line.trim() == field), "{field}");
    }
    let values: Vec<_> = sdl
        .split("enum EventType {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap()
        .split_whitespace()
        .collect();
    assert_eq!(
        values,
        [
            "LOGIN",
            "LOGIN_FAILED",
            "LOGOUT",
            "REVOKE_SESSION",
            "UPDATE_DEVICE_NAME",
            "MOUNT",
            "MOUNT_FAILED",
            "UNMOUNT",
            "FORMAT_DISK",
            "FORMAT_DISK_FAILED",
        ]
    );
    assert!(sdl.contains("enum ContactEventType {"));
    for name in ["ContactEvent", "ContactEventInput"] {
        let object = sdl
            .split(&format!("{name} {{"))
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert!(object.contains("type: ContactEventType!"));
    }
    for name in [
        "ContactPhoneType",
        "ContactEmailType",
        "ContactPostalType",
        "ContactWebsiteType",
        "ContactImProtocol",
    ] {
        assert!(sdl.contains(&format!("enum {name} {{")), "{name}");
    }
    for (object, field, kind) in [
        ("ContactPhoneNumber", "type", "ContactPhoneType"),
        ("ContactPhoneInput", "type", "ContactPhoneType"),
        ("ContactEmail", "type", "ContactEmailType"),
        ("ContactEmailInput", "type", "ContactEmailType"),
        ("ContactAddress", "type", "ContactPostalType"),
        ("ContactAddressInput", "type", "ContactPostalType"),
        ("ContactWebsite", "type", "ContactWebsiteType"),
        ("ContactWebsiteInput", "type", "ContactWebsiteType"),
        ("ContactIm", "protocol", "ContactImProtocol"),
        ("ContactImInput", "protocol", "ContactImProtocol"),
    ] {
        let body = sdl
            .split(&format!("{object} {{"))
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert!(
            body.contains(&format!("{field}: {kind}!")),
            "{object}.{field}"
        );
    }
    assert!(sdl.contains("type ContactOrganization {"));
    assert!(sdl.contains("input ContactOrganizationInput {"));
    assert!(!sdl.contains("auditEvents("));
    assert!(!sdl.contains("type AuditEvent {"));
}

#[test]
fn event_storage_rejects_unknown_and_retired_event_names() {
    use plain_rs::features::audit_events::EventType;
    for name in ["set_hostname", "revoke", "UNKNOWN"] {
        assert!(
            plain_libs::serde_json::from_value::<EventType>(plain_libs::serde_json::json!(name))
                .is_err()
        );
    }
    assert_eq!(
        plain_libs::serde_json::from_value::<EventType>(plain_libs::serde_json::json!(
            "UPDATE_DEVICE_NAME"
        ))
        .unwrap(),
        EventType::UpdateDeviceName
    );
}

#[test]
fn event_payloads_keep_named_union_members_enums_and_nullable_actor_fields() {
    let sdl = sdl();
    assert!(sdl.contains("union EventData = LoginEventData | LoginFailedEventData | SessionEventData | DeviceNameEventData | VolumeEventData | VolumeFailedEventData | DiskFormatEventData | DiskFormatFailedEventData"));
    for (name, fields) in [
        (
            "Event",
            vec![
                "clientName: String",
                "clientIp: String",
                "osName: String",
                "osVersion: String",
                "appName: String",
                "appVersion: String",
            ],
        ),
        ("LoginEventData", vec!["sessionType: SessionType!"]),
        (
            "EventSession",
            vec![
                "type: SessionType!",
                "clientId: String!",
                "clientName: String!",
            ],
        ),
        ("LoginFailedEventData", vec!["reason: AuthErrorCode!"]),
        (
            "MountFailure",
            vec!["reason: MountErrorCode!", "detail: String"],
        ),
        ("EventDisk", vec!["sizeBytes: Long"]),
        ("VolumeFailedEventData", vec!["failure: MountFailure!"]),
        ("DiskFormatFailedEventData", vec!["failure: DiskFormatFailure!"]),
    ] {
        let body = sdl
            .split(&format!("type {name} {{"))
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        for field in fields {
            assert!(
                body.lines().any(|line| line.trim() == field),
                "{name}.{field}"
            );
        }
    }
    let event = sdl
        .split("type Event {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert!(!event.contains("message:"));
    let reasons: Vec<_> = sdl
        .split("enum AuthErrorCode {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap()
        .split_whitespace()
        .collect();
    for reason in [
            "INVALID_LOGIN_REQUEST",
            "INVALID_PASSWORD",
            "PEER_IDENTITY_CHANGED",
            "REMOTE_ACCESS_DISABLED",
            "TIMEOUT",
            "TOO_MANY_LOGIN_ATTEMPTS",
            "PASSWORD_SETUP_REQUIRED",
            "INTERNAL_ERROR",
            "REJECTED",
            "CANCELED",
    ] {
        assert!(reasons.contains(&reason), "missing authentication error {reason}");
    }
}

#[tokio::test]
async fn all_public_enum_values_use_uppercase_snake_case_without_a_global_error_enum() {
    use crate::graphql::schemas::public::{PublicSchema, Query, Mutation};
    let schema = PublicSchema::build(Query::default(), Mutation::default(), crate::graphql::subscriptions::Subscription::default()).finish();
    let response = schema.execute("{ __schema { types { kind name enumValues { name } } } }").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let types = data["__schema"]["types"].as_array().unwrap();
    for (name, expected) in [
        ("AuthErrorCode", "CANCELED INTERNAL_ERROR INVALID_LOGIN_REQUEST INVALID_PASSWORD PASSWORD_SETUP_REQUIRED PEER_IDENTITY_CHANGED REJECTED REMOTE_ACCESS_DISABLED TOO_MANY_LOGIN_ATTEMPTS TIMEOUT"),
        ("PairingErrorCode", "TIMEOUT REJECTED PASSWORD_REQUIRED INVALID_PASSWORD PEER_IDENTITY_CHANGED TOO_MANY_LOGIN_ATTEMPTS INTERNAL_ERROR"),
    ] {
        let entry = types.iter().find(|entry| entry["name"] == name).unwrap();
        let actual = entry["enumValues"].as_array().unwrap().iter().map(|value| value["name"].as_str().unwrap()).collect::<std::collections::BTreeSet<_>>();
        assert_eq!(actual, expected.split_whitespace().collect::<std::collections::BTreeSet<_>>(), "{name}");
    }
    for entry in types {
        if entry["kind"] != "ENUM" { continue; }
        assert_ne!(entry["name"], "ErrorCode");
        for value in entry["enumValues"].as_array().unwrap() {
            let name = value["name"].as_str().unwrap();
            assert!(name.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'), "{}: {name}", entry["name"]);
        }
    }
}
