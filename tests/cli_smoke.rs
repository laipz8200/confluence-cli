use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_flag_prints_package_version() {
    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();

    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_mentions_agent_first_commands() {
    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();

    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("space"))
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("page"));
}
