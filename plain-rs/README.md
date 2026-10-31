# plain-rs

Shared Rust business code and reusable libraries for Plain projects.
See the [workspace source layout](../README.md#source-layout) for directory
responsibilities, module paths and test placement.

[Cargo.toml](Cargo.toml) owns the optional compilation feature definitions.
Compilation features select code; capabilities determine runtime support and
permissions authorize operations.
