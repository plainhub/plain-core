pub trait ClientConnections: Send + Sync {
    fn close(&self, client: Option<&str>);
    fn revoke(&self, client: &str) -> Result<(), String>;
    fn contains(&self, client: &str) -> bool;
}
