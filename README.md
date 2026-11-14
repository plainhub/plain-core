# plain-core

Shared Rust code for PlainApp desktop, mobile and command-line hosts.

## Workspace

```text
plain-core/
├── Cargo.toml                Builds and tests the four packages together
├── plain-libs/               Business-independent algorithms, formats, files and native primitives
├── plain-rs/                 Domain services, application storage and runtime composition
├── plain-server/             GraphQL, HTTP and native transport adapters
├── plain-cli/                The plainapp command-line executable
├── tests/                    Tests grouped by package; unit paths mirror src/
├── assets/                   Shared Web bundle and canonical file icons
├── schema/                   GraphQL and SQLite definitions
└── scripts/                  Development tools
```

Desktop UI and OS adapters live in plain-desktop; mobile UI and JNI/C ABI adapters live in plain-app.
Native and HTTP callers use the same main GraphQL schema.

## Source layout

```text
plain-rs/src/
├── lib.rs                  Public module entry points and compilation gates
├── runtime.rs              Core construction and worker lifecycle
├── features/               Product behavior and domain operations
│   ├── chat/               Chat delivery and message operations
│   ├── files/              File operations and resource references
│   ├── media/              Media library, playback and catalog state
│   ├── prefs/              Application preferences and identity
│   └── services/           Runtime operations by domain
│       └── files/          Browsing, previews, indexing and transfers
└── infrastructure/         Application storage, host contracts and protocol policy
    ├── db/                 Product schema, models and repositories
    ├── host/               Application host commands, scopes and resource streams
    ├── platform/           Application startup, host facts and system coordination
    ├── protocols/          Peer framing and product discovery policy
    ├── transport/          Authenticated peer transport
    └── data_paths/         Application data roots and ownership

plain-libs/src/
├── crypto/                 Encryption, signatures, hashes and key formats
├── media/                  Format parsing, decoding and thumbnails
├── ai_image_search/        Model validation, preprocessing and inference
├── apk/                    Android binary XML and resource parsing
├── tls/                    Certificates and PKCS#12
├── dlna/                   UPnP/DLNA parsing and protocol encoding
├── protocols/mdns/         DNS/mDNS wire codecs and records
├── phone/                  Number parsing and offline metadata lookup
├── filesystem/             File operations and native watching
├── kv.rs                   Generic SQLite key-value storage
├── platform/               Native network, volume and filesystem primitives
└── utils/                  Parsing, encoding, time and file utilities

plain-cli/src/
├── bootstrap.rs            Process initialization and command dispatch
├── cli/                    Command parsing and terminal interaction
├── service.rs              Background service startup, shutdown and failure handling
├── host.rs                 CLI host capabilities and event handling
├── host/linux.rs           Linux host command adapters
├── login_socket.rs         Local login approval socket server and client
└── storage.rs              Startup media sources and mount event wiring

plain-server/
├── src/
│   ├── server.rs             Compose core runtime, schemas and transport state
│   ├── http/
│   │   ├── router.rs         All HTTP route registrations
│   │   ├── fs.rs             /fs: files and previews
│   │   ├── file_response.rs  Shared file streaming, Range and response headers
│   │   ├── zip.rs            /zip/dir and /zip/files
│   │   ├── upload.rs         /upload and /upload_chunk
│   │   ├── managed_proxy.rs  /proxy/{peerId}/*: GraphQL, files, uploads and sockets to known devices
│   │   ├── managed_proxy/    Raw relay, attachment, upload and socket proxy transports
│   │   ├── graphql.rs        Main, peer and guest GraphQL HTTP adapters
│   │   ├── init.rs           /init
│   │   ├── nearby.rs         /nearby
│   │   ├── health.rs         Health handler
│   │   ├── dlna.rs           DLNA description, control, events, media and callbacks
│   │   ├── static_files.rs   Web UI fallback and static files
│   │   ├── web_bundle.rs     Embedded Web asset access
│   │   ├── ble.rs            BLE adapter for the shared transfer routes
│   │   ├── ble/nearby.rs     BLE nearby-device request handling
│   │   ├── auth/            HTTP authorization and CORS middleware
│   │   ├── listeners/       HTTP/TLS sockets and listener lifecycle
│   │   └── websocket/       WebSocket entry points and transport
│   │       ├── auth.rs      /auth upgrade
│   │       ├── graphql.rs   /graphql upgrade and subscriptions
│   │       ├── status.rs    /status and peer status connections
│   │       ├── screen.rs    /screen upgrade
│   │       ├── image_editor.rs  /image_editor upgrade
│   │       ├── runtime.rs   Shared auth, screen and image-editor connections
│   │       └── frames.rs    WebSocket frame encoding
│   ├── graphql/
│   │   ├── schemas/         Main, peer and guest schema construction
│   │   ├── resolvers/       GraphQL queries and mutations by domain
│   │   │   └── mobile/      Mobile GraphQL adapters (contacts and call log)
│   │   ├── subscriptions/   GraphQL subscriptions
│   │   ├── types/           GraphQL types
│   │   └── executor.rs      Shared schema execution
│   ├── host/events.rs        Host event dispatch into the core services
│   └── native/              Native API entry points

```

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

### Developer mode

The `plain-cli/` package builds the `plainapp` executable. Stop the service before changing its persisted developer mode setting, then start it again:

```sh
sudo systemctl stop plainapp
sudo plainapp dev on       # enable developerMode
sudo systemctl start plainapp
```

Use `sudo plainapp dev off` to disable it while the service is stopped.
For a custom instance, use the same `--data-dir` and `--cache-dir` as `plainapp run`.
`plainapp dev` displays usage; `plainapp dev --help` lists the commands and options.

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
