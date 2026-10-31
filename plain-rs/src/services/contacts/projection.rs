use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

#[derive(Deserialize)]
struct Page {
    rows: Vec<Value>,
    details: Vec<Data>,
    groups: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    raw_id: String,
    mime_type: String,
    data1: String,
    data2: String,
    data3: String,
    data4: String,
    data5: String,
    data6: String,
}

pub fn groups(rows: Vec<Value>) -> Vec<Value> {
    let mut names = HashSet::new();
    rows.into_iter()
        .filter(|row| {
            if row["autoAdd"].as_bool().unwrap_or(false)
                || row["favorites"].as_bool().unwrap_or(false)
            {
                return false;
            }
            let name = row["name"].as_str().unwrap_or_default().to_owned();
            if names.contains(&name) && row["systemId"].as_str().is_some_and(|id| !id.is_empty()) {
                return false;
            }
            names.insert(name);
            true
        })
        .collect()
}

pub fn sources(rows: Vec<Value>) -> Vec<Value> {
    let mut seen = HashSet::new();
    rows.into_iter()
        .filter(|row| {
            seen.insert((
                row["name"].as_str().unwrap_or_default().to_owned(),
                row["type"].as_str().unwrap_or_default().to_owned(),
            ))
        })
        .collect()
}

pub fn contacts(value: Value) -> anyhow::Result<Vec<Value>> {
    let page: Page = serde_json::from_value(value)?;
    let groups: HashMap<String, Value> = groups(page.groups)
        .into_iter()
        .map(|group| (group["id"].as_str().unwrap_or_default().to_owned(), group))
        .collect();
    let mut details: HashMap<String, Vec<Data>> = HashMap::new();
    for data in page.details {
        details.entry(data.raw_id.clone()).or_default().push(data);
    }
    page.rows.into_iter().map(|mut row| {
        anyhow::ensure!(row.is_object(), "invalid contact row");
        for key in ["phoneNumbers", "emails", "addresses", "events", "websites", "ims", "groups"] {
            row[key] = json!([]);
        }
        row["nickname"] = json!("");
        row["notes"] = json!("");
        row["organization"] = json!({"company":"","title":""});
        let mut first = HashSet::new();
        if let Some(data) = details.get(row["id"].as_str().unwrap_or_default()) {
            for data in data {
                let kind = data.mime_type.strip_prefix("vnd.android.cursor.item/").unwrap_or(&data.mime_type);
                match kind {
                    "nickname" | "note" => {
                        let field = if kind == "note" { "notes" } else { "nickname" };
                        if first.insert(field) { row[field] = json!(data.data1); }
                    }
                    "organization" => {
                        if first.insert("organization") { row["organization"] = json!({"company":data.data1,"title":data.data4}); }
                    }
                    "group_membership" => {
                        let id = data.data1.parse::<i64>().unwrap_or_default().to_string();
                        if let Some(group) = groups.get(&id) { row["groups"].as_array_mut().unwrap().push(group.clone()); }
                    }
                    _ => {
                        let field = match kind {
                            "phone_v2" => "phoneNumbers", "email_v2" => "emails",
                            "postal-address_v2" => "addresses", "contact_event" => "events",
                            "website" => "websites", "im" => "ims", _ => continue,
                        };
                        let mut detail = json!({"value":data.data1,"type":data.data2.parse::<i64>().unwrap_or_default(),"label":data.data3});
                        if field == "phoneNumbers" { detail["normalizedNumber"] = json!(data.data4); }
                        if field == "ims" { detail["type"] = json!(data.data5.parse::<i64>().unwrap_or_default()); detail["label"] = json!(data.data6); }
                        row[field].as_array_mut().unwrap().push(detail);
                    }
                }
            }
        }
        Ok(row)
    }).collect()
}

#[cfg(test)]
#[path = "../../../tests/unit/services/contacts/projection.rs"]
mod tests;
