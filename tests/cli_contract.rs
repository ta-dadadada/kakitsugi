//! 実バイナリを起動し、CLI、ループバック待受、停止、再起動後の永続性を検証する。

use std::{
    path::Path,
    process::Stdio,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    process::{Child, Command},
};

const BINARY: &str = env!("CARGO_BIN_EXE_kakitsugi");

struct Server {
    child: Child,
    port: u16,
}

impl Server {
    async fn start(database: &Path, working_dir: &Path) -> Self {
        let mut child = Command::new(BINARY)
            .arg("--database")
            .arg(database)
            .args(["serve", "--port", "0"])
            .current_dir(working_dir)
            .env("NO_COLOR", "1")
            .env("RUST_LOG", "kakitsugi=info")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("kakitsugi binary should start");
        let mut lines = BufReader::new(child.stderr.take().unwrap()).lines();
        let port = tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(line) = lines.next_line().await.unwrap() {
                if line.contains("kakitsugi is ready") {
                    return parse_loopback_port(&line);
                }
            }
            panic!("server exited before becoming ready");
        })
        .await
        .expect("server should become ready");
        // パイプが詰まって子プロセスが止まらないよう、残りのログを読み捨てる。
        tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
        Self { child, port }
    }

    fn signal(&self, name: &str) {
        let pid = self.child.id().expect("server should still be running");
        let status = std::process::Command::new("kill")
            .arg(format!("-{name}"))
            .arg(pid.to_string())
            .status()
            .unwrap();
        assert!(status.success());
    }

    async fn stop_with(mut self, name: &str) {
        self.signal(name);
        let status = tokio::time::timeout(Duration::from_secs(5), self.child.wait())
            .await
            .unwrap_or_else(|_| panic!("server should exit within 5s after SIG{name}"))
            .unwrap();
        assert!(status.success(), "server exited with {status}");
    }

    async fn request(&self, method: &str, path: &str, body: Option<Value>) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).await.unwrap();
        let body = body.map(|value| value.to_string()).unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\
             Content-Type: application/json\r\nContent-Length: {length}\r\n\r\n{body}",
            port = self.port,
            length = body.len(),
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (status, body.to_owned())
    }

    async fn json(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let (status, body) = self.request(method, path, body).await;
        (status, serde_json::from_str(&body).unwrap())
    }

    /// SSE 接続を開き、応答ヘッダーを受け取った状態のソケットを返す。
    async fn open_event_stream(&self) -> TcpStream {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).await.unwrap();
        let request = format!(
            "GET /api/v1/events HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAccept: text/event-stream\r\n\r\n",
            self.port
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut head = Vec::new();
        let mut byte = [0_u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).await.unwrap();
            head.push(byte[0]);
        }
        let head = String::from_utf8(head).unwrap();
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        assert!(head.contains("text/event-stream"), "{head}");
        stream
    }
}

fn parse_loopback_port(line: &str) -> u16 {
    let (_, rest) = line
        .split_once("http://127.0.0.1:")
        .unwrap_or_else(|| panic!("ready log should name a 127.0.0.1 URL: {line}"));
    rest.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap()
}

#[test]
fn cli_exposes_no_option_to_change_the_bind_address() {
    for args in [&["--help"][..], &["serve", "--help"][..]] {
        let output = std::process::Command::new(BINARY)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
        let help = String::from_utf8(output.stdout).unwrap().to_lowercase();
        for forbidden in ["--host", "--bind", "--listen", "--address", "--addr"] {
            assert!(!help.contains(forbidden), "{args:?} exposes {forbidden}");
        }
    }
}

#[tokio::test]
async fn binary_serves_from_empty_directory_and_keeps_data_after_restart() {
    let dir = TempDir::new().unwrap();
    let database = dir.path().join("data").join("kakitsugi.sqlite3");

    let server = Server::start(&database, dir.path()).await;
    let (status, body) = server.request("GET", "/healthz", None).await;
    assert_eq!((status, body.as_str()), (200, "ok\n"));
    let (status, page) = server.request("GET", "/", None).await;
    assert_eq!(status, 200);
    assert!(page.contains("Kakitsugi"));

    let (status, created) = server
        .json(
            "POST",
            "/api/v1/threads",
            Some(json!({
                "title": "再起動をまたぐ引き継ぎ",
                "author": "claude",
                "body": "最初の投稿",
                "tags": ["handoff"]
            })),
        )
        .await;
    assert_eq!(status, 201);
    let thread_id = created["thread"]["id"].as_str().unwrap().to_owned();
    let (status, _) = server
        .json(
            "POST",
            &format!("/api/v1/threads/{thread_id}/posts"),
            Some(json!({"author": "codex", "body": "返信"})),
        )
        .await;
    assert_eq!(status, 201);
    let (status, _) = server
        .json(
            "PATCH",
            &format!("/api/v1/threads/{thread_id}"),
            Some(json!({"actor": "codex", "status": "closed"})),
        )
        .await;
    assert_eq!(status, 200);
    server.stop_with("INT").await;

    let server = Server::start(&database, dir.path()).await;
    let (status, thread) = server
        .json("GET", &format!("/api/v1/threads/{thread_id}"), None)
        .await;
    assert_eq!(status, 200);
    assert_eq!(thread["status"], "closed");
    assert_eq!(thread["tags"], json!(["handoff"]));
    let (status, posts) = server
        .json("GET", &format!("/api/v1/threads/{thread_id}/posts"), None)
        .await;
    assert_eq!(status, 200);
    let bodies: Vec<_> = posts["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|post| (post["author"].clone(), post["body"].clone()))
        .collect();
    assert_eq!(
        bodies,
        vec![
            (json!("claude"), json!("最初の投稿")),
            (json!("codex"), json!("返信"))
        ]
    );
    let (status, search) = server
        .json("GET", "/api/v1/search?q=%E8%BF%94%E4%BF%A1", None)
        .await;
    assert_eq!(status, 200);
    assert_eq!(search["items"][0]["id"], json!(thread_id));
    server.stop_with("TERM").await;
}

#[tokio::test]
async fn signals_stop_the_server_while_an_event_stream_is_open() {
    for signal in ["INT", "TERM"] {
        let dir = TempDir::new().unwrap();
        let database = dir.path().join("kakitsugi.sqlite3");
        let server = Server::start(&database, dir.path()).await;
        let _events = server.open_event_stream().await;
        let started = Instant::now();
        server.stop_with(signal).await;
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
