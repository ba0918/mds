use assert_cmd::Command;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

const SCHEMA_BODY: &str = "document:\n  title:\n    pattern: \"^T-\\\\d{4}:\"\n";

fn write_file(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path
}

/// 固定のスキーマを返すローカル HTTP サーバを立て、URL と接続数を返す。
fn serve_schema() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let counter = Arc::new(AtomicUsize::new(0));
    let server_counter = Arc::clone(&counter);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            server_counter.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                SCHEMA_BODY.len(),
                SCHEMA_BODY
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    (format!("http://127.0.0.1:{port}/schema.yaml"), counter)
}

fn mds() -> Command {
    Command::cargo_bin("mds").unwrap()
}

#[test]
fn url_schema_is_fetched_once_and_cached() {
    let (url, counter) = serve_schema();
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".mds")).unwrap();
    let doc = write_file(
        dir.path(),
        "doc.md",
        &format!("---\n$schema: {url}\n---\n# T-1234: 例\n"),
    );

    let first = mds()
        .current_dir(dir.path())
        .args(["check", doc.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(first.status.code(), Some(0), "1回目はサーバから取得して成功");

    let cache_dir = dir.path().join(".mds").join("cache");
    let entries: Vec<_> = std::fs::read_dir(&cache_dir)
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert_eq!(entries.len(), 1, "キャッシュに1件保存される");
    let cached = std::fs::read_to_string(entries[0].path()).unwrap();
    assert_eq!(cached, SCHEMA_BODY);

    let second = mds()
        .current_dir(dir.path())
        .args(["check", doc.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(second.status.code(), Some(0));
    assert_eq!(counter.load(Ordering::SeqCst), 1, "2回目はサーバに到達しない");
}

#[test]
fn url_schema_resolves_the_same_for_all_commands() {
    let (url, counter) = serve_schema();
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".mds")).unwrap();
    let doc = write_file(
        dir.path(),
        "doc.md",
        &format!("---\n$schema: {url}\n---\n# T-1234: 例\n"),
    );

    for args in [
        vec!["check", doc.to_str().unwrap()],
        vec!["values", doc.to_str().unwrap()],
        vec!["ast", doc.to_str().unwrap(), "--schema"],
    ] {
        let output = mds().current_dir(dir.path()).args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "args {args:?}");
    }
    assert_eq!(counter.load(Ordering::SeqCst), 1, "3コマンドとも同じキャッシュを使う");
}

#[test]
fn url_schema_without_cache_and_unreachable_server_stops() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".mds")).unwrap();
    // ポートを取得して閉じ、接続できない URL を作る
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let doc = write_file(
        dir.path(),
        "doc.md",
        &format!("---\n$schema: http://127.0.0.1:{port}/schema.yaml\n---\n# T-1234: 例\n"),
    );
    let output = mds()
        .current_dir(dir.path())
        .args(["check", doc.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("schema_not_found"));
}

#[test]
fn url_schema_server_that_never_responds_stops_after_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            // リクエストは読むが、応答を返さずにブロックする
            let _ = stream.read(&mut [0u8; 1024]);
            thread::sleep(std::time::Duration::from_secs(60));
        }
    });
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".mds")).unwrap();
    let doc = write_file(
        dir.path(),
        "doc.md",
        &format!("---\n$schema: http://127.0.0.1:{port}/schema.yaml\n---\n# T-1234: 例\n"),
    );
    let output = mds()
        .env("MDS_SCHEMA_TIMEOUT_MS", "300")
        .timeout(std::time::Duration::from_secs(5))
        .current_dir(dir.path())
        .args(["check", doc.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("schema_not_found"));
}