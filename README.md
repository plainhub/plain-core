# plain-core

Shared Rust code for PlainApp desktop, mobile and command-line hosts.

## Workspace

| Directory | Purpose |
| --- | --- |
| [`plain-libs/`](plain-libs/) | Reusable algorithms, formats and native primitives |
| [`plain-rs/`](plain-rs/) | Domain services, storage and core runtime |
| [`plain-server/`](plain-server/) | GraphQL, HTTP, WebSocket and BLE adapters |
| [`plain-cli/`](plain-cli/) | The `plainapp` executable and CLI host adapters |
| [`plain-graphql/`](plain-graphql/) | GraphQL engine and macros |
| [`tests/`](tests/) | Tests grouped by package; unit paths mirror `src/` |
| [`schema/`](schema/) | Generated GraphQL and SQLite contracts |
| [`assets/`](assets/) | Embedded Web UI and file icons |

Desktop UI and OS adapters live in plain-desktop; mobile UI and JNI/C ABI adapters live in plain-app.
Native and HTTP callers use the same main GraphQL schema.

## Build

Requires Rust 1.99+ and a C compiler. Building ONNX Runtime from source requires Python 3.10+.

For builds with prepared ONNX Runtime artifacts, set `PLAIN_ONNX_RUNTIME_MANIFEST`
to the absolute path of a trusted JSON file mapping Rust target triples to
their library SHA-256 digests. Obtain the manifest independently from the trusted
build record. Place each artifact in a directory named after its target next to
the manifest. The producer prepares each target with the existing recipe:

```sh
python3 plain-libs/native/ai_image_search/prepare_onnxruntime.py --target "$target" --output "$artifact_root/$target"
```

Each target directory contains the native library, `runtime.json`, `LICENSE`
and `ThirdPartyNotices.txt`. Cargo uses CMake 3.19+ to verify the selected
artifact and stage it in the usual build output without running Python.
`PLAIN_ONNX_RUNTIME_DIR` still controls the output directory when set by a consumer.

For a single prepared runtime already in place, set `PLAIN_ONNX_RUNTIME_DIR` and `PLAIN_ONNX_RUNTIME_SHA256` instead.

```sh
cargo check --workspace --locked
cargo test --workspace --locked
cargo run -p plainapp -- run --help
```

Use release builds for performance measurements. Consumer workspaces must configure their own Cargo profiles.

## Generated contracts

Regenerate the contracts with Node.js and Rust:

```sh
node scripts/gen-schema.mjs --write
```

This updates `schema/database.sql`, `schema/api/`, `schema/guest-api.graphqls` and `schema/peer-api.graphqls`. Omit `--write` to check them.
After API changes, run `yarn graphql:codegen` in plain-desktop. Mobile reads the Core schema directly.

## Deploy the CLI

`plainapp` embeds the Web UI and runs as a Linux systemd service.

### Install

```bash
curl -fsSL https://raw.githubusercontent.com/plainhub/plain-core/main/install.sh | bash
```

The installer downloads the matching release, installs dependencies and starts the service.
First installation requires an admin password entered through a terminal; reinstalling preserves it.
For unattended installation, provision it first with `plainapp passwd --stdin` using protected standard input.

```bash
sudo plainapp passwd   # change the admin password while the service is stopped
```

- `http://<host>:8080` — HTTP
- `https://<host>:8443` — HTTPS with a self-signed certificate created on first start for `plainapp.local` and `localhost`

### Install manually

```sh
cargo build --release --locked -p plainapp
install -m 0755 target/release/plainapp /usr/local/bin/plainapp
sudo plainapp install                     # admin password, ffmpeg and Avahi
sudo systemctl enable --now plainapp
```

`plainapp install` configures the password, dependencies and `/etc/systemd/system/plainapp.service`.
The unit runs `/usr/local/bin/plainapp run`; copy the binary and enable the service as shown above.
Listeners require a configured password.

### Web UI

Run `bash build.sh` in plain-desktop to regenerate `assets/web`, then commit the assets with the Core changes.
CLI and mobile builds embed this bundle.

To embed another bundle, set `PLAIN_WEB_DIST` to its absolute directory, or use `-PplainWebDist=...` for mobile Gradle builds.
The directory must contain `index.html`. Without `assets/web` or an override, the build provides APIs only.
For local frontend tests, `run --web-root <directory>` serves files from disk without rebuilding.

### Configuration

The service runs as root. Data defaults to `$HOME/.plainapp/data`; cache uses the OS cache location. Ports default to `8080` and `8443` and can be changed in user settings.

Linux root services automatically mount eligible unmounted filesystems at stable `/mnt/usbX` paths and monitor block-device changes.

Environment variables: `PLAINAPP_DATA_DIR`, `PLAINAPP_CACHE_DIR`, `PLAINAPP_TLS_CERT` and `PLAINAPP_TLS_KEY`.
System service installation requires default data/cache paths and rejects those two environment overrides.

For isolated instances, `run` accepts `--data-dir`, `--cache-dir`, repeated `--media-dir` and `--web-root`.
Use `<command> --help` for options on other commands.

### Upgrade and removal

Self-update is not implemented. Replace the executable and restart:

```sh
install -m 0755 new/plainapp /usr/local/bin/plainapp
sudo systemctl restart plainapp
```

`sudo plainapp uninstall` removes the unit, the certificate and the data directory. Pass `--keep-config` and `--keep-data` to retain them.

### Releases

A `v*` tag builds the frontend from plain-desktop, compiles it into `plainapp` for Linux amd64 and arm64, and attaches the archives to the GitHub release.

## Work on desktop or app

Keep plain-core next to plain-desktop or plain-app. In the consumer repository:

```bash
./scripts/core local        # Use local plain-core
./scripts/core local --off  # Stop using local paths
./scripts/core update       # Lock this project to the latest remote commit
```
