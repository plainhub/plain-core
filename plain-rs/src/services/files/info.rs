use plain_rs::enums::DataType;
use plain_rs::host::bridge::Host;
use serde_json::{Value, json};
use std::{
    io::{BufReader, Read, Seek, SeekFrom},
    path::PathBuf,
};

pub struct Info {
    host: std::sync::Arc<Host>,
    sdk: bool,
    cache: PathBuf,
    resources: std::sync::Arc<plain_rs::host::resources::NativeResources>,
}
impl Info {
    pub fn new(
        host: std::sync::Arc<Host>,
        resources: std::sync::Arc<plain_rs::host::resources::NativeResources>,
        cache: PathBuf,
    ) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            host,
            resources,
            cache,
            sdk: cfg!(any(target_os = "android", target_os = "ios")),
        })
    }
    #[cfg(any(test, feature = "test-seams"))]
    pub fn sdk(host: std::sync::Arc<Host>) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            resources: std::sync::Arc::new(plain_rs::host::resources::NativeResources::new(
                host.clone(),
            )),
            host,
            cache: std::env::temp_dir().join("plain-test-cache"),
            sdk: true,
        })
    }
    pub async fn read(&self, path: &str, kind: DataType) -> Result<Value, String> {
        if kind == DataType::Video {
            let sample = plain_rs::services::files::input::read_source(
                &self.resources,
                &self.cache,
                path,
                |reader| {
                    plain_rs::media::video_info::read(reader)
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(|error| error.to_string())
                },
            )
            .await?;
            return Ok(match sample {
                Some(mut facts) => {
                    normalize_video(&mut facts);
                    facts
                }
                None => Value::Null,
            });
        }
        if kind == DataType::Audio {
            let sample = plain_rs::services::files::input::read_source(
                &self.resources,
                &self.cache,
                path,
                |reader| {
                    let duration = plain_rs::media::probe::probe_duration_reader(reader);
                    let location = plain_rs::media::isobmff::location_reader(reader);
                    Ok(duration
                        .map(|duration| json!({"durationMs":duration,"rawLocation":location})))
                },
            )
            .await;
            match sample {
                Ok(Some(facts)) => return Ok(facts),
                Ok(None) => return Ok(Value::Null),
                Err(error) => return Err(error),
            }
        }
        if kind != DataType::Image {
            return Err("Unsupported media info kind".into());
        }
        let mut facts = plain_rs::services::files::input::read_source(
            &self.resources,
            &self.cache,
            path,
            |reader| image_reader(reader),
        )
        .await?;
        if self.sdk
            && (facts.is_null()
                || facts["width"].as_i64().unwrap_or(0) <= 0
                || facts["height"].as_i64().unwrap_or(0) <= 0)
        {
            let fallback = self
                .host
                .call(
                    "imageDecodeBounds",
                    json!({"path":plain_rs::utils::file_uri::local_path(path)?}),
                )
                .await?;
            apply_image_bounds(&mut facts, fallback)?;
        }
        Ok(facts)
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageBounds {
    width: i32,
    height: i32,
}

fn apply_image_bounds(facts: &mut Value, sample: Value) -> Result<(), String> {
    let bounds = if sample.is_null() {
        None
    } else {
        if !sample.is_object() {
            return Err("Invalid image decode bounds".into());
        }
        let bounds: ImageBounds = serde_json::from_value(sample)
            .map_err(|error| format!("Invalid image decode bounds: {error}"))?;
        if bounds.width < 0 || bounds.height < 0 {
            return Err("Invalid image decode bounds".into());
        }
        Some(bounds)
    };
    if facts.is_null() {
        *facts = json!({"width":0,"height":0});
    }
    if let Some(bounds) = bounds.filter(|bounds| bounds.width > 0 && bounds.height > 0) {
        let swapped = matches!(facts["rotation"].as_i64(), Some(90 | 270));
        facts["width"] = json!(if swapped { bounds.height } else { bounds.width });
        facts["height"] = json!(if swapped { bounds.width } else { bounds.height });
    }
    Ok(())
}

fn image_reader(mut reader: impl Read + Seek) -> Result<Value, String> {
    let bytes = image_prefix(&mut reader).map_err(|error| error.to_string())?;
    let exif = match plain_rs::utils::image_exif::read(&mut reader) {
        Ok(exif) => exif,
        Err(error) => {
            log::debug!("Image EXIF read failed: {error}");
            None
        }
    };
    let mut facts = image_facts_with_exif(&bytes, exif.as_deref());
    if facts["width"].as_i64().unwrap_or(0) <= 0 || facts["height"].as_i64().unwrap_or(0) <= 0 {
        reader
            .seek(SeekFrom::Start(0))
            .map_err(|error| error.to_string())?;
        if let Some((width, height)) = image::ImageReader::new(BufReader::new(reader))
            .with_guessed_format()
            .ok()
            .and_then(|reader| reader.into_dimensions().ok())
        {
            let swapped = matches!(facts["rotation"].as_i64(), Some(90 | 270));
            facts["width"] = json!(if swapped { height } else { width });
            facts["height"] = json!(if swapped { width } else { height });
        }
    }
    Ok(facts)
}
fn image_facts_with_exif(bytes: &[u8], exif: Option<&[u8]>) -> Value {
    let metadata_bytes = exif.unwrap_or(bytes);
    let metadata = plain_rs::utils::image_metadata::read(metadata_bytes);
    let date = metadata.taken_at();
    let mut facts = serde_json::to_value(metadata).unwrap_or_else(|_| json!({}));
    let (mut width, mut height) = plain_rs::utils::image_dimensions::dimensions(bytes)
        .or_else(|| exif.and_then(plain_rs::utils::image_dimensions::dimensions))
        .unwrap_or((0, 0));
    if matches!(facts["rotation"].as_i64(), Some(90 | 270)) {
        std::mem::swap(&mut width, &mut height);
    }
    facts["width"] = json!(width);
    facts["height"] = json!(height);
    facts["takenAt"] = json!(date);
    facts["location"] = plain_rs::utils::image_metadata::gps(metadata_bytes)
        .map(|(latitude, longitude)| json!({"latitude":latitude,"longitude":longitude}))
        .unwrap_or(Value::Null);
    facts
}
fn image_prefix(reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(128 * 1024).read_to_end(&mut bytes)?;
    Ok(bytes)
}
fn normalize_video(facts: &mut Value) {
    let integer = |key: &str| facts[key].as_i64().unwrap_or(0);
    let width = i32::try_from(integer("width")).unwrap_or(0).max(0);
    let height = i32::try_from(integer("height")).unwrap_or(0).max(0);
    let rotation = facts["rotation"]
        .as_i64()
        .unwrap_or_default()
        .rem_euclid(360);
    let date = facts["rawTakenAt"]
        .as_str()
        .or_else(|| facts["takenAt"].as_str())
        .and_then(taken_at);
    let frame_rate = facts["frameRate"]
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(0.0);
    let duration = integer("durationMs").max(0);
    let bitrate = integer("bitrate").max(0);
    facts["width"] = json!(width);
    facts["height"] = json!(height);
    facts["rotation"] = json!(rotation);
    let swapped = matches!(rotation, 90 | 270);
    facts["displayWidth"] = json!(if swapped { height } else { width });
    facts["displayHeight"] = json!(if swapped { width } else { height });
    facts["takenAt"] = json!(date);
    facts["frameRate"] = json!(frame_rate);
    facts["durationMs"] = json!(duration);
    facts["bitrate"] = json!(bitrate);
}
fn taken_at(raw: &str) -> Option<String> {
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .or_else(|| {
            ["%Y%m%dT%H%M%S%.f%#z", "%Y%m%dT%H%M%S%#z"]
                .into_iter()
                .find_map(|format| chrono::DateTime::parse_from_str(raw, format).ok())
        })
        .map(|date| {
            date.with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        })
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/info.rs"]
mod tests;
