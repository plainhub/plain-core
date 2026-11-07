# plain-core

Shared Rust code for PlainApp desktop, mobile and command-line hosts.

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

Tests live under `tests/plain-libs/`, `tests/plain-rs/`, `tests/plain-server/` and `tests/plain-cli/`.
Each contains its own unit tests and any shared fixtures or support code.
Unit tests remain compiled inside their owning package; HTTP integration tests
are registered explicitly in that package's `Cargo.toml`.

`plain-libs` owns reusable components and depends only on third-party libraries.
Its optional Cargo features select components without importing application state,
models, host commands or GraphQL. `plain-rs` owns product behavior in `features/`
and application storage, host contracts and protocol policy in `infrastructure/`.
Callers import reusable components directly, such as `plain_libs::crypto`, and
application modules through `plain_rs::features` or `plain_rs::infrastructure`.
Unit-test paths mirror each package's source tree under `tests/<package>/unit/`.

Each `plain-*` directory is a Rust package managed by the root `Cargo.toml`.
Desktop shell and mobile JNI/C ABI code live in plain-desktop and plain-app, respectively.

## Source layout

HTTP entry points are named after the resources they serve. Start with
[`plain-server/src/http/router.rs`](plain-server/src/http/router.rs) to find a URL's
method, handler, request limits and middleware.

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
│   │   ├── proxy.rs          /proxyfs and proxy transport
│   │   ├── proxy/local/      Local proxy support
│   │   ├── graphql.rs        Main, peer and guest GraphQL HTTP adapters
│   │   ├── init.rs           /init
│   │   ├── nearby.rs         /nearby
│   │   ├── health.rs         Health and listener readiness handlers
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

For example, `/fs` is registered in `http/router.rs`, handled by
[`http/fs.rs`](plain-server/src/http/fs.rs), and uses
[`http/file_response.rs`](plain-server/src/http/file_response.rs) for local file
streaming and Range responses. The handler parses HTTP options, calls
[`plain-rs/features/services/files/links.rs`](plain-rs/src/features/services/files/links.rs) and
[`plain-rs/features/services/files/content.rs`](plain-rs/src/features/services/files/content.rs), then
maps the result to an HTTP response. Conversions and temporary-file ownership live in
[`plain-rs/features/services/files/previews.rs`](plain-rs/src/features/services/files/previews.rs).
[`plain-rs/features/services/files/references.rs`](plain-rs/src/features/services/files/references.rs)
resolves local paths, `app://` paths and `fid:` references. HTTP Range, thumbnail
ETag/304 responses and download headers stay in `http/file_response.rs`.
GraphQL HTTP extraction and response handling belong to `http/graphql.rs`;
schema definitions and resolvers belong to `graphql/`. Native and HTTP use the same main schema on every host. Capabilities and
permissions control operation support and authorization.

`plain-rs/src/runtime.rs` owns each core service once. `plain-server/src/server.rs`
adds schemas and transport connections to that runtime. HTTP listener startup and
configuration live in `http/listeners/public_server.rs`, failure monitoring and
shutdown in `lifecycle.rs`, and restart commands in `control.rs`. There is no
separate assembly directory or duplicate service container.

`plain-cli/src/service.rs` combines the server with CLI host adapters and local
approval. Linux mounting and disk formatting implementations remain in
`plain-rs/src/infrastructure/platform/linux/`. `plain-server` contains no CLI/headless runtime;
its native APIs and `ContentServer::runtime()` provide the in-process entry points
used by CLI, desktop and mobile hosts.

`router.rs::transfer_routes` contains the `/fs`, `/peer_graphql` and
`/guest_graphql` routes shared by HTTP and BLE. `http/ble.rs` reuses that router;
it does not maintain a second set of handlers.

## Build

Requires Rust 1.99+ and a C compiler. Building ONNX Runtime from source requires Python 3.10+.

To build with an already prepared ONNX Runtime, set `PLAIN_ONNX_RUNTIME_DIR` to its artifact directory and `PLAIN_ONNX_RUNTIME_SHA256` to the library digest from a trusted build record. Cargo uses CMake 3.19+ to validate the artifact, target, source recipe and notices without running Python.

```sh
cargo check --workspace --locked    # Check all packages for compile errors
cargo test --workspace --locked     # Test Rust functionality in all packages
cargo run -p plainapp -- run --help  # Build plainapp and show run-command help
```

The dev profile uses incremental compilation without optimization for domain services,
server adapters and the CLI. Reusable components and third-party dependencies retain
their configured optimization levels. Use release builds for runtime performance measurements.
Consumer workspaces must configure their own profiles; dependency profiles are not inherited.

## Generated contracts

Regenerate GraphQL and SQLite artifacts from the current Rust implementation with Node.js and Rust:

```sh
node scripts/gen-schema.mjs --write
```

The command writes `schema/database.sql`, the main schema fragments in `schema/api/`, and `schema/guest-api.graphqls` and `schema/peer-api.graphqls`. It builds and runs the three schema snapshot checks. Omit `--write` to verify the generated files without updating them.

Desktop consumes these same fragments. After a contract change, run `yarn graphql:codegen` in the sibling `plain-desktop` checkout to refresh its operation types. Mobile reads the Core schema directly and keeps no separate SDL copy.

## Deploy the CLI

`plainapp` is a single executable with the web UI compiled into it, and a Linux host runs it as the systemd service `plainapp`.

### One-line install (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/plainhub/plain-core/main/install.sh | bash
```

It detects the architecture, downloads the matching release, installs the dependencies and the systemd unit, starts the service, and prints the addresses to open.

```bash
sudo plainapp passwd   # admin password, prompted twice
```

- `http://<host>:8080` — HTTP
- `https://<host>:8443` — HTTPS with a self-signed certificate created on first start for `plainapp.local` and `localhost`

### Install manually

```sh
cargo build --release --locked -p plainapp
install -m 0755 target/release/plainapp /usr/local/bin/plainapp
sudo plainapp install                     # ffmpeg and Avahi
sudo systemctl enable --now plainapp
```

`plainapp install` writes `/etc/systemd/system/plainapp.service`, whose `ExecStart` is `/usr/local/bin/plainapp run`, so that path is required. It does not copy the binary, and it neither enables nor starts the service.

The Web UI is compiled into `plain-server` and shared by the CLI and mobile libraries. From the sibling `plain-desktop` checkout, run `bash build.sh` to build directly into `plain-core/assets/web`, then commit the generated assets with the corresponding core changes. Consumers receive the assets through their pinned core Git dependency; no separate mobile Web resource copy is needed.

Set `PLAIN_WEB_DIST` to an absolute frontend build directory to override the embedded build input. Mobile Gradle builds also accept `-PplainWebDist=/absolute/path/to/dist`. Explicit inputs must exist and contain `index.html`; invalid inputs fail the build. A checkout without the default `assets/web` directory and without an override builds with API support only.

### Configuration

For local frontend tests, `run --web-root <directory>` serves a bundle from disk instead of the compiled one without rebuilding the server.

Ports come from user settings and default to `8080` and `8443`. Data defaults to `$HOME/.plainapp/data`; cache uses an application directory under the OS cache location. The installed service runs as root.

Linux root services automatically mount eligible unmounted filesystems at stable `/mnt/usbX` paths and monitor block-device changes.

Configure paths with the `PLAINAPP_DATA_DIR`, `PLAINAPP_CACHE_DIR`, `PLAINAPP_TLS_CERT` and `PLAINAPP_TLS_KEY` environment variables. For isolated test instances, `run` accepts `--data-dir`, `--cache-dir`, repeated `--media-dir` and `--web-root`. `passwd` accepts data/cache directories; `uninstall` and `login` accept a data directory. Installation, update and benchmark commands accept no path overrides.

### Upgrade and removal

`plainapp update` only reports that self-update is not implemented, so an upgrade replaces the executable and restarts the service:

```sh
install -m 0755 new/plainapp /usr/local/bin/plainapp
sudo systemctl restart plainapp
```

`sudo plainapp uninstall` removes the unit, the certificate and the data directory. Pass `--keep-config` and `--keep-data` to retain them.

### Releases

A `v*` tag builds the frontend from plain-desktop, compiles it into `plainapp` for Linux amd64 and arm64, and attaches the archives to the GitHub release.

## Work on desktop or app

Keep plain-core next to plain-desktop or plain-app. Run these commands in the desktop or app repository; its `scripts/core` calls `plain-core/scripts/core-dependencies.py`.

```bash
./scripts/core local        # Use local plain-core
./scripts/core local --off  # Stop using local paths
./scripts/core update       # Lock this project to the latest remote commit
```
