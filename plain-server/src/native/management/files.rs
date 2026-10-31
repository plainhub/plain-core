use super::NativeManagement;

impl NativeManagement {
    pub fn file_paths(&self, paths: Vec<String>) -> Result<Vec<Option<String>>, String> {
        self.state.native_client_id()?;
        Ok(plain_rs::services::files::references::resolve_many(
            paths,
            |path| {
                plain_rs::services::files::references::resolve(
                    self.state.db.clone(),
                    self.state.directory.clone(),
                    path,
                )
                .ok()
            },
        ))
    }

    pub async fn import_note_image(
        &self,
        source: String,
        file_name: String,
    ) -> Result<String, String> {
        self.state.native_client_id()?;
        plain_rs::services::files::note_images::import(&self.state, &source, &file_name).await
    }

    pub async fn import_pomodoro_sound(&self, source: String) -> Result<String, String> {
        self.state.native_client_id()?;
        plain_rs::services::pomodoro_sound::import(&self.state, &source).await
    }
}
