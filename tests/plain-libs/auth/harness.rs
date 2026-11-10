use std::io::{self, BufRead, Write};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode(line: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let line = line.trim();
    if !line.is_ascii() || line.len() % 2 != 0 {
        return Err("invalid test input".into());
    }
    (0..line.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&line[i..i + 2], 16).map_err(Into::into))
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let identity = [7u8; 32];
    let context = plain_libs::auth::context("wasm-test-client", &identity);
    let registration = plain_libs::auth::server::register(b"test password for OPAQUE", &identity)?;
    println!(
        "{}",
        hex(&plain_libs::auth::server::public_key(&registration.setup)?)
    );
    io::stdout().flush()?;
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let request = decode(&lines.next().ok_or("missing KE1")??)?;
    let (login, response) = plain_libs::auth::server::start(
        &registration.setup,
        &registration.record,
        &request,
        &context,
        &identity,
    )?;
    println!("{}", hex(&response));
    io::stdout().flush()?;
    if let Some(line) = lines.next() {
        let key = login.finish(&decode(&line?)?)?;
        println!("{}", hex(&plain_libs::auth::api_key(&key, &context)));
        println!("{}", hex(&plain_libs::auth::auth_aad(&context, false)));
        println!("{}", hex(&plain_libs::auth::auth_aad(&context, true)));
        io::stdout().flush()?;
    }
    Ok(())
}
