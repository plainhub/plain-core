# plain-core

Shared Rust code for PlainApp desktop, mobile and command-line hosts.

```text
plain-core/
├── Cargo.toml                Builds and tests the five packages together
├── plain-rs/                 Domain services, storage, crypto and protocols
├── plain-server/             Server, GraphQL and host communication
├── plain-media/              Media catalog, scanning and processing
├── plain-inference/          Model inference
├── plain-cli/                The plainapp command-line executable
├── schema/                   GraphQL and SQLite definitions
└── scripts/                  Development tools
```

Each `plain-*` directory is a Rust package managed by the root `Cargo.toml`.
Desktop shell and mobile JNI/C ABI code live in plain-desktop and plain-app, respectively.

## Server source layout

HTTP entry points are named after the resources they serve. Start with
[`plain-server/src/http/router.rs`](plain-server/src/http/router.rs) to find a URL's
method, handler, request limits and middleware.

```text
plain-server/
├── src/
│   ├── assembly/             Core construction and server lifecycle
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
│   │   ├── subscriptions/   GraphQL subscriptions
│   │   ├── types/           GraphQL types
│   │   └── executor.rs      Shared schema execution
│   ├── services/            Business operations by domain
│   │   └── files/
│   │       ├── links.rs     Encrypted file links and share reference resolution
│   │       ├── content.rs   File reads, previews and representation selection
│   │       ├── previews.rs  Conversions and temporary preview ownership
│   │       └── references.rs  Local paths, app:// and fid: references
│   ├── host/                Platform requests, resources and event coordination
│   └── native/              Native API entry points
└── tests/unit/              Rust unit tests mirroring src/ paths
```

For example, `/fs` is registered in `http/router.rs`, handled by
[`http/fs.rs`](plain-server/src/http/fs.rs), and uses
[`http/file_response.rs`](plain-server/src/http/file_response.rs) for local file
streaming and Range responses. The handler parses HTTP options, calls
[`services/files/links.rs`](plain-server/src/services/files/links.rs) and
[`services/files/content.rs`](plain-server/src/services/files/content.rs), then
maps the result to an HTTP response. Conversions and temporary-file ownership live in
[`services/files/previews.rs`](plain-server/src/services/files/previews.rs).
[`services/files/references.rs`](plain-server/src/services/files/references.rs)
resolves local paths, `app://` paths and `fid:` references. HTTP Range, thumbnail
ETag/304 responses and download headers stay in `http/file_response.rs`.
GraphQL HTTP extraction and response handling belong to `http/graphql.rs`;
schema definitions and resolvers belong to `graphql/`.

`router.rs::transfer_routes` contains the `/fs`, `/peer_graphql` and
`/guest_graphql` routes shared by HTTP and BLE. `http/ble.rs` reuses that router;
it does not maintain a second set of handlers.

## Build

Requires Rust 1.99+, Python 3.10+ and a C compiler.

```sh
cargo check --workspace --locked    # Check all packages for compile errors
cargo test --workspace --locked     # Test Rust functionality in all packages
cargo run -p plainapp -- run --help  # Build plainapp and show run-command help
```

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
sudo plainapp install                     # ffmpeg, Samba, Avahi
sudo plainapp install --with-libreoffice  # add DOC/DOCX preview
sudo systemctl enable --now plainapp
```

`plainapp install` writes `/etc/systemd/system/plainapp.service`, whose `ExecStart` is `/usr/local/bin/plainapp run`, so that path is required. It does not copy the binary, and it neither enables nor starts the service.

The Web UI is compiled into `plain-server` and shared by the CLI and mobile libraries. From the sibling `plain-desktop` checkout, run `bash build.sh` to build directly into `plain-core/plain-cli/src/web_dist`, then commit the generated assets with the corresponding core changes. Consumers receive the assets through their pinned core Git dependency; no separate mobile Web resource copy is needed.

Set `PLAIN_WEB_DIST` to an absolute frontend build directory to override the embedded build input. Mobile Gradle builds also accept `-PplainWebDist=/absolute/path/to/dist`. Explicit inputs must exist and contain `index.html`; invalid inputs fail the build. A checkout without the default `plain-cli/src/web_dist` directory and without an override builds with API support only.

### Configuration

`run --web-root <directory>` serves a frontend bundle from disk instead of the compiled one, which is how a local frontend build is tried without rebuilding:

```ini
[Service]
ExecStart=/usr/local/bin/plainapp run --web-root /opt/plainapp/web
```

Ports come from user settings and default to `8080` and `8443`. Data and cache live in `$HOME/.plainapp/data` and `$HOME/.plainapp/cache`; the unit sets no `User=`, so it runs as root and uses `/root/.plainapp`. Override with `--data-dir`, `--cache-dir`, `--tls-cert`, `--tls-key` or the `PLAINAPP_DATA_DIR`, `PLAINAPP_CACHE_DIR`, `PLAINAPP_TLS_CERT` and `PLAINAPP_TLS_KEY` environment variables.

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
