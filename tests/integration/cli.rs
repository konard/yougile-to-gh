use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

#[test]
fn cli_dry_run_reads_task_json_without_network_credentials() {
    let fixture_path = temp_fixture_path();
    fs::write(
        &fixture_path,
        serde_json::to_string_pretty(&json!({
            "task": {
                "id": "root",
                "title": "Offline root",
                "timestamp": 1_700_000_000_000_u64,
                "subtasks": ["child"]
            },
            "messages": [],
            "subtasks": [
                {
                    "task": {
                        "id": "child",
                        "title": "Offline child",
                        "timestamp": 1_700_000_001_000_u64,
                        "subtasks": []
                    },
                    "messages": [],
                    "subtasks": []
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_yougile-to-gh"))
        .args([
            "--task-json",
            fixture_path.to_str().unwrap(),
            "--dry-run",
            "--mode",
            "single-issue",
        ])
        .output()
        .expect("failed to execute yougile-to-gh");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"mode\": \"single-issue\""));
    assert!(stdout.contains("Offline child"));

    fs::remove_file(fixture_path).unwrap();
}

fn temp_fixture_path() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("yougile-to-gh-fixture-{nanos}.json"))
}
