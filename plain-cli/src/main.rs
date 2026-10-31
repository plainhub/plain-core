use anyhow::Result;

mod bootstrap;
mod cli;
mod host;
mod login_socket;
mod service;
mod storage;
mod version;

fn main() -> Result<()> {
    bootstrap::run()
}
