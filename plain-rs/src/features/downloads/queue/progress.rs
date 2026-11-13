use super::*;

impl Queue {
    fn update<T>(
        &self,
        id: &str,
        generation: &str,
        work: impl FnOnce(&mut Task) -> Result<T>,
    ) -> Result<T> {
        let mut state = self.state.lock().unwrap();
        let task = active(&mut state, id, generation)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Download attempt expired"))?;
        let result = work(task)?;
        self.notify();
        Ok(result)
    }
    pub fn commit<T>(
        &self,
        id: &str,
        generation: &str,
        work: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        self.update(id, generation, |_| work())
    }
    pub fn plan(
        &self,
        id: &str,
        generation: &str,
        total_files: i32,
        total_bytes: i64,
        title: &str,
    ) -> Result<()> {
        self.update(id, generation, |task| {
            ensure!(
                total_files >= 0 && total_bytes >= 0,
                "Invalid download totals"
            );
            task.run.download.title = title.into();
            task.run.download.total_files = total_files;
            task.run.download.total_bytes = total_bytes;
            Ok(())
        })
    }
    pub fn begin_file(&self, id: &str, generation: &str, name: &str, size: i64) -> Result<()> {
        self.update(id, generation, |task| {
            ensure!(size >= 0, "Invalid download size");
            task.current_size = size;
            task.current_bytes = 0;
            task.run.download.current_file = name.into();
            Ok(())
        })
    }
    pub fn progress(&self, id: &str, generation: &str, bytes: u64) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let task = active(&mut state, id, generation)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Download attempt expired"))?;
        let elapsed = task.last.elapsed();
        if advance(task, bytes, elapsed)? {
            self.notify();
        }
        Ok(())
    }
    pub fn finish_file(
        &self,
        id: &str,
        generation: &str,
        path: &str,
        work: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        self.update(id, generation, |task| {
            ensure!(
                task.current_bytes == task.current_size,
                "Incomplete download"
            );
            work()?;
            task.run.completed.insert(path.into(), task.current_size);
            task.run.download.done_files = i32::try_from(task.run.completed.len())?;
            task.current_bytes = 0;
            Ok(())
        })
    }
    pub fn fail_file(
        &self,
        id: &str,
        generation: &str,
        path: &str,
        error: plain_libs::anyhow::Error,
    ) -> Result<()> {
        self.update(id, generation, |task| {
            failure(task, path, error);
            Ok(())
        })
    }
    pub fn packing(&self, id: &str, generation: &str) -> Result<()> {
        self.update(id, generation, |task| {
            task.run.download.status = Status::Packing;
            Ok(())
        })
    }
    pub fn finish(&self, id: &str, generation: &str, result: Result<()>) -> Result<bool> {
        let mut state = self.state.lock().unwrap();
        let Some(task) = active(&mut state, id, generation) else {
            return Ok(false);
        };
        let error = result
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        if task.run.download.zip_name.is_some() && !error.is_empty() {
            task.run.completed.clear();
        }
        task.run.download.done_files = i32::try_from(task.run.completed.len())?;
        task.run.download.downloaded_bytes = task.run.completed.values().sum();
        task.current_bytes = 0;
        let download = &mut task.run.download;
        download.bytes_per_second = 0;
        download.current_file.clear();
        download.error = error;
        if download.error.is_empty() && download.done_files == 0 && !download.failures.is_empty() {
            download.error = download.failures[0].error.clone();
        }
        download.status = if download.error.is_empty() && download.failures.is_empty() {
            Status::Completed
        } else if download.done_files > 0 && download.zip_name.is_none() {
            Status::Partial
        } else {
            Status::Failed
        };
        task.run.generation.clear();
        self.notify();
        Ok(true)
    }
}

fn failure(task: &mut Task, path: &str, error: plain_libs::anyhow::Error) {
    task.run.download.failures.push(Failure {
        path: path.into(),
        error: error.to_string(),
    });
    task.run.download.downloaded_bytes -= task.current_bytes;
    task.current_bytes = 0;
    task.last_bytes = task.run.download.downloaded_bytes;
    task.last = Instant::now();
    task.run.download.bytes_per_second = 0;
}

fn active<'a>(state: &'a mut State, id: &str, generation: &str) -> Option<&'a mut Task> {
    let task = state.tasks.get_mut(id)?;
    (!generation.is_empty()
        && task.run.generation == generation
        && matches!(
            task.run.download.status,
            Status::Downloading | Status::Packing
        ))
    .then_some(task)
}

pub(super) fn advance(task: &mut Task, bytes: u64, elapsed: std::time::Duration) -> Result<bool> {
    let bytes = i64::try_from(bytes)?;
    ensure!(
        bytes >= task.current_bytes && bytes <= task.current_size,
        "Invalid download progress"
    );
    task.run.download.downloaded_bytes = task
        .run
        .download
        .downloaded_bytes
        .checked_add(bytes - task.current_bytes)
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Download size overflow"))?;
    task.current_bytes = bytes;
    let report = elapsed >= std::time::Duration::from_millis(300) || bytes == task.current_size;
    if report {
        if !elapsed.is_zero() {
            task.run.download.bytes_per_second = ((task.run.download.downloaded_bytes
                - task.last_bytes) as f64
                / elapsed.as_secs_f64()) as i64;
        }
        task.last_bytes = task.run.download.downloaded_bytes;
        task.last = Instant::now();
    }
    Ok(report)
}
