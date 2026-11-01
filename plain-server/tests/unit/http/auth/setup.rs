use super::*;

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn setup_route_uses_the_real_listener_scheme_and_persists_across_restart() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    plain_rs::features::prefs::security::configure_password_login(&state.prefs).unwrap();
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let client = reqwest::Client::builder()
        .add_root_certificate(
            reqwest::Certificate::from_pem(certificate.cert.pem().as_bytes()).unwrap(),
        )
        .build()
        .unwrap();
    let (http, https) = core
        .start_public(
            0,
            0,
            certificate.cert.pem().into_bytes(),
            certificate.key_pair.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
    for (scheme, port, expected) in [("http", http, 403), ("https", https, 204)] {
        let response = client
            .post(format!("{scheme}://localhost:{port}/auth/setup"))
            .header("x-forwarded-proto", "https")
            .header("Content-Type", "application/json")
            .body(r#"{"password":"owned-password"}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), expected);
    }
    assert_eq!(
        client
            .post(format!("https://localhost:{https}/auth/setup"))
            .header("Content-Type", "application/json")
            .body(r#"{"password":"replacement"}"#)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        409
    );
    core.stop_core().await;
    drop(state);
    drop(core);
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    plain_rs::features::prefs::security::configure_password_login(&state.prefs).unwrap();
    assert!(!plain_rs::features::prefs::security::password_setup_required(&state.prefs).unwrap());
    assert_eq!(
        state.prefs.get::<String>("password").unwrap().as_deref(),
        Some("owned-password")
    );
    core.stop_core().await;
}

#[tokio::test]
async fn password_setup_requires_tls_and_cannot_replace_an_existing_password() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    plain_rs::features::prefs::security::configure_password_login(&state.prefs).unwrap();
    let request = || {
        Json(SetupRequest {
            password: "owned-password".into(),
        })
    };
    assert_eq!(
        setup(
            State(state.clone()),
            Extension(ConnectionScheme("http")),
            request()
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert!(plain_rs::features::prefs::security::password_setup_required(&state.prefs).unwrap());
    assert_eq!(
        setup(
            State(state.clone()),
            Extension(ConnectionScheme("https")),
            Json(SetupRequest {
                password: " ".into()
            })
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        setup(
            State(state.clone()),
            Extension(ConnectionScheme("https")),
            request()
        )
        .await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        setup(
            State(state.clone()),
            Extension(ConnectionScheme("https")),
            Json(SetupRequest {
                password: "replacement".into()
            })
        )
        .await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        state.prefs.get::<String>("password").unwrap().as_deref(),
        Some("owned-password")
    );
    assert!(!state.prefs.get_or("login_require_approval", true));
    core.stop_core().await;
}
