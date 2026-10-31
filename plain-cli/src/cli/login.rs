use crate::login_socket as login;
use anyhow::Result;
use plain_rs::platform::system::consts::AppPaths;
use plain_server::native::management::NativeWebLoginRequests;
use std::io::{self, Write};

#[derive(Debug)]
pub enum Command {
    Requests { after: Option<String> },
    Allow { request_id: String },
    Deny { request_id: String },
}

pub async fn run(paths: &AppPaths, command: Command) -> Result<()> {
    match command {
        Command::Requests { after } => {
            let requests = login::requests(&paths.data_dir, after).await?;
            display_requests(&mut io::stdout().lock(), &requests)?;
        }
        Command::Allow { request_id } => {
            login::decide(&paths.data_dir, request_id, true).await?;
            println!("Login request allowed.");
        }
        Command::Deny { request_id } => {
            login::decide(&paths.data_dir, request_id, false).await?;
            println!("Login request denied.");
        }
    }
    Ok(())
}

fn display_requests(output: &mut impl Write, page: &NativeWebLoginRequests) -> io::Result<()> {
    if page.requests.is_empty() {
        writeln!(output, "No pending login requests.")?;
    }
    for request in &page.requests {
        writeln!(output, "Request {:?}", request.request_id)?;
        writeln!(
            output,
            "  Client: {:?} at {:?}",
            request.client_id, request.client_ip
        )?;
        writeln!(
            output,
            "  Browser: {:?} {:?}",
            request.browser_name, request.browser_version
        )?;
        writeln!(
            output,
            "  OS: {:?} {:?}",
            request.os_name, request.os_version
        )?;
        if let Some(code) = &request.verification_code {
            writeln!(output, "  Verification code: {code:?}")?;
        }
    }
    if !page.requests.is_empty() {
        writeln!(
            output,
            "Compare the code on both devices, then use plainapp login allow <request-id> or plainapp login deny <request-id> with the same data directory."
        )?;
    }
    if let Some(cursor) = &page.next_after {
        writeln!(
            output,
            "More requests are available; use --after {cursor:?}."
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/cli/login.rs"]
mod tests;
