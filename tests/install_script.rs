#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::tempdir;

fn install_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh")
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn path_with_fake_bin(fake_bin: &Path) -> OsString {
    let mut paths = vec![fake_bin.to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(paths).unwrap()
}

fn write_mock_curl(fake_bin: &Path) {
    write_executable(
        &fake_bin.join("curl"),
        r#"#!/bin/sh
set -eu

printf '%s\n' "$*" >> "$MOCK_LOG_DIR/curl.log"

url=
output=
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o)
      output=$2
      shift 2
      ;;
    -*)
      shift
      ;;
    *)
      url=$1
      shift
      ;;
  esac
done

case "$url" in
  "$CONFLUENCE_CLI_GITHUB_API_URL"/repos/laipz8200/confluence-cli/releases/latest)
    printf '{"tag_name":"v1.2.3"}\n'
    ;;
  *)
    if [ -z "$output" ]; then
      printf 'download output path was not provided\n' >&2
      exit 1
    fi
    printf '%s\n' "$url" > "$MOCK_LOG_DIR/download.url"
    printf 'archive' > "$output"
    ;;
esac
"#,
    );
}

fn write_mock_tar_for_target(fake_bin: &Path, target: &str) {
    write_executable(
        &fake_bin.join("tar"),
        &format!(
            r#"#!/bin/sh
set -eu

extract_dir=
while [ "$#" -gt 0 ]; do
  case "$1" in
    -C)
      extract_dir=$2
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [ -z "$extract_dir" ]; then
  printf 'extract directory was not provided\n' >&2
  exit 1
fi

package="$extract_dir/confluence-cli-1.2.3-{target}"
mkdir -p "$package"
cat > "$package/confluence-cli" <<'BIN'
#!/bin/sh
printf '%s\n' "$*" >> "$MOCK_LOG_DIR/confluence-cli.log"
BIN
chmod 755 "$package/confluence-cli"
"#,
            target = target
        ),
    );
}

fn write_mock_tar(fake_bin: &Path) {
    write_mock_tar_for_target(fake_bin, "x86_64-unknown-linux-gnu");
}

fn write_mock_npx(fake_bin: &Path) {
    write_executable(
        &fake_bin.join("npx"),
        r#"#!/bin/sh
set -eu

printf '%s\n' "$*" >> "$MOCK_LOG_DIR/npx.log"
"#,
    );
}

fn run_install_script(temp: &Path, fake_bin: &Path, install_dir: &Path) -> Output {
    Command::new("sh")
        .arg(install_script())
        .env("PATH", path_with_fake_bin(fake_bin))
        .env("HOME", temp)
        .env("MOCK_LOG_DIR", temp)
        .env("CONFLUENCE_CLI_GITHUB_API_URL", "https://api.github.test")
        .env("CONFLUENCE_CLI_GITHUB_BASE_URL", "https://github.example.test")
        .env("CONFLUENCE_CLI_INSTALL_DIR", install_dir)
        .env("CONFLUENCE_CLI_TARGET", "x86_64-unknown-linux-gnu")
        .output()
        .unwrap()
}

fn run_uninstall_script(temp: &Path, fake_bin: &Path, install_dir: &Path) -> Output {
    Command::new("sh")
        .arg(install_script())
        .arg("--uninstall")
        .env("PATH", path_with_fake_bin(fake_bin))
        .env("HOME", temp)
        .env("MOCK_LOG_DIR", temp)
        .env("CONFLUENCE_CLI_INSTALL_DIR", install_dir)
        .output()
        .unwrap()
}

#[test]
fn install_script_downloads_latest_release_asset_for_target() {
    let temp = tempdir().unwrap();
    let fake_bin = temp.path().join("bin");
    let install_dir = temp.path().join("install");
    fs::create_dir(&fake_bin).unwrap();
    write_mock_curl(&fake_bin);
    write_mock_tar(&fake_bin);

    let output = run_install_script(temp.path(), &fake_bin, &install_dir);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("download.url")).unwrap().trim(),
        "https://github.example.test/laipz8200/confluence-cli/releases/download/v1.2.3/confluence-cli-1.2.3-x86_64-unknown-linux-gnu.tar.gz"
    );
    assert!(install_dir.join("confluence-cli").is_file());
    assert_eq!(
        fs::read_to_string(temp.path().join("confluence-cli.log"))
            .unwrap()
            .trim(),
        "config init"
    );
}

#[test]
fn install_script_rejects_unsupported_platforms_before_download() {
    let temp = tempdir().unwrap();
    let fake_bin = temp.path().join("bin");
    let install_dir = temp.path().join("install");
    fs::create_dir(&fake_bin).unwrap();
    write_mock_curl(&fake_bin);
    write_executable(
        &fake_bin.join("uname"),
        r#"#!/bin/sh
case "$1" in
  -s) printf 'FreeBSD\n' ;;
  -m) printf 'x86_64\n' ;;
esac
"#,
    );

    let output = Command::new("sh")
        .arg(install_script())
        .env("PATH", path_with_fake_bin(&fake_bin))
        .env("HOME", temp.path())
        .env("MOCK_LOG_DIR", temp.path())
        .env("CONFLUENCE_CLI_GITHUB_API_URL", "https://api.github.test")
        .env("CONFLUENCE_CLI_GITHUB_BASE_URL", "https://github.example.test")
        .env("CONFLUENCE_CLI_INSTALL_DIR", &install_dir)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unsupported platform"));
    assert!(!temp.path().join("download.url").exists());
}

#[test]
fn install_script_detects_linux_arm64_release_target() {
    let temp = tempdir().unwrap();
    let fake_bin = temp.path().join("bin");
    let install_dir = temp.path().join("install");
    fs::create_dir(&fake_bin).unwrap();
    write_mock_curl(&fake_bin);
    write_mock_tar_for_target(&fake_bin, "aarch64-unknown-linux-gnu");
    write_executable(
        &fake_bin.join("uname"),
        r#"#!/bin/sh
case "$1" in
  -s) printf 'Linux\n' ;;
  -m) printf 'aarch64\n' ;;
esac
"#,
    );

    let output = Command::new("sh")
        .arg(install_script())
        .env("PATH", path_with_fake_bin(&fake_bin))
        .env("HOME", temp.path())
        .env("MOCK_LOG_DIR", temp.path())
        .env("CONFLUENCE_CLI_GITHUB_API_URL", "https://api.github.test")
        .env("CONFLUENCE_CLI_GITHUB_BASE_URL", "https://github.example.test")
        .env("CONFLUENCE_CLI_INSTALL_DIR", &install_dir)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("download.url")).unwrap().trim(),
        "https://github.example.test/laipz8200/confluence-cli/releases/download/v1.2.3/confluence-cli-1.2.3-aarch64-unknown-linux-gnu.tar.gz"
    );
    assert!(install_dir.join("confluence-cli").is_file());
}

#[test]
fn install_script_uninstalls_binary_and_agent_skill() {
    let temp = tempdir().unwrap();
    let fake_bin = temp.path().join("bin");
    let install_dir = temp.path().join("install");
    fs::create_dir(&fake_bin).unwrap();
    fs::create_dir(&install_dir).unwrap();
    fs::write(install_dir.join("confluence-cli"), "installed").unwrap();
    write_mock_curl(&fake_bin);
    write_mock_tar(&fake_bin);
    write_mock_npx(&fake_bin);

    let output = run_uninstall_script(temp.path(), &fake_bin, &install_dir);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!install_dir.join("confluence-cli").exists());
    assert_eq!(
        fs::read_to_string(temp.path().join("npx.log"))
            .unwrap()
            .trim(),
        "skills remove confluence-cli --yes"
    );
    assert!(!temp.path().join("download.url").exists());
}
