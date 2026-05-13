use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use serde_json::Value;
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
async fn page_create_without_execute_is_dry_run_and_does_not_post() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("keys", "ENG"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "987", "key": "ENG", "name": "Engineering"}],
            "_links": {}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let dir = TempDir::new().unwrap();
    write_config(dir.path(), &server.uri());
    let body_file = dir.path().join("body.md");
    std::fs::write(&body_file, "# New Page\n\nBody").unwrap();

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    let output = cmd
        .env("CONFLUENCE_CLI_CONFIG", dir.path().join("config.toml"))
        .args([
            "page",
            "create",
            "--space-key",
            "ENG",
            "--title",
            "New Page",
            "--body-file",
        ])
        .arg(&body_file)
        .args(["--parent-id", "123"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(stdout["ok"], true);
    assert_eq!(stdout["command"], "page.create");
    assert_eq!(stdout["dry_run"], true);
    assert_eq!(stdout["data"]["endpoint"], "/api/v2/pages");
}

#[tokio::test]
async fn page_create_execute_posts_payload() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("keys", "ENG"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "987", "key": "ENG", "name": "Engineering"}],
            "_links": {}
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/pages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "456",
            "title": "New Page"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let dir = TempDir::new().unwrap();
    write_config(dir.path(), &server.uri());
    let body_file = dir.path().join("body.md");
    std::fs::write(&body_file, "# New Page\n\nBody").unwrap();

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    let output = cmd
        .env("CONFLUENCE_CLI_CONFIG", dir.path().join("config.toml"))
        .args([
            "page",
            "create",
            "--space-key",
            "ENG",
            "--title",
            "New Page",
            "--body-file",
        ])
        .arg(&body_file)
        .args(["--parent-id", "123", "--execute"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(stdout["ok"], true);
    assert_eq!(stdout["command"], "page.create");
    assert_eq!(stdout["dry_run"], false);
    assert_eq!(stdout["data"]["id"], "456");
}

#[tokio::test]
async fn page_update_without_execute_reads_version_and_does_not_put() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/pages/456"))
        .and(query_param("body-format", "storage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "456",
            "status": "current",
            "title": "Old Page",
            "version": {"number": 7}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let dir = TempDir::new().unwrap();
    write_config(dir.path(), &server.uri());
    let body_file = dir.path().join("body.md");
    std::fs::write(&body_file, "# Updated Page\n\nBody").unwrap();

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    let output = cmd
        .env("CONFLUENCE_CLI_CONFIG", dir.path().join("config.toml"))
        .args([
            "page",
            "update",
            "--page-id",
            "456",
            "--title",
            "Updated Page",
            "--body-file",
        ])
        .arg(&body_file)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(stdout["ok"], true);
    assert_eq!(stdout["command"], "page.update");
    assert_eq!(stdout["dry_run"], true);
    assert_eq!(stdout["data"]["target"]["current_version"], 7);
    assert_eq!(stdout["data"]["target"]["next_version"], 8);
}

#[tokio::test]
async fn page_update_execute_reads_version_then_puts_next_version() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/pages/456"))
        .and(query_param("body-format", "storage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "456",
            "status": "current",
            "title": "Old Page",
            "version": {"number": 7}
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/pages/456"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "456",
            "title": "Updated Page",
            "version": {"number": 8}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let dir = TempDir::new().unwrap();
    write_config(dir.path(), &server.uri());
    let body_file = dir.path().join("body.md");
    std::fs::write(&body_file, "# Updated Page\n\nBody").unwrap();

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    let output = cmd
        .env("CONFLUENCE_CLI_CONFIG", dir.path().join("config.toml"))
        .args([
            "page",
            "update",
            "--page-id",
            "456",
            "--title",
            "Updated Page",
            "--body-file",
        ])
        .arg(&body_file)
        .args(["--execute"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(stdout["ok"], true);
    assert_eq!(stdout["command"], "page.update");
    assert_eq!(stdout["dry_run"], false);
    assert_eq!(stdout["data"]["id"], "456");
}

#[tokio::test]
async fn page_update_conflict_returns_stable_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/pages/456"))
        .and(query_param("body-format", "storage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "456",
            "status": "current",
            "title": "Old Page",
            "version": {"number": 7}
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/pages/456"))
        .respond_with(ResponseTemplate::new(409))
        .expect(1)
        .mount(&server)
        .await;
    let dir = TempDir::new().unwrap();
    write_config(dir.path(), &server.uri());
    let body_file = dir.path().join("body.md");
    std::fs::write(&body_file, "# Updated Page\n\nBody").unwrap();

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", dir.path().join("config.toml"))
        .args([
            "page",
            "update",
            "--page-id",
            "456",
            "--title",
            "Updated Page",
            "--body-file",
        ])
        .arg(&body_file)
        .args(["--execute"])
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            r#""code": "confluence_version_conflict""#,
        ));
}
