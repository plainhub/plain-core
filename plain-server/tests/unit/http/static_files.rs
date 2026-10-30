use super::*;

struct Bundle {
    dir: tempfile::TempDir,
    root: PathBuf,
    outside: PathBuf,
}

fn bundle() -> Bundle {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bundle");
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(
        root.join("index.html"),
        "<html><head><title>shell</title></head></html>",
    )
    .unwrap();
    std::fs::write(root.join("assets/app.js"), "console.log(1)").unwrap();
    std::fs::write(outside.join("secret.txt"), "top secret").unwrap();
    Bundle { dir, root, outside }
}

struct MemoryBundle(&'static [(&'static str, &'static [u8])]);

impl crate::http::web_bundle::WebBundle for MemoryBundle {
    fn read(&self, relative: &str) -> Option<&'static [u8]> {
        self.0
            .iter()
            .find(|(key, _)| *key == relative)
            .map(|(_, bytes)| *bytes)
    }
}

fn embedded() -> SharedWebBundle {
    std::sync::Arc::new(MemoryBundle(&[
        (
            "index.html",
            b"<html><head><title>embedded</title></head></html>",
        ),
        ("assets/app.js", b"console.log(2)"),
    ]))
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

async fn get(root: &Path, path: &str) -> Option<Response> {
    let bundle = embedded();
    get_from(Some(root), Some(&bundle), path).await
}

async fn get_from(
    root: Option<&Path>,
    bundle: Option<&SharedWebBundle>,
    path: &str,
) -> Option<Response> {
    let dir = tempfile::tempdir().unwrap();
    let prefs = plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap();
    serve(&prefs, root, bundle, &Method::GET, path).await
}

async fn text(response: Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a complete body");
    String::from_utf8_lossy(&bytes).into_owned()
}

#[test]
fn client_paths_cannot_escape_the_bundle_root() {
    assert!(
        matches!(classify("assets/app.js"), Requested::Relative(path) if path == PathBuf::from("assets/app.js"))
    );
    assert!(
        matches!(classify("/index.html"), Requested::Relative(path) if path == PathBuf::from("index.html"))
    );
    assert!(matches!(classify(""), Requested::Root));
    assert!(matches!(classify("/"), Requested::Root));
    for rejected in [
        "..",
        "../secret",
        "assets/../../secret",
        "assets\\..\\..\\secret",
    ] {
        assert!(
            matches!(classify(rejected), Requested::Rejected),
            "{rejected}"
        );
    }
}

#[test]
fn a_directory_is_not_a_bundle_file() {
    let bundle = bundle();
    let root = canonical_root(&bundle.root).expect("the bundle root exists");
    assert!(bundle_file(&root, Path::new("assets")).is_none());
    assert!(bundle_file(&root, Path::new("assets/app.js")).is_some());
}

#[test]
fn hashed_asset_prefixes_are_immutable_and_the_shell_is_not() {
    assert_eq!(
        cache_control("assets/index-abc123.js"),
        "public, max-age=31536000"
    );
    assert_eq!(cache_control("ficons/note.svg"), "public, max-age=31536000");
    assert_eq!(cache_control("index.html"), "no-cache, no-store");
    assert_eq!(cache_control("favicon.ico"), "no-cache, no-store");
}

#[test]
fn server_time_is_injected_into_the_spa_shell() {
    let html = inject_server_time("<html><head><title>x</title></head></html>", 42);
    assert!(html.starts_with("<html><head><script>window.__SERVER_TIME__=42</script>"));
    // A shell without <head> still gets the bootstrap rather than losing it.
    assert!(
        inject_server_time("<div/>", 7).starts_with("<script>window.__SERVER_TIME__=7</script>")
    );
}

#[test]
fn content_types_cover_the_bundler_output() {
    assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
    assert_eq!(
        content_type("assets/app-abc.js"),
        "application/javascript; charset=utf-8"
    );
    assert_eq!(
        content_type("assets/app-abc.css"),
        "text/css; charset=utf-8"
    );
    assert_eq!(content_type("ficons/note.svg"), "image/svg+xml");
    assert_eq!(content_type("logo.png"), "image/png");
    assert_eq!(content_type("Inter.woff2"), "font/woff2");
    assert_eq!(content_type("noext"), "application/octet-stream");
}

#[tokio::test]
async fn extension_less_routes_fall_back_to_the_shell_and_missing_assets_do_not() {
    let bundle = bundle();
    let route = get(&bundle.root, "/settings/profile")
        .await
        .expect("a client route serves the shell");
    assert_eq!(route.status(), StatusCode::OK);
    let body = text(route).await;
    assert!(body.contains("<title>shell</title>"), "{body}");
    assert!(body.contains("window.__SERVER_TIME__"), "{body}");
    assert!(get(&bundle.root, "/assets/missing.js").await.is_none());
    assert!(get(&bundle.root, "/assets/app.js").await.is_some());
}

#[tokio::test]
async fn a_rejected_path_is_not_served_as_a_client_route() {
    let bundle = bundle();
    assert!(get(&bundle.root, "/../outside/secret.txt").await.is_none());
    assert!(
        get(&bundle.root, "/assets/../../outside/secret.txt")
            .await
            .is_none()
    );
}

#[tokio::test]
async fn the_access_switch_and_the_method_gate_every_asset() {
    let bundle = bundle();
    let dir = tempfile::tempdir().unwrap();
    let prefs = plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap();
    prefs
        .set("desktop_access", serde_json::json!(false))
        .unwrap();
    assert!(
        serve(
            &prefs,
            Some(&bundle.root),
            None,
            &Method::GET,
            "/assets/app.js"
        )
        .await
        .is_none()
    );
    assert!(
        serve(&prefs, Some(&bundle.root), None, &Method::POST, "/")
            .await
            .is_none()
    );
}

#[tokio::test]
async fn an_embedded_bundle_serves_the_ui_without_a_directory() {
    let bundle = embedded();
    let shell = get_from(None, Some(&bundle), "/").await.expect("the shell");
    assert_eq!(shell.status(), StatusCode::OK);
    assert!(text(shell).await.contains("<title>embedded</title>"));
    let asset = get_from(None, Some(&bundle), "/assets/app.js")
        .await
        .expect("an embedded asset");
    assert!(text(asset).await.contains("console.log(2)"));
    let route = get_from(None, Some(&bundle), "/settings/profile")
        .await
        .expect("a client route falls back to the shell");
    assert!(text(route).await.contains("<title>embedded</title>"));
    assert!(
        get_from(None, Some(&bundle), "/assets/missing.js")
            .await
            .is_none()
    );
    assert!(
        get_from(None, Some(&bundle), "/../secret.txt")
            .await
            .is_none()
    );
    assert!(
        get_from(None, Some(&bundle), "/assets/../../secret.txt")
            .await
            .is_none()
    );
    assert!(get_from(None, None, "/").await.is_none());
}

#[tokio::test]
async fn a_web_directory_overrides_the_embedded_bundle() {
    let bundle = bundle();
    let embedded = embedded();
    let on_disk = get_from(Some(&bundle.root), Some(&embedded), "/")
        .await
        .expect("the directory serves the shell");
    assert!(text(on_disk).await.contains("<title>shell</title>"));
    let on_disk = get_from(Some(&bundle.root), Some(&embedded), "/assets/app.js")
        .await
        .expect("the directory serves its own asset");
    assert!(text(on_disk).await.contains("console.log(1)"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_that_stays_inside_the_bundle_is_still_served() {
    let bundle = bundle();
    symlink(
        &bundle.root.join("assets/app.js"),
        &bundle.root.join("assets/copy.js"),
    );
    let response = get(&bundle.root, "/assets/copy.js")
        .await
        .expect("a symlink inside the bundle stays usable");
    assert!(text(response).await.contains("console.log"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_that_leaves_the_bundle_is_refused() {
    let bundle = bundle();
    symlink(
        &bundle.outside.join("secret.txt"),
        &bundle.root.join("assets/leak.txt"),
    );
    assert!(get(&bundle.root, "/assets/leak.txt").await.is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn the_shell_cannot_be_redirected_out_of_the_bundle() {
    let bundle = bundle();
    std::fs::write(
        bundle.outside.join("shell.html"),
        "<html>secret shell</html>",
    )
    .unwrap();
    std::fs::remove_file(bundle.root.join("index.html")).unwrap();
    symlink(
        &bundle.outside.join("shell.html"),
        &bundle.root.join("index.html"),
    );
    assert!(get(&bundle.root, "/").await.is_none());
    assert!(get(&bundle.root, "/chat").await.is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn a_bundle_root_reached_through_a_symlink_still_serves() {
    let bundle = bundle();
    let link = bundle.dir.path().join("link");
    symlink(&bundle.root, &link);
    assert!(get(&link, "/assets/app.js").await.is_some());
    assert!(get(&link, "/").await.is_some());
}

#[cfg(unix)]
#[tokio::test]
async fn serves_a_read_only_packaged_bundle_without_staging_or_a_completion_stamp() {
    use std::os::unix::fs::PermissionsExt;

    let bundle = bundle();
    let source = std::fs::read(bundle.root.join("index.html")).unwrap();
    std::fs::set_permissions(&bundle.root, std::fs::Permissions::from_mode(0o555)).unwrap();
    std::fs::set_permissions(
        bundle.root.join("assets"),
        std::fs::Permissions::from_mode(0o555),
    )
    .unwrap();
    let shell = get_from(Some(&bundle.root), None, "/").await;
    let asset = get_from(Some(&bundle.root), None, "/assets/app.js").await;
    std::fs::set_permissions(&bundle.root, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(
        bundle.root.join("assets"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();

    assert_eq!(shell.as_ref().unwrap().status(), StatusCode::OK);
    assert!(
        text(shell.unwrap())
            .await
            .contains("window.__SERVER_TIME__=")
    );
    assert_eq!(text(asset.unwrap()).await, "console.log(1)");
    assert_eq!(
        std::fs::read(bundle.root.join("index.html")).unwrap(),
        source
    );
    assert_eq!(std::fs::read_dir(&bundle.root).unwrap().count(), 2);
    assert!(!bundle.root.join(".complete").exists());
}
