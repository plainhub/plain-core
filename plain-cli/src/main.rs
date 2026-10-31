use anyhow::Result;

mod bootstrap;
mod host;
mod login_socket;
mod service;
mod storage;
mod cli;
mod version;

fn main() -> Result<()> {
    bootstrap::run()
}
