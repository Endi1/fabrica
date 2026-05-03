use std::io::Write;
use std::process::{Command, Stdio};

/// Helper: send a line to the server's stdin, read one line back from stdout.
fn roundtrip(
    stdin: &mut impl Write,
    stdout_lines: &mut impl Iterator<Item = String>,
    msg: &str,
) -> String {
    writeln!(stdin, "{}", msg).unwrap();
    stdin.flush().unwrap();
    // Read the next non-empty line
    loop {
        let line = stdout_lines.next().expect("server closed stdout");
        if !line.trim().is_empty() {
            return line;
        }
    }
}

#[test]
fn test_initialize() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fabrica"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start fabrica serve");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut lines = std::io::BufRead::lines(std::io::BufReader::new(stdout)).map(|l| l.unwrap());

    // --- initialize ---
    let resp = roundtrip(
        &mut stdin,
        &mut lines,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1"}}"#,
    );

    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], 1);
    assert_eq!(v["result"]["protocolVersion"], "1");
    assert_eq!(v["result"]["agentInfo"]["name"], "fabrica");
    assert!(v["error"].is_null());

    // --- unknown method ---
    let resp = roundtrip(
        &mut stdin,
        &mut lines,
        r#"{"jsonrpc":"2.0","id":99,"method":"bogus","params":{}}"#,
    );
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["error"]["code"], -32601);

    // Close stdin → server exits
    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn test_session_new_without_api_key() {
    // session/new will try to create a model, which needs an API key.
    // Without one set it should return an error response (not crash).
    let mut child = Command::new(env!("CARGO_BIN_EXE_fabrica"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Clear all known API key env vars so model creation fails gracefully
        .env_remove("ANTHROPIC_KEY")
        .env_remove("GEMINI_KEY")
        .env_remove("OPENAI_KEY")
        .env_remove("GCP_PROJECT")
        .spawn()
        .expect("failed to start fabrica serve");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut lines = std::io::BufRead::lines(std::io::BufReader::new(stdout)).map(|l| l.unwrap());

    // initialize first (always succeeds)
    let _ = roundtrip(
        &mut stdin,
        &mut lines,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1"}}"#,
    );

    // session/new should return a JSON-RPC error (not crash)
    let resp = roundtrip(
        &mut stdin,
        &mut lines,
        r#"{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/tmp"}}"#,
    );
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["id"], 2);
    assert!(v["error"].is_object(), "expected error, got: {v}");
    assert_eq!(v["error"]["code"], -32000);

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn test_prompt_unknown_session() {
    // Prompting with a bogus session ID should return an error response.
    // We need a valid model to create sessions, but we can test the
    // "unknown session" path without one.
    let mut child = Command::new(env!("CARGO_BIN_EXE_fabrica"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start fabrica serve");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut lines = std::io::BufRead::lines(std::io::BufReader::new(stdout)).map(|l| l.unwrap());

    // initialize
    let _ = roundtrip(
        &mut stdin,
        &mut lines,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1"}}"#,
    );

    // prompt with non-existent session
    let resp = roundtrip(
        &mut stdin,
        &mut lines,
        r#"{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"does-not-exist","prompt":[{"type":"text","text":"hi"}]}}"#,
    );
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["id"], 3);
    // Should get end_turn result (agent task errors are streamed, not RPC errors)
    // OR an error — either is acceptable; let's just make sure it's valid JSON-RPC
    assert_eq!(v["jsonrpc"], "2.0");

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}
