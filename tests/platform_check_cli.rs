use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn unique_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "aopmem-platform-check-cli-{label}-{}",
        uuid::Uuid::new_v4().simple()
    ))
}

#[test]
fn platform_check_json_is_workspace_independent_private_and_repeatable() {
    let aopmem_home = unique_path("AOPMEM_HOME_SECRET_CANARY");
    let poisoned_tmpdir = unique_path("TMPDIR_SECRET_CANARY");
    let fallback_home = unique_path("HOME_SECRET_CANARY");
    let repository = unique_path("repo");
    fs::write(&aopmem_home, b"AOPMEM_HOME_POISON_CANARY").expect("poison AOPMEM_HOME");
    fs::write(&poisoned_tmpdir, b"TMPDIR_POISON_CANARY").expect("poison TMPDIR");
    let poison_before = fs::metadata(&aopmem_home).expect("poison metadata");
    let tmpdir_before = fs::metadata(&poisoned_tmpdir).expect("TMPDIR metadata");
    fs::create_dir(&repository).expect("repository fixture");

    for arguments in [
        ["platform", "check", "--json"],
        ["--json", "platform", "check"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_aopmem"))
            .args(arguments)
            .current_dir(&repository)
            .env("AOPMEM_HOME", &aopmem_home)
            .env("HOME", &fallback_home)
            .env("TMPDIR", &poisoned_tmpdir)
            .output()
            .expect("platform check process");
        assert!(
            output.status.success(),
            "stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 JSON");
        let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
        assert_eq!(value["ok"], true);
        assert_eq!(value["command"], "platform_check");
        assert_eq!(value["data"]["schema_version"], 1);
        assert_eq!(value["data"]["status"], "pass");
        assert_eq!(value["data"]["location"], "private_os_temp");
        assert_eq!(value["data"]["observability_recorded"], false);
        assert_eq!(value["data"]["admin_required"], false);
        assert_eq!(value["data"]["cleanup"]["root_removed"], true);
        assert_eq!(
            value["data"]["checks"]
                .as_array()
                .expect("checks array")
                .len(),
            10
        );
        assert!(!stdout.contains("SECRET_CANARY"));
        assert!(!stdout.contains(&std::env::temp_dir().display().to_string()));
        assert_eq!(
            fs::read(&aopmem_home).expect("poison AOPMEM_HOME unchanged"),
            b"AOPMEM_HOME_POISON_CANARY"
        );
        let poison_after = fs::metadata(&aopmem_home).expect("poison metadata unchanged");
        assert_eq!(poison_after.len(), poison_before.len());
        assert_eq!(
            poison_after.modified().expect("modified time"),
            poison_before.modified().expect("original modified time")
        );
        assert_eq!(
            fs::read(&poisoned_tmpdir).expect("poison TMPDIR unchanged"),
            b"TMPDIR_POISON_CANARY"
        );
        let tmpdir_after = fs::metadata(&poisoned_tmpdir).expect("TMPDIR metadata unchanged");
        assert_eq!(tmpdir_after.len(), tmpdir_before.len());
        assert_eq!(
            tmpdir_after.modified().expect("TMPDIR modified time"),
            tmpdir_before
                .modified()
                .expect("original TMPDIR modified time")
        );
        assert!(!fallback_home.exists());
        assert_eq!(
            fs::read_dir(&repository)
                .expect("repository readable")
                .count(),
            0,
            "platform check must not create workspace files"
        );
    }

    fs::remove_file(aopmem_home).expect("poison cleanup");
    fs::remove_file(poisoned_tmpdir).expect("TMPDIR cleanup");
    fs::remove_dir(repository).expect("repository cleanup");
}
