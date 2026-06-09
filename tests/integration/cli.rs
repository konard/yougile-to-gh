use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
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

#[test]
fn cli_uses_gh_to_detect_missing_github_credentials() {
    let fixture_path = temp_fixture_path();
    fs::write(
        &fixture_path,
        serde_json::to_string_pretty(&json!({
            "task": {
                "id": "root",
                "title": "Offline root",
                "timestamp": 1_700_000_000_000_u64,
                "subtasks": []
            },
            "messages": [],
            "subtasks": []
        }))
        .unwrap(),
    )
    .unwrap();

    let fake_bin_dir = temp_dir_path("yougile-to-gh-fake-bin");
    fs::create_dir_all(&fake_bin_dir).unwrap();
    write_fake_gh(&fake_bin_dir);

    let (api_url, server) = start_github_api_server();
    let path = prepend_path(&fake_bin_dir);
    let output = Command::new(env!("CARGO_BIN_EXE_yougile-to-gh"))
        .args([
            "--task-json",
            fixture_path.to_str().unwrap(),
            "--mode",
            "single-issue",
            "--github-api-url",
            &api_url,
        ])
        .env("PATH", path)
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITHUB_REPOSITORY")
        .output()
        .expect("failed to execute yougile-to-gh");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let request = server.join().expect("GitHub API server panicked");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("\"github_issue_number\": 7"));
    assert!(!stdout.contains("gh-token-from-cli"));
    assert!(!stderr.contains("gh-token-from-cli"));
    assert!(request.starts_with("POST /repos/owner/from-gh/issues HTTP/1.1"));
    assert!(request
        .to_ascii_lowercase()
        .contains("authorization: bearer gh-token-from-cli"));
    assert!(request.contains("\"title\":\"Offline root\""));

    fs::remove_file(fixture_path).unwrap();
    fs::remove_dir_all(fake_bin_dir).unwrap();
}

#[test]
fn cli_resolves_yougile_token_from_credentials_and_saves_to_lenv() {
    let work_dir = temp_dir_path("yougile-to-gh-auth-work");
    fs::create_dir_all(&work_dir).unwrap();
    let lenv_path = work_dir.join(".lenv");

    let (api_url, server) = start_yougile_api_server();

    let output = Command::new(env!("CARGO_BIN_EXE_yougile-to-gh"))
        .current_dir(&work_dir)
        .args([
            "--yougile-base-url",
            &api_url,
            "--yougile-login",
            "user@example.com",
            "--yougile-password",
            "s3cr3t",
            "--task-id",
            "root",
            "--mode",
            "single-issue",
            "--dry-run",
            "--lenv-path",
            lenv_path.to_str().unwrap(),
        ])
        .env_remove("YOUGILE_TOKEN")
        .env_remove("YOUGILE_LOGIN")
        .env_remove("YOUGILE_PASSWORD")
        .output()
        .expect("failed to execute yougile-to-gh");

    let requests = server.join().expect("YouGile API server panicked");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The credential exchange hit the documented AuthKeyController endpoints.
    assert!(requests
        .iter()
        .any(|request| request.starts_with("POST /api-v2/auth/companies HTTP/1.1")));
    assert!(requests
        .iter()
        .any(|request| request.starts_with("POST /api-v2/auth/keys HTTP/1.1")));
    // The resolved token was used as a bearer credential for task fetching.
    assert!(requests.iter().any(|request| {
        request.starts_with("GET /api-v2/tasks/root HTTP/1.1")
            && request
                .to_ascii_lowercase()
                .contains("authorization: bearer yg-secret-key")
    }));

    // The dry-run plan reflects the task fetched with the resolved token.
    assert!(stdout.contains("\"mode\": \"single-issue\""));
    assert!(stdout.contains("Auth root"));

    // The token is persisted to `.lenv` for reuse, preserving the lino format.
    let saved = fs::read_to_string(&lenv_path).expect("`.lenv` should be written");
    assert!(
        saved.contains("YOUGILE_TOKEN: yg-secret-key"),
        "unexpected .lenv contents:\n{saved}"
    );

    // Secrets never leak to the user-facing streams.
    assert!(!stdout.contains("yg-secret-key"));
    assert!(!stderr.contains("yg-secret-key"));
    assert!(!stdout.contains("s3cr3t"));
    assert!(!stderr.contains("s3cr3t"));

    fs::remove_dir_all(work_dir).unwrap();
}

fn temp_fixture_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("yougile-to-gh-fixture-{nanos}.json"))
}

fn temp_dir_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}"))
}

fn write_fake_gh(bin_dir: &Path) {
    #[cfg(windows)]
    {
        // Command::new("gh") resolves the runner's gh.exe before a fake gh.bat.
        let source_path = bin_dir.join("gh.rs");
        let exe_path = bin_dir.join("gh.exe");
        fs::write(
            &source_path,
            r#"fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("auth")
        && args.get(1).map(String::as_str) == Some("token")
    {
        println!("gh-token-from-cli");
        return;
    }
    if args.first().map(String::as_str) == Some("repo")
        && args.get(1).map(String::as_str) == Some("view")
    {
        println!("owner/from-gh");
        return;
    }
    eprintln!("unexpected gh args: {}", args.join(" "));
    std::process::exit(1);
}
"#,
        )
        .unwrap();
        let output = Command::new("rustc")
            .arg(&source_path)
            .arg("-o")
            .arg(&exe_path)
            .output()
            .expect("failed to compile fake gh executable");
        assert!(
            output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = bin_dir.join("gh");
        fs::write(
            &path,
            r#"#!/bin/sh
if [ "$1 $2" = "auth token" ]; then
  printf 'gh-token-from-cli\n'
  exit 0
fi
if [ "$1 $2" = "repo view" ]; then
  printf 'owner/from-gh\n'
  exit 0
fi
printf 'unexpected gh args: %s\n' "$*" >&2
exit 1
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn prepend_path(bin_dir: &Path) -> std::ffi::OsString {
    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(bin_dir.to_path_buf()).chain(std::env::split_paths(&current_path));
    std::env::join_paths(paths).unwrap()
}

fn start_github_api_server() -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let request = read_http_request(&mut stream);
        let body = concat!(
            "{\"id\":42,\"number\":7,\"html_url\":\"https://github.com/owner/from-gh/issues/7\",",
            "\"url\":\"https://api.github.test/repos/owner/from-gh/issues/7\"}"
        );
        let response = format!(
            "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        request
    });

    (format!("http://{address}"), handle)
}

/// A mock `YouGile` API server that serves the credential-auth handshake
/// followed by a minimal single-task fetch, then returns the captured requests.
///
/// Every response sets `Connection: close` so each `ureq` request arrives on a
/// fresh connection, keeping the accept loop a simple one-request-per-stream.
fn start_yougile_api_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        // companies + keys + task + messages = four sequential requests.
        for _ in 0..4 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let request = read_http_request(&mut stream);
            let body = yougile_mock_response(&request);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
            requests.push(request);
        }
        requests
    });

    (format!("http://{address}"), handle)
}

fn yougile_mock_response(request: &str) -> String {
    let request_line = request.lines().next().unwrap_or_default();
    if request_line.contains("/api-v2/auth/companies") {
        concat!(
            "{\"paging\":{\"count\":1,\"limit\":50,\"offset\":0,\"next\":false},",
            "\"content\":[{\"id\":\"company-1\",\"name\":\"Acme\",\"isAdmin\":true}]}"
        )
        .to_owned()
    } else if request_line.contains("/api-v2/auth/keys") {
        "{\"key\":\"yg-secret-key\"}".to_owned()
    } else if request_line.contains("/api-v2/tasks/") {
        concat!(
            "{\"id\":\"root\",\"title\":\"Auth root\",",
            "\"timestamp\":1700000000000,\"subtasks\":[]}"
        )
        .to_owned()
    } else if request_line.contains("/api-v2/chats/") {
        "{\"paging\":{\"count\":0,\"limit\":1000,\"offset\":0,\"next\":false},\"content\":[]}"
            .to_owned()
    } else {
        panic!("unexpected YouGile request: {request_line}");
    }
}

fn read_http_request(stream: &mut TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 512];

    loop {
        let bytes_read = stream.read(&mut chunk).unwrap();
        assert!(bytes_read > 0, "connection closed before request completed");
        buffer.extend_from_slice(&chunk[..bytes_read]);

        if let Some(header_end) = find_header_end(&buffer) {
            let headers = String::from_utf8_lossy(&buffer[..header_end]);
            let content_length = parse_content_length(&headers);
            let body_start = header_end + 4;
            if buffer.len() >= body_start + content_length {
                return String::from_utf8_lossy(&buffer).to_string();
            }
        }
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap_or(0)
}
