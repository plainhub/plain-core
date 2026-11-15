use plain_libs::anyhow::Result;

mod bootstrap;
mod cli;
mod host;
mod management_socket;
mod service;
mod storage;
mod version;

fn main() -> Result<()> {
    bootstrap::run()
}
