use super::*;

fn data(id: &str, kind: &str, values: [&str; 6]) -> Value {
    json!({"rawId":id,"mimeType":format!("vnd.android.cursor.item/{kind}"),
        "data1":values[0],"data2":values[1],"data3":values[2],"data4":values[3],"data5":values[4],"data6":values[5]})
}

#[test]
fn selected_contacts_own_only_their_details_in_provider_order() {
    let page = json!({"rows":[{"id":"b"},{"id":"a"},{"id":"b"}],"groups":[],"details":[
        data("not-selected","note",["private","","","","",""]),
        data("a","note",["first","","","","",""]),
        data("a","note",["second","","","","",""]),
        data("b","nickname",["","","","","",""]),
        data("b","nickname",["second","","","","",""]),
        data("b","organization",["Company","ignored","","Title","",""]),
        data("b","organization",["Ignored","","","Ignored","",""]),
        data("a","im",["name","wrong-type","wrong-label","wrong-extra","6","custom"]),
        data("a","phone_v2",["number","99","custom","normalized","",""])
    ]});
    let rows = contacts(page).unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["b", "a", "b"]
    );
    assert_eq!(rows[0]["notes"], "");
    assert_eq!(rows[1]["notes"], "first");
    assert_eq!(rows[0]["nickname"], "");
    assert_eq!(
        rows[0]["organization"],
        json!({"company":"Company","title":"Title"})
    );
    assert_eq!(
        rows[1]["ims"],
        json!([{"value":"name","type":6,"label":"custom"}])
    );
    assert_eq!(
        rows[1]["phoneNumbers"],
        json!([{"value":"number","type":99,"label":"custom","normalizedNumber":"normalized"}])
    );
}

#[test]
fn group_visibility_and_duplicate_system_titles_match_provider_behavior() {
    let rows = groups(vec![
        json!({"id":"1","name":"Friends"}),
        json!({"id":"2","name":"Friends","systemId":"system"}),
        json!({"id":"3","name":"Friends","systemId":""}),
        json!({"id":"4","name":"Hidden","autoAdd":true}),
        json!({"id":"5","name":"Favorites","favorites":true}),
        json!({"id":"6","name":"Other","systemId":"system"}),
    ]);
    assert_eq!(
        rows.iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["1", "3", "6"]
    );
    let page = contacts(json!({"rows":[{"id":"a"}],"groups":rows,"details":[
        data("a","group_membership",["003","","","","",""]),
        data("a","group_membership",["missing","","","","",""])
    ]}))
    .unwrap();
    assert_eq!(page[0]["groups"][0]["id"], "3");
    assert_eq!(page[0]["groups"].as_array().unwrap().len(), 1);
}

#[test]
fn sources_deduplicate_name_and_type_without_losing_first_order() {
    assert_eq!(
        sources(vec![
            json!({"name":"local","type":"a"}),
            json!({"name":"local","type":"a"}),
            json!({"name":"local","type":"b"})
        ]),
        vec![
            json!({"name":"local","type":"a"}),
            json!({"name":"local","type":"b"})
        ]
    );
}

#[test]
fn malformed_contact_pages_fail_instead_of_appearing_empty() {
    assert!(contacts(json!([])).is_err());
    assert!(contacts(json!({"rows":["invalid"],"groups":[],"details":[]})).is_err());
    assert!(contacts(json!({"rows":[],"groups":[],"details":[{"rawId":"a"}]})).is_err());
    assert!(
        contacts(json!({"rows":[],"groups":[],"details":[]}))
            .unwrap()
            .is_empty()
    );
}
