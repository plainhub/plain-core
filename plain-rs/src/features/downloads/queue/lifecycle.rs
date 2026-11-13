use super::*;

impl Queue {
    pub fn control(&self, id: &str, command: Command) -> Result<bool> {
        let mut state = self.state.lock().unwrap();
        ensure!(!state.stopped, "Downloads stopped");
        let Some(task) = state.tasks.get(id) else {
            return Ok(false);
        };
        let status = task.run.download.status;
        let allowed = match command {
            Command::Pause => matches!(
                status,
                Status::Pending | Status::Downloading | Status::Packing
            ),
            Command::Resume => status == Status::Paused,
            Command::Retry => matches!(status, Status::Failed | Status::Partial | Status::Canceled),
            Command::Cancel => !status.terminal(),
            Command::Remove => true,
        };
        if !allowed {
            return Ok(false);
        }
        if matches!(command, Command::Retry) {
            capacity(&state)?;
        }
        let task = state.tasks.get_mut(id).expect("Checked download exists");
        task.run.generation.clear();
        task.current_bytes = 0;
        task.run.download.downloaded_bytes = task.run.completed.values().sum();
        task.run.download.bytes_per_second = 0;
        task.run.download.status = match command {
            Command::Pause => Status::Paused,
            Command::Cancel => Status::Canceled,
            _ => Status::Pending,
        };
        if matches!(command, Command::Retry) {
            task.run.attempt = crate::infrastructure::transport::router::Attempt::Retry;
        }
        if task.run.download.zip_name.is_some() {
            task.run.completed.clear();
            task.run.download.done_files = 0;
            task.run.download.downloaded_bytes = 0;
        }
        if matches!(command, Command::Remove) {
            state.tasks.remove(id);
        }
        state.pending.retain(|pending| pending != id);
        if matches!(command, Command::Resume | Command::Retry) {
            state.pending.push_back(id.into());
        }
        self.notify();
        Ok(true)
    }
    pub fn stop(&self) {
        let mut state = self.state.lock().unwrap();
        state.stopped = true;
        state.pending.clear();
        for task in state
            .tasks
            .values_mut()
            .filter(|task| !task.run.download.status.terminal())
        {
            task.run.generation.clear();
            task.run.download.status = Status::Canceled;
            task.run.download.downloaded_bytes = task.run.completed.values().sum();
            task.run.download.bytes_per_second = 0;
        }
        self.notify();
    }
}
