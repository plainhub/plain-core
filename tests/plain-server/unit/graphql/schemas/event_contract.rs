use super::*;

#[test]
fn events_keep_their_own_named_type_and_closed_status_values() {
    let sdl = sdl();
    assert!(sdl.contains("events(query: String!, offset: Int!, limit: Int!): [Event!]!"));
    assert!(sdl.contains("type Event {"));
    let object = sdl.split("type Event {").nth(1).unwrap().split('}').next().unwrap();
    for field in ["id: ID!", "type: EventType!", "message: String!", "clientId: String", "createdAt: Instant!"] { assert!(object.contains(field), "{field}"); }
    let values: Vec<_> = sdl.split("enum EventType {").nth(1).unwrap().split('}').next().unwrap().split_whitespace().collect();
    assert_eq!(values, ["LOGIN", "LOGIN_FAILED", "LOGOUT", "REVOKE", "SET_HOSTNAME", "UPDATE_DEVICE_NAME", "MOUNT", "MOUNT_FAILED", "UNMOUNT", "FORMAT_DISK", "FORMAT_DISK_FAILED"]);
    assert!(sdl.contains("enum ContactEventType {"));
    for name in ["ContactEvent", "ContactEventInput"] {
        let object = sdl.split(&format!("{name} {{")).nth(1).unwrap().split('}').next().unwrap();
        assert!(object.contains("type: ContactEventType!"));
    }
    for name in ["ContactPhoneType", "ContactEmailType", "ContactPostalType", "ContactWebsiteType", "ContactImProtocol"] {
        assert!(sdl.contains(&format!("enum {name} {{")), "{name}");
    }
    for (object, field, kind) in [
        ("ContactPhoneNumber", "type", "ContactPhoneType"), ("ContactPhoneInput", "type", "ContactPhoneType"),
        ("ContactEmail", "type", "ContactEmailType"), ("ContactEmailInput", "type", "ContactEmailType"),
        ("ContactAddress", "type", "ContactPostalType"), ("ContactAddressInput", "type", "ContactPostalType"),
        ("ContactWebsite", "type", "ContactWebsiteType"), ("ContactWebsiteInput", "type", "ContactWebsiteType"),
        ("ContactIm", "protocol", "ContactImProtocol"), ("ContactImInput", "protocol", "ContactImProtocol"),
    ] {
        let body = sdl.split(&format!("{object} {{")).nth(1).unwrap().split('}').next().unwrap();
        assert!(body.contains(&format!("{field}: {kind}!")), "{object}.{field}");
    }
    assert!(sdl.contains("type ContactOrganization {"));
    assert!(sdl.contains("input ContactOrganizationInput {"));
    assert!(!sdl.contains("auditEvents("));
    assert!(!sdl.contains("type AuditEvent {"));
}
