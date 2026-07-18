#![cfg(unix)]

use serde_json::Value;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const EXIT_VALIDATION_FAILED: i32 = 5;
const FILESYSTEM_UNSAFE: &str = "TOOL_DEDUPE_FILESYSTEM_UNSAFE";

struct TestTree {
    root: PathBuf,
}

impl TestTree {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "aopmem-dedupe-RAW_PATH_CANARY-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("isolated test root should create");
        Self { root }
    }
}

impl Drop for TestTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn run_cli(
    binary: &Path,
    repository: &Path,
    aopmem_home: &Path,
    user_home: &Path,
    args: &[&str],
    input: Option<&str>,
) -> Output {
    let mut command = Command::new(binary);
    command
        .args(args)
        .current_dir(repository)
        .env("AOPMEM_HOME", aopmem_home)
        .env("HOME", user_home)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("compiled aopmem should start");
    if let Some(input) = input {
        child
            .stdin
            .take()
            .expect("aopmem stdin should be piped")
            .write_all(input.as_bytes())
            .expect("init answers should write");
    }
    child.wait_with_output().expect("aopmem should finish")
}

fn json_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout should be one JSON envelope: {error}; stdout={:?}; stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &Output, command: &str) -> Value {
    assert!(
        output.status.success(),
        "{command} failed: stdout={:?}; stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope = json_stdout(output);
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["command"], command);
    envelope
}

fn write_runner(path: &Path) {
    fs::write(path, b"#!/bin/sh\nexit 0\n").expect("runner should write");
    let mut permissions = fs::metadata(path)
        .expect("runner metadata should read")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("runner should become executable");
}

fn assert_private_failure(output: &Output, expected_command: &str, json: bool, forbidden: &[&str]) {
    assert_eq!(
        output.status.code(),
        Some(EXIT_VALIDATION_FAILED),
        "unexpected exit: stdout={:?}; stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8");
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8");
    let captured = format!("{stdout}\n{stderr}");
    assert!(captured.contains(FILESYSTEM_UNSAFE));
    for value in forbidden {
        assert!(
            !captured.contains(value),
            "CLI output exposed private filesystem canary {value:?}: {captured:?}"
        );
    }

    if json {
        assert!(stderr.is_empty(), "JSON mode stderr should stay empty");
        let envelope: Value =
            serde_json::from_str(stdout.trim()).expect("error envelope should parse");
        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["command"], expected_command);
        assert!(envelope["data"].is_null());
        assert_eq!(
            envelope["errors"].as_array().map(Vec::len),
            Some(1),
            "one stable error is required"
        );
        assert_eq!(envelope["errors"][0]["code"], "TOOL_DEDUPE_PLAN_FAILED");
        assert_eq!(envelope["errors"][0]["message"], FILESYSTEM_UNSAFE);
        assert!(envelope["warnings"].as_array().is_some_and(Vec::is_empty));
        assert!(envelope["meta"]["version"].is_string());
    } else {
        assert!(stdout.is_empty(), "text error should use stderr only");
        assert_eq!(
            stderr.trim(),
            format!("TOOL_DEDUPE_PLAN_FAILED: {FILESYSTEM_UNSAFE}")
        );
    }
}

#[test]
fn stage_012_014_cli_dedupe_filesystem_errors_are_private_in_text_and_json() {
    let binary = Path::new(env!("CARGO_BIN_EXE_aopmem"));
    let tree = TestTree::new();
    let repository = tree.root.join("repository");
    let aopmem_home = tree.root.join("aopmem-home");
    let user_home = tree.root.join("user-home");
    fs::create_dir(&repository).expect("repository should create");
    fs::create_dir(&aopmem_home).expect("AOPMEM_HOME should create");
    fs::create_dir(&user_home).expect("HOME should create");

    let init = run_cli(
        binary,
        &repository,
        &aopmem_home,
        &user_home,
        &["--json", "init"],
        Some("no\nno\nDedupe privacy proof\nTest CLI output\nLocal files only\n"),
    );
    let init = assert_success(&init, "init");
    let workspace_key = init["meta"]["workspace_key"]
        .as_str()
        .expect("init should return a workspace key");
    let workspace = aopmem_home.join("workspaces").join(workspace_key);

    let first = run_cli(
        binary,
        &repository,
        &aopmem_home,
        &user_home,
        &[
            "--json",
            "tool",
            "create-draft",
            "--id",
            "reader",
            "--name",
            "Reader",
        ],
        None,
    );
    assert_success(&first, "tool_create_draft");
    write_runner(&workspace.join("tools/reader/bin/reader"));

    let second = run_cli(
        binary,
        &repository,
        &aopmem_home,
        &user_home,
        &[
            "--json",
            "tool",
            "create-draft",
            "--id",
            "reader-user",
            "--name",
            "Reader user",
            "--technical-distinction",
            "separate wrapper",
        ],
        None,
    );
    assert_success(&second, "tool_create_draft");
    write_runner(&workspace.join("tools/reader-user/bin/reader-user"));

    let canary = format!(
        "RAW_PATH_CANARY_{}",
        tree.root
            .file_name()
            .and_then(|name| name.to_str())
            .expect("test root should be UTF-8")
    );
    let absolute_target = tree.root.join(format!("{canary}.txt"));
    fs::write(&absolute_target, b"private").expect("absolute canary target should write");
    symlink(
        &absolute_target,
        workspace
            .join("tools/reader-user")
            .join(format!("unsafe-child-{canary}")),
    )
    .expect("unsafe absolute symlink should create");
    let root_text = tree.root.to_string_lossy().into_owned();
    let target_text = absolute_target.to_string_lossy().into_owned();
    let forbidden = [canary.as_str(), root_text.as_str(), target_text.as_str()];

    for (args, expected_command, json) in [
        (
            ["tool", "dedupe", "plan", ""].as_slice(),
            "tool_dedupe_plan",
            false,
        ),
        (
            ["tool", "dedupe", "plan", "--json"].as_slice(),
            "tool_dedupe_plan",
            true,
        ),
        (
            ["tool", "dedupe", "apply", "--exact-only"].as_slice(),
            "tool_dedupe_apply",
            false,
        ),
        (
            ["tool", "dedupe", "apply", "--exact-only", "--json"].as_slice(),
            "tool_dedupe_apply",
            true,
        ),
    ] {
        let args = args
            .iter()
            .copied()
            .filter(|argument| !argument.is_empty())
            .collect::<Vec<_>>();
        let output = run_cli(binary, &repository, &aopmem_home, &user_home, &args, None);
        assert_private_failure(&output, expected_command, json, &forbidden);
    }
}
