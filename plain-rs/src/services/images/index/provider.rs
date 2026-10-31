use plain_rs::error::DomainError;
use plain_rs::error::DomainResult;
use plain_rs::host::bridge::Host;
use plain_rs::media::images::indexing::Embedded;
use plain_rs::media::images::indexing::Image;
use plain_rs::media::images::indexing::Page;
use plain_rs::media::images::indexing::Provider;
use plain_rs::media::images::indexing::Snapshot;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::sync::Arc;
pub struct ImageProvider {
    catalog: Option<Arc<plain_rs::db::Db>>,
    resources: Arc<plain_rs::host::resources::NativeResources>,
    cache: std::path::PathBuf,
    host: Arc<Host>,
    runtime: tokio::runtime::Handle,
    ended: bool,
    encoder: Option<Arc<dyn plain_inference::embedding::ImageEncoder>>,
}
impl ImageProvider {
    pub fn new(
        host: Arc<Host>,
        resources: Arc<plain_rs::host::resources::NativeResources>,
        cache: std::path::PathBuf,
        encoder: Option<Arc<dyn plain_inference::embedding::ImageEncoder>>,
        db: Arc<plain_rs::db::Db>,
        system_catalog: bool,
    ) -> Self {
        Self {
            catalog: (!system_catalog).then_some(db),
            host,
            resources,
            cache,
            runtime: tokio::runtime::Handle::current(),
            ended: false,
            encoder,
        }
    }

    pub fn finish(&mut self) {
        if self.ended {
            return;
        }
        self.ended = true;
        if let Some(encoder) = &self.encoder {
            encoder.release();
        }
    }

    fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> DomainResult<T> {
        let value = self
            .runtime
            .block_on(self.host.call(method, params))
            .map_err(DomainError::Other)?;
        serde_json::from_value(value)
            .map_err(|e| DomainError::Other(format!("invalid image index host response: {e}")))
    }

    fn encode(
        &self,
        encoder: Arc<dyn plain_inference::embedding::ImageEncoder>,
        item: &Image,
    ) -> DomainResult<Option<Vec<f32>>> {
        self.encode_with(encoder, item, |source, output| {
            self.runtime
                .block_on(plain_rs::services::files::resources::read(
                    &self.host,
                    "decodePng",
                    &source.to_string_lossy(),
                    json!({"output": output}),
                ))
        })
    }

    fn encode_with(
        &self,
        encoder: Arc<dyn plain_inference::embedding::ImageEncoder>,
        item: &Image,
        decode: impl FnOnce(&std::path::Path, &std::path::Path) -> Result<Value, String>,
    ) -> DomainResult<Option<Vec<f32>>> {
        let mut staged = None;
        let resource = item.path.starts_with("ph://") || item.path.starts_with("content://");
        let vector = if resource {
            let source_encoder = encoder.clone();
            let cache = self.cache.clone();
            let (vector, temporary) = self
                .runtime
                .block_on(plain_rs::services::files::input::read_source(
                    &self.resources,
                    &self.cache,
                    &item.path,
                    move |reader| {
                        let vector = source_encoder.image_reader(reader)?;
                        let temporary = if vector.is_none() {
                            use std::io::{Seek, SeekFrom};
                            reader
                                .seek(SeekFrom::Start(0))
                                .map_err(|error| error.to_string())?;
                            std::fs::create_dir_all(&cache).map_err(|error| error.to_string())?;
                            let mut temporary = tempfile::NamedTempFile::new_in(cache)
                                .map_err(|error| error.to_string())?;
                            std::io::copy(reader, &mut temporary)
                                .map_err(|error| error.to_string())?;
                            Some(temporary)
                        } else {
                            None
                        };
                        Ok((vector, temporary))
                    },
                ))
                .map_err(DomainError::Other)?;
            staged = temporary;
            vector
        } else {
            encoder
                .image(std::path::Path::new(&item.path))
                .map_err(DomainError::Other)?
        };
        if vector.is_some() || self.catalog.is_some() {
            return Ok(vector);
        }
        std::fs::create_dir_all(&self.cache)
            .map_err(|error| DomainError::Other(error.to_string()))?;
        let output = tempfile::NamedTempFile::new_in(&self.cache)
            .map_err(|error| DomainError::Other(error.to_string()))?;
        let source = staged
            .as_ref()
            .map(|file| file.path())
            .unwrap_or_else(|| std::path::Path::new(&item.path));
        let decoded = decode(source, output.path()).map_err(DomainError::Other)?;
        if decoded != Value::Bool(true) {
            return Ok(None);
        }
        encoder.image(output.path()).map_err(DomainError::Other)
    }
}
impl Provider for ImageProvider {
    fn resolve(&mut self, revision: &str, ids: &[String]) -> DomainResult<Vec<Image>> {
        if let Some(db) = &self.catalog {
            return plain_rs::db::image_catalog::resolve(db, revision, ids);
        }
        self.call(
            "imageCatalogResolve",
            json!({"revision":revision,"ids":ids}),
        )
    }
    fn snapshot(&mut self) -> DomainResult<Snapshot> {
        if let Some(db) = &self.catalog {
            return plain_rs::db::image_catalog::snapshot(db);
        }
        self.call("imageCatalogSnapshot", json!({}))
    }
    fn page(&mut self, revision: &str, cursor: &str, limit: usize) -> DomainResult<Page> {
        if let Some(db) = &self.catalog {
            return plain_rs::db::image_catalog::page(db, revision, cursor, limit);
        }
        self.call(
            "imageCatalogPage",
            json!({"revision":revision,"cursor":cursor,"limit":limit}),
        )
    }
    fn embed(&mut self, revision: &str, items: &[Image]) -> DomainResult<Embedded> {
        self.verify(revision)?;
        let encoder = self
            .encoder
            .as_ref()
            .ok_or_else(|| DomainError::Other("Image encoder unavailable".into()))?;
        let mut embedded = Embedded {
            items: Vec::new(),
            skipped_ids: Vec::new(),
        };
        for item in items {
            let vector = self.encode(encoder.clone(), item)?;
            match vector {
                Some(vector) => {
                    embedded
                        .items
                        .push(plain_rs::media::images::embeddings::EmbeddingInput {
                            id: item.id.clone(),
                            path: item.path.clone(),
                            embedding_base64: plain_rs::utils::base64::base64_encode(
                                &vector
                                    .iter()
                                    .flat_map(|v| v.to_be_bytes())
                                    .collect::<Vec<_>>(),
                            ),
                        })
                }
                None => embedded.skipped_ids.push(item.id.clone()),
            }
        }
        self.verify(revision)?;
        Ok(embedded)
    }
    fn verify(&mut self, revision: &str) -> DomainResult<()> {
        if let Some(db) = &self.catalog {
            return plain_rs::db::image_catalog::verify(db, revision);
        }
        let verified: bool = self.call("imageCatalogVerify", json!({"revision":revision}))?;
        if !verified {
            return Err(DomainError::Other(
                "image catalog verification failed".into(),
            ));
        }
        Ok(())
    }
}
impl Drop for ImageProvider {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/services/images/index/provider.rs"]
mod tests;
