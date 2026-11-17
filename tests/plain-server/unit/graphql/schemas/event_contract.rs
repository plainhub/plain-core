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
            "FORMAT_DISK_FAILED"
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
        ("LoginFailedEventData", vec!["reason: ErrorCode!"]),
        (
            "EventFailure",
            vec!["reason: ErrorCode!", "detail: String"],
        ),
        ("EventDisk", vec!["sizeBytes: Long"]),
        ("VolumeFailedEventData", vec!["failure: EventFailure!"]),
        ("DiskFormatFailedEventData", vec!["failure: EventFailure!"]),
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
        .split("enum ErrorCode {")
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
            "LOGIN_EXPIRED",
            "TOO_MANY_LOGIN_ATTEMPTS",
            "PASSWORD_SETUP_REQUIRED",
            "INTERNAL_ERROR",
            "REJECTED",
            "CANCELED",
            "MOUNT_FAILED",
            "FORMAT_DISK_FAILED"
    ] {
        assert!(reasons.contains(&reason), "missing shared error {reason}");
    }
}
