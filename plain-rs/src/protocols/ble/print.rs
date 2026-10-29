use crate::protocols::ble::{CCC_DESCRIPTOR_UUID, HTTP_CHAR_UUID, NEARBY_CHAR_UUID, SERVICE_UUID};

pub fn kotlin_uuids_source() -> String {
    format!(
        r#"// GENERATED from plain-rs protocols/ble UUID constants. Do not edit by hand.
// Regenerate: cargo run -p plain-rs --features ble --example ble_uuids
package com.ismartcoding.plain.platform.network.ble

object BleUuids {{
    const val SERVICE_UUID = "{SERVICE_UUID}"
    const val NEARBY_CHAR_UUID = "{NEARBY_CHAR_UUID}"
    const val HTTP_CHAR_UUID = "{HTTP_CHAR_UUID}"
    const val CCC_DESCRIPTOR_UUID = "{CCC_DESCRIPTOR_UUID}"
}}
"#
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/protocols/ble/print.rs"]
mod tests;
