use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn write_config(dir: &Path, site_url: &str) {
    std::fs::write(
        dir.join("config.toml"),
        format!(
            r#"site_url = "{site_url}"
email = "user@example.com"
api_token = "token-value"
default_space = "ENG"
"#
        ),
    )
    .unwrap();
}

#[tokio::test]
async fn space_list_prints_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "987", "key": "ENG", "name": "Engineering"}],
            "_links": {}
        })))
        .mount(&server)
        .await;
    let dir = TempDir::new().unwrap();
    write_config(dir.path(), &server.uri());

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", dir.path().join("config.toml"))
        .args(["space", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""ok": true"#))
        .stdout(predicate::str::contains(r#""command": "space.list""#))
        .stdout(predicate::str::contains(r#""key": "ENG""#));
}

#[tokio::test]
async fn search_query_builds_cql() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/search"))
        .and(query_param("cql", r#"text ~ "deploy""#))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"content": {"id": "123", "title": "Deploy Guide"}}]
        })))
        .mount(&server)
        .await;
    let dir = TempDir::new().unwrap();
    write_config(dir.path(), &server.uri());

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", dir.path().join("config.toml"))
        .args(["search", "--query", "deploy"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""command": "search""#))
        .stdout(predicate::str::contains("Deploy Guide"));
}

#[tokio::test]
async fn page_get_requests_storage_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/pages/123"))
        .and(query_param("body-format", "storage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "123",
            "status": "current",
            "title": "Deploy Guide",
            "spaceId": "987",
            "parentId": null,
            "version": {"number": 7},
            "body": {"storage": {"value": "<p>Deploy</p>", "representation": "storage"}},
            "_links": {}
        })))
        .mount(&server)
        .await;
    let dir = TempDir::new().unwrap();
    write_config(dir.path(), &server.uri());

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", dir.path().join("config.toml"))
        .args(["page", "get", "--page-id", "123"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""command": "page.get""#))
        .stdout(predicate::str::contains(r#""title": "Deploy Guide""#));
}
