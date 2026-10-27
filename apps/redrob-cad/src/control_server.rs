//! Loopback JSON-lines control server: one request per line, one reply per line.
//! This is the transport the MCP server (`redrob-cad-cli mcp --connect`) wraps.
//!
//! With a token, every request must carry `"token": "<token>"` or it is refused. The built-in
//! agent's server always has one: it listens on an ephemeral port that any local process could
//! find, and the token is handed only to the MCP server the app itself starts. `--control PORT`
//! keeps the tokenless protocol for scripts.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use redrob_cad_ui_egui::ControlRequest;
use serde_json::{Value, json};

/// Start the server on `127.0.0.1:port` (`0` picks a free port). Returns the request stream and
/// the port actually bound (`None` when binding failed; the app then runs without it).
pub fn start(port: u16, ctx: egui::Context, token: Option<String>) -> (Receiver<ControlRequest>, Option<u16>) {
    let (tx, rx) = channel::<ControlRequest>();
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("redrob-cad: control server failed to bind 127.0.0.1:{port}: {e}");
            return (rx, None);
        }
    };
    let bound = listener.local_addr().map(|a| a.port()).ok();
    if token.is_none() {
        eprintln!("redrob-cad: control server listening on 127.0.0.1:{}", bound.unwrap_or(port));
    }
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let tx = tx.clone();
            let ctx = ctx.clone();
            let token = token.clone();
            std::thread::spawn(move || serve(stream, tx, ctx, token));
        }
    });
    (rx, bound)
}

/// Whether a request may run: always without a token, else only with the same token.
pub fn authorized(msg: &Value, token: Option<&str>) -> bool {
    match token {
        None => true,
        Some(t) => msg.get("token").and_then(Value::as_str).is_some_and(|got| constant_time_eq(got.as_bytes(), t.as_bytes())),
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn serve(stream: TcpStream, tx: Sender<ControlRequest>, ctx: egui::Context, token: Option<String>) {
    let Ok(read) = stream.try_clone() else { return };
    let mut out = stream;
    for line in BufReader::new(read).lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) if !authorized(&msg, token.as_deref()) => {
                let id = msg.get("id").cloned().unwrap_or(Value::Null);
                json!({"id": id, "ok": false, "error": "unauthorized"})
            }
            Ok(msg) => {
                let id = msg.get("id").cloned().unwrap_or(Value::Null);
                let method = msg.get("method").and_then(Value::as_str).unwrap_or("").to_string();
                let params = msg.get("params").cloned().unwrap_or(json!({}));
                let (req, rrx) = ControlRequest::new(method, params);
                if tx.send(req).is_err() {
                    break;
                }
                ctx.request_repaint();
                let mut r = rrx.recv_timeout(Duration::from_secs(60)).unwrap_or_else(|_| json!({"ok": false, "error": "timeout"}));
                if let Some(o) = r.as_object_mut() {
                    o.insert("id".into(), id);
                }
                r
            }
            Err(e) => json!({"ok": false, "error": format!("bad JSON: {e}")}),
        };
        if writeln!(out, "{reply}").is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_gates_requests_only_when_set() {
        let ok = json!({"id": 1, "method": "drawing.inspect", "token": "abc"});
        let none = json!({"id": 1, "method": "drawing.inspect"});
        let wrong = json!({"id": 1, "method": "drawing.inspect", "token": "abd"});
        assert!(authorized(&none, None));
        assert!(authorized(&ok, Some("abc")));
        assert!(!authorized(&none, Some("abc")));
        assert!(!authorized(&wrong, Some("abc")));
        assert!(!authorized(&json!({"token": "ab"}), Some("abc")));
    }
}
