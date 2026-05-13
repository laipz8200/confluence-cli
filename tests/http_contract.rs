use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use confluence_cli::client::{ConfluenceClient, CreatePageRequest, UpdatePageRequest};
use confluence_cli::config::Config;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::matchers::{basic_auth, body_string, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config(base_url: &str) -> Config {
    Config {
        site_url: base_url.to_string(),
        email: "user@example.com".to_string(),
        api_token: "token-value".to_string(),
        default_space: "ENG".to_string(),
    }
}

#[tokio::test]
async fn list_spaces_calls_v2_spaces_with_auth() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("limit", "25"))
        .and(basic_auth("user@example.com", "token-value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "987", "key": "ENG", "name": "Engineering"}],
            "_links": {}
        })))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let spaces = client.list_spaces().await.unwrap();

    assert_eq!(spaces[0].id, "987");
    assert_eq!(spaces[0].key, "ENG");
}

#[tokio::test]
async fn resolve_space_key_returns_matching_space_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("keys", "ENG"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "987", "key": "ENG", "name": "Engineering"}],
            "_links": {}
        })))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let space_id = client.resolve_space_id("ENG").await.unwrap();

    assert_eq!(space_id, "987");
}

#[tokio::test]
async fn search_uses_v1_cql_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/search"))
        .and(query_param("cql", "text ~ \"deploy\""))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"content": {"id": "123", "title": "Deploy Guide"}}]
        })))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let result = client.search("text ~ \"deploy\"").await.unwrap();

    assert_eq!(result["results"][0]["content"]["id"], "123");
}

#[tokio::test]
async fn create_page_sends_v2_payload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/pages"))
        .and(body_string(
            r#"{"spaceId":"987","status":"current","title":"New Page","parentId":"123","body":{"representation":"storage","value":"<h1>New Page</h1>\n"},"subtype":"live"}"#,
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "456"})))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let result = client
        .create_page(CreatePageRequest {
            space_id: "987".to_string(),
            title: "New Page".to_string(),
            parent_id: Some("123".to_string()),
            storage_html: "<h1>New Page</h1>\n".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(result["id"], "456");
}

#[tokio::test]
async fn update_page_sends_next_version_payload() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/pages/456"))
        .and(body_string(
            r#"{"id":"456","status":"current","title":"Updated Page","body":{"representation":"storage","value":"<h1>Updated Page</h1>\n"},"version":{"number":8,"message":"Updated by confluence-cli"}}"#,
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "456"})))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let result = client
        .update_page(UpdatePageRequest {
            page_id: "456".to_string(),
            title: "Updated Page".to_string(),
            next_version: 8,
            storage_html: "<h1>Updated Page</h1>\n".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(result["id"], "456");
}

#[tokio::test]
async fn status_mapping_returns_stable_error_code() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let error = client.list_spaces().await.unwrap_err();

    assert_eq!(error.code.as_str(), "auth_failed");
}

#[tokio::test]
async fn update_page_conflict_returns_actionable_retryable_error() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/pages/456"))
        .respond_with(ResponseTemplate::new(409).set_body_string("conflict token-value"))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let error = client
        .update_page(UpdatePageRequest {
            page_id: "456".to_string(),
            title: "Updated Page".to_string(),
            next_version: 8,
            storage_html: "<h1>Updated Page</h1>\n".to_string(),
        })
        .await
        .unwrap_err();

    assert_eq!(error.code.as_str(), "confluence_version_conflict");
    assert_eq!(
        error.message,
        "Page was updated by someone else. Fetch the latest version and retry."
    );
    assert!(error.retryable);
    assert_eq!(error.details["response_body"], "conflict [redacted]");
}

#[tokio::test]
async fn error_response_redacts_embedded_basic_credentials() {
    let server = MockServer::start().await;
    let encoded = STANDARD.encode("user@example.com:token-value");
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_string(format!("upstream echoed Authorization: Basic {encoded}")),
        )
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let error = client.list_spaces().await.unwrap_err();
    let details = serde_json::to_string(&error.details).unwrap();

    assert_eq!(error.code.as_str(), "network_error");
    assert!(!details.contains(&encoded));
    assert!(!details.contains("token-value"));
    assert!(!details.contains("user@example.com"));
    assert!(details.contains("[redacted]"));
}
