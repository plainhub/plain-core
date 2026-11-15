use crate::management_socket::{self, ListenerState, Status};
use plain_libs::anyhow::Result;
use plain_rs::infrastructure::platform::system::consts::AppPaths;
use std::io::{self, Write};

pub async fn run(paths: &AppPaths) -> Result<()> {
    match management_socket::status(&paths.data_dir).await {
        Ok(status) => display(&mut io::stdout().lock(), &status)?,
        Err(error) if management_socket::not_running(&error) => println!("Service: not running"),
        Err(error) => return Err(error),
    }
    Ok(())
}

fn display(output: &mut impl Write, status: &Status) -> io::Result<()> {
    writeln!(output, "Service: running")?;
    writeln!(output, "PID: {}", status.pid)?;
    let state = match status.listeners {
        ListenerState::Running => "running",
        ListenerState::Restarting => "restarting",
        ListenerState::Stopped => "stopped",
        ListenerState::Failed => "failed",
    };
    writeln!(output, "HTTP server: {state}")?;
    for (scheme, port) in [("http", status.http_port), ("https", status.https_port)] {
        if port == 0 {
            writeln!(output, "{scheme}: not listening")?;
        } else {
            writeln!(output, "{scheme}: {port} ({scheme}://127.0.0.1:{port})")?;
        }
    }
    if !status.error.is_empty() {
        writeln!(output, "Error: {}", status.error)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/plain-cli/unit/cli/status.rs"]
mod tests;
