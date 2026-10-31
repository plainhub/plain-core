use crate::server::ContentServer;
use crate::test_support::graphql_http as call;
use std::sync::Arc;

#[tokio::test]
async fn projects_persist_project_summaries_and_validate_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plain.db");
    let token = plain_rs::utils::base64::base64_encode(&[12; 32]);
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let server = ContentServer::start(path.parent().unwrap(), &token, prefs.clone()).unwrap();
    let save = r#"mutation {saveImageEditorProject(id:"",input:{stateB64:"AQID",thumbnail:"data:image/jpeg;base64,AQID",canvasWidth:100,canvasHeight:200,layerCount:2}) {id stateB64 thumbnail createdAt updatedAt}}"#;
    let result = call(server.port, &token, save).await;
    assert!(result.get("errors").is_none(), "{result}");
    let saved = &result["data"]["saveImageEditorProject"];
    let id = saved["id"].as_str().unwrap();
    assert!(!id.is_empty());
    let update = format!(
        r#"mutation {{saveImageEditorProject(id:"{id}",input:{{stateB64:"BAU",canvasWidth:300,canvasHeight:200,layerCount:3}}) {{id stateB64 createdAt}}}}"#
    );
    let updated = call(server.port, &token, &update).await;
    assert_eq!(
        updated["data"]["saveImageEditorProject"]["createdAt"],
        saved["createdAt"]
    );
    assert_eq!(updated["data"]["saveImageEditorProject"]["stateB64"], "BAU");
    for (state, width) in [("!", 100), ("AQID", -1)] {
        let invalid = format!(
            r#"mutation {{saveImageEditorProject(id:"invalid",input:{{stateB64:"{state}",canvasWidth:{width},canvasHeight:1,layerCount:1}}) {{id}}}}"#
        );
        assert!(
            call(server.port, &token, &invalid)
                .await
                .get("errors")
                .is_some()
        );
    }
    for n in 0..55 {
        let save = format!(
            r#"mutation {{saveImageEditorProject(id:"project-{n}%_",input:{{stateB64:"AQID",canvasWidth:1,canvasHeight:1,layerCount:1}}) {{id}}}}"#
        );
        assert!(
            call(server.port, &token, &save)
                .await
                .get("errors")
                .is_none()
        );
    }
    server
        .runtime_state()
        .db
        .with_conn(|connection| -> rusqlite::Result<()> {
            for n in 0..55 {
                connection.execute(
                    "UPDATE image_editor_projects SET updated_at=?1 WHERE id=?2",
                    rusqlite::params![
                        format!("2099-01-01T00:00:{n:02}.000Z"),
                        format!("project-{n}%_")
                    ],
                )?;
            }
            Ok(())
        })
        .unwrap();
    let list = call(server.port, &token, "{imageEditorProjects {id}}").await;
    assert!(list.get("errors").is_none(), "{list}");
    let rows = list["data"]["imageEditorProjects"].as_array().unwrap();
    assert_eq!(rows.len(), 50);
    assert_eq!(rows[0]["id"], "project-54%_");
    assert_eq!(rows[49]["id"], "project-5%_");
    server.shutdown().await;
    let server = ContentServer::start(path.parent().unwrap(), &token, prefs).unwrap();
    let read = call(
        server.port,
        &token,
        &format!(r#"{{imageEditorProject(id:"{id}") {{id stateB64 canvasWidth}}}}"#),
    )
    .await;
    assert_eq!(read["data"]["imageEditorProject"]["stateB64"], "BAU");
    assert_eq!(read["data"]["imageEditorProject"]["canvasWidth"], 300);
    call(
        server.port,
        &token,
        &format!(r#"mutation {{deleteImageEditorProject(id:"{id}")}}"#),
    )
    .await;
    let read = call(
        server.port,
        &token,
        &format!(r#"{{imageEditorProject(id:"{id}") {{id}}}}"#),
    )
    .await;
    assert!(read["data"]["imageEditorProject"].is_null());
    server.shutdown().await;
}

#[tokio::test]
async fn binary_delta_does_not_invalidate_content() {
    let dir = tempfile::tempdir().unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[13; 32]);
    let server = ContentServer::start(
        dir.path(),
        &token,
        Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap()),
    )
    .unwrap();
    let mut socket = server.runtime_state().events.subscribe();
    let result = serde_json::to_value(
        server
            .native_graphql()
            .unwrap()
            .execute(async_graphql::Request::new(
                r#"mutation {broadcastImageEditorUpdate(id:"p",update:"AQID")}"#,
            ))
            .await,
    )
    .unwrap();
    assert!(result.get("errors").is_none(), "{result}");
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), socket.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.event_type, "IMAGE_EDITOR_UPDATE");
    assert_eq!(event.binary_payload.unwrap(), [1, b'p', 1, 2, 3]);
    assert!(socket.try_recv().is_err());
    drop(socket);
    server.shutdown().await;
}
