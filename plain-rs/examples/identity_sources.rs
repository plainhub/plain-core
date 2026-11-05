fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("client") => print!(
            "{}",
            plain_rs::features::client_identity::typescript_source()
        ),
        Some("record") => print!("{}", plain_rs::features::record_id::typescript_source()),
        _ => panic!("Expected client or record"),
    }
}
