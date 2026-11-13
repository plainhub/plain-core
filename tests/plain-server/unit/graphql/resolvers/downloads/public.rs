use super::*;
use plain_graphql::{EmptySubscription, Request, Schema};
use plain_libs::serde_json::json;

#[tokio::test]
async fn source_validation_rejects_missing_and_competing_sources_before_side_effects() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::start(
        directory.path(),
        &plain_libs::utils::base64::base64_encode(&[7; 32]),
        std::sync::Arc::new(
            plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
        ),
    )
    .unwrap();
    let state = server.runtime_state();
    let schema = Schema::build(DownloadQuery, DownloadMutation, EmptySubscription)
        .data(state.clone())
        .finish();
    for input in [
        json!({}),
        json!({"resources":{"urls":[],"targetDir":""},"chat":{"messageId":"message","peerId":"peer","fileIds":["file"]}}),
    ] {
        let result = schema
            .execute(
                Request::new("mutation($input:DownloadInput!){createDownload(input:$input){id}}")
                    .variables(plain_graphql::Variables::from_json(json!({"input":input}))),
            )
            .await;
        assert_eq!(result.errors.len(), 1);
        assert!(
            result.errors[0]
                .message
                .contains("Exactly one download source")
        );
        assert!(state.downloads.snapshot().is_empty());
    }
    let result = schema.execute("query { downloads { id } }").await;
    assert!(result.errors.is_empty());
    assert_eq!(result.data.into_json().unwrap(), json!({"downloads":[]}));
    for field in [
        "pauseDownload",
        "resumeDownload",
        "retryDownload",
        "cancelDownload",
        "removeDownload",
    ] {
        let response = schema
            .execute(format!("mutation {{ {field}(id: \"missing\") }}"))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()[field], false);
        assert!(state.downloads.snapshot().is_empty());
    }
    server.shutdown().await;
}

#[test]
fn public_contract_has_one_task_control_family_and_typed_sources() {
    let schema = Schema::build(DownloadQuery, DownloadMutation, EmptySubscription).finish();
    let sdl = schema.sdl();
    for field in [
        "createDownload(input: DownloadInput!): Download!",
        "pauseDownload(id: ID!): Boolean!",
        "resumeDownload(id: ID!): Boolean!",
        "retryDownload(id: ID!): Boolean!",
        "cancelDownload(id: ID!): Boolean!",
        "removeDownload(id: ID!): Boolean!",
        "downloads: [Download!]!",
        "source: DownloadSource!",
        "status: DownloadStatus!",
        "downloadedBytes: Long!",
        "failures: [DownloadFailure!]!",
    ] {
        assert!(sdl.contains(field), "Missing {field}");
    }
    for retired in [
        "downloadPeerFile",
        "enqueueChatDownload",
        "pauseChatDownload",
        "resumeChatDownload",
        "retryChatDownload",
        "removeChatDownload",
        "chatDownloadQueue",
        "generation",
        "runtimeId",
        "revision",
    ] {
        assert!(!sdl.contains(retired), "Retired field {retired}");
    }
    for status in [
        "PENDING",
        "DOWNLOADING",
        "PAUSED",
        "PACKING",
        "COMPLETED",
        "PARTIAL",
        "FAILED",
        "CANCELED",
    ] {
        assert!(sdl.contains(status));
    }
}

#[tokio::test]
async fn source_union_and_status_enum_are_exact_and_retired_arguments_are_rejected() {
    let schema = Schema::build(DownloadQuery, DownloadMutation, EmptySubscription).finish();
    let response = schema.execute("{ statuses:__type(name:\"DownloadStatus\"){enumValues{name}} sources:__type(name:\"DownloadSource\"){possibleTypes{name}} }").await;
    assert!(response.errors.is_empty());
    let data = response.data.into_json().unwrap();
    let names = |values: &plain_libs::serde_json::Value| {
        let mut names: Vec<_> = values
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value["name"].as_str().unwrap().to_owned())
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        names(&data["statuses"]["enumValues"]),
        [
            "CANCELED",
            "COMPLETED",
            "DOWNLOADING",
            "FAILED",
            "PACKING",
            "PARTIAL",
            "PAUSED",
            "PENDING"
        ]
    );
    assert_eq!(
        names(&data["sources"]["possibleTypes"]),
        ["ChatDownloadSource", "ResourceDownloadSource"]
    );
    for document in [
        "mutation{pauseDownload(messageId:\"message\")}",
        "mutation{resumeDownload(messageId:\"message\",peerId:\"peer\")}",
        "mutation{createDownload(input:{resources:{urls:[],targetDir:\"\",messageId:\"message\"}}){id}}",
    ] {
        let response = schema.execute(document).await;
        assert!(
            !response.errors.is_empty(),
            "Accepted retired contract: {document}"
        );
        assert!(
            response
                .errors
                .iter()
                .all(|error| !error.message.contains("ServerState")),
            "Validation must precede execution"
        );
    }
}
