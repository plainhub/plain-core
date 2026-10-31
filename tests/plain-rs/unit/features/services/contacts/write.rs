use super::*;

fn contact(source: &str, extra: Value) -> Value {
    let mut input = json!({
        "prefix": "",
        "firstName": "",
        "middleName": "",
        "lastName": "",
        "suffix": "",
        "nickname": "",
        "phoneNumbers": [],
        "emails": [],
        "addresses": [],
        "events": [],
        "source": source,
        "starred": false,
        "notes": "",
        "groupIds": [],
        "websites": [],
        "ims": [],
    });
    let (Value::Object(base), Value::Object(extra)) = (&mut input, extra) else {
        unreachable!()
    };
    base.extend(extra);
    input
}

#[test]
fn contact_writes_require_the_write_contacts_api_permission() {
    assert!(!permission_allowed(&[]));
    assert!(!permission_allowed(&["READ_CONTACTS".into()]));
    assert!(permission_allowed(&["WRITE_CONTACTS".into()]));
}

#[test]
fn create_requires_a_resolvable_account_source() {
    assert!(validate_contact_input(&contact("", json!({"firstName": "Ada"})), true).is_err());
    assert!(validate_contact_input(&contact("SIM", json!({"firstName": "Ada"})), true).is_ok());
    // Updating an existing contact keeps whatever account it already lives in.
    assert!(validate_contact_input(&contact("", json!({"firstName": "Ada"})), false).is_ok());
}

#[test]
fn a_contact_needs_a_name_a_phone_number_or_an_email() {
    assert!(validate_contact_input(&contact("SIM", json!({})), true).is_err());
    assert!(
        validate_contact_input(
            &contact(
                "SIM",
                json!({"phoneNumbers": [{"value": "", "type": "MOBILE", "label": ""}]})
            ),
            true
        )
        .is_err(),
        "the desktop edit form submits blank phone rows while it is being filled in"
    );
    assert!(
        validate_contact_input(
            &contact(
                "SIM",
                json!({"phoneNumbers": [{"value": "555", "type": "MOBILE", "label": ""}]})
            ),
            true
        )
        .is_ok()
    );
    assert!(
        validate_contact_input(
            &contact(
                "SIM",
                json!({"emails": [{"value": "a@b.c", "type": "HOME", "label": ""}]})
            ),
            true
        )
        .is_ok()
    );
    assert!(
        validate_contact_input(
            &contact("SIM", json!({"organization": {"company": "Bell Labs"}})),
            true
        )
        .is_ok()
    );
}

#[test]
fn oversized_and_malformed_contact_input_is_rejected() {
    let long = "x".repeat(MAX_NAME_LEN + 1);
    assert!(validate_contact_input(&contact("SIM", json!({"firstName": long})), true).is_err());
    assert!(
        validate_contact_input(
            &contact("SIM", json!({"emails": [{"value": "a@b.c", "type": 7}]})),
            true
        )
        .is_ok(),
        "the platform enum mapping stays in the host, so unknown types are not rejected here"
    );
    assert!(validate_contact_input(&contact("SIM", json!({"phoneNumbers": "555"})), true).is_err());
    assert!(
        validate_contact_input(
            &contact("SIM", json!({"phoneNumbers": [{"value": 7}]})),
            true
        )
        .is_err()
    );
    let many: Vec<Value> = (0..=MAX_ENTRIES)
        .map(|_| json!({"value": "a", "type": "HOME", "label": ""}))
        .collect();
    assert!(validate_contact_input(&contact("SIM", json!({"addresses": many})), true).is_err());
}

#[tokio::test]
async fn creation_resolves_the_account_before_any_provider_write() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    prefs
        .set("api_permissions", json!(["WRITE_CONTACTS"]))
        .unwrap();
    let host = std::sync::Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = calls.clone();
    let responder = host.clone();
    let provider = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let method = request["method"].as_str().unwrap().to_owned();
            if method == "systemCapabilities" {
                responder
                    .reply(
                        generation,
                        json!({"id":request["id"],"result":["CONTACTS"]}),
                    )
                    .unwrap();
                continue;
            }
            seen.lock().unwrap().push(method.clone());
            let value = match method.as_str() {
                "systemContactSources" => {
                    json!([{"name":"local","type":"first"},{"name":"local","type":"second"}])
                }
                "systemCreateContact" => {
                    assert_eq!(request["params"]["accountType"], "first");
                    json!("new-contact")
                }
                _ => panic!("unexpected operation {method}"),
            };
            responder
                .reply(generation, json!({"id":request["id"],"result":value}))
                .unwrap();
        }
    });
    for (source, success) in [("local", true), ("unknown", false)] {
        let result = execute(
            &prefs,
            &host,
            Request {
                action: Action::CreateContact,
                id: None,
                input: Some(contact(source, json!({"firstName":"Sample"}))),
                name: None,
                account_name: None,
                account_type: None,
            },
        )
        .await;
        assert_eq!(result.is_ok(), success);
        if success {
            assert_eq!(result.unwrap()["id"], "new-contact");
        } else {
            assert_eq!(result.unwrap_err().to_string(), "Contact source not found");
        }
    }
    assert_eq!(
        *calls.lock().unwrap(),
        [
            "systemContactSources",
            "systemCreateContact",
            "systemContactSources"
        ]
    );
    provider.abort();
    let _ = provider.await;
}
