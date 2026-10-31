pub fn uses_system_catalog() -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
}
