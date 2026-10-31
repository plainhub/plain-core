use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub path: String,
    pub content_type: String,
    pub name: String,
    pub file_name: String,
    pub has_data: bool,
    pub text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub path: String,
    pub content_type: String,
    pub name: String,
}

pub struct Message {
    pub body: String,
    pub attachments: Vec<Attachment>,
}

pub fn message(parts: &[Part]) -> Message {
    let mut texts = Vec::new();
    let mut attachments = Vec::new();
    for part in parts {
        let mime = part.content_type.to_ascii_lowercase();
        if mime == "text/plain" {
            if !part.text.is_empty() {
                texts.push(part.text.as_str());
            }
        } else if mime.starts_with("image/")
            || mime.starts_with("video/")
            || mime.starts_with("audio/")
            || part.has_data
        {
            attachments.push(Attachment {
                path: part.path.clone(),
                content_type: part.content_type.clone(),
                name: if part.name.is_empty() {
                    &part.file_name
                } else {
                    &part.name
                }
                .clone(),
            });
        }
    }
    let body = texts.join("\n").trim().to_owned();
    let body = if body.is_empty() && !attachments.is_empty() {
        "[MMS]".to_owned()
    } else {
        body
    };
    Message { body, attachments }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    date_millis: i64,
    value: String,
    parts: Option<Vec<Part>>,
}

pub fn snippet(candidates: &[Snippet]) -> Option<String> {
    candidates
        .iter()
        .reduce(|latest, candidate| {
            if candidate.date_millis > latest.date_millis {
                candidate
            } else {
                latest
            }
        })
        .map(|candidate| {
            candidate
                .parts
                .as_deref()
                .map(message)
                .map(|message| message.body)
                .unwrap_or_else(|| candidate.value.clone())
        })
}

#[cfg(test)]
#[path = "../../../tests/unit/services/sms/parts.rs"]
mod tests;
