use crate::catalog;
use crate::protocol::{Cmd, HostEvent, Node, PROTOCOL_VERSION};
use anyhow::{Context, Result, bail};
use gpui_kit::component as gpui_component;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub struct ClojureHost {
    pub nrepl_port: u16,
    #[allow(dead_code)]
    pub app: String,
    pub cmd_tx: mpsc::Sender<Cmd>,
    pub event_rx: async_channel::Receiver<HostEvent>,
}

impl Drop for ClojureHost {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(Cmd::Shutdown);
    }
}

type Pending = Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>>;

fn write_json(stream: &Mutex<TcpStream>, value: &Value) -> Result<()> {
    let mut stream = stream.lock().unwrap();
    writeln!(stream, "{value}")?;
    stream.flush()?;
    Ok(())
}

fn rpc(
    stream: &Mutex<TcpStream>,
    pending: &Pending,
    next_id: &AtomicU64,
    mut request: Value,
) -> Result<Value> {
    let id = next_id.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = mpsc::channel();
    pending.lock().unwrap().insert(id, tx);
    request["id"] = json!(id);
    let result = write_json(stream, &request).and_then(|_| {
        rx.recv_timeout(Duration::from_secs(30))
            .context("timed out waiting for Clojure to answer")
    });
    pending.lock().unwrap().remove(&id);
    result
}

fn parse_tree(value: &Value) -> Result<(Node, Vec<gpui_component::theme::ThemeSet>)> {
    if !value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        let err = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Clojure render failed");
        bail!("{err}");
    }
    let tree = value
        .get("tree")
        .context("Clojure response missing :tree")?;
    // Include the field path and cause in Display, since every bridge caller
    // sends errors to the native UI with `to_string()` (not the anyhow chain).
    let node = serde_path_to_error::deserialize(tree).map_err(|err| {
        anyhow::anyhow!(
            "invalid UI tree from Clojure at {}: {}",
            err.path(),
            err.inner()
        )
    })?;
    let themes = catalog::theme_sets_from_value(value.get("themes"));
    Ok((node, themes))
}

fn send_event(event_tx: &async_channel::Sender<HostEvent>, result: Result<HostEvent>) {
    match result {
        Ok(event) => {
            let _ = event_tx.send_blocking(event);
        }
        Err(err) => {
            let _ = event_tx.send_blocking(HostEvent::Error(err.to_string()));
        }
    }
}

fn callback_failed(value: &Value) -> Option<String> {
    if value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        None
    } else {
        Some(
            value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("Clojure callback failed")
                .to_string(),
        )
    }
}

/// Invoke every callback in one native action against the same registry
/// generation, then fetch exactly one tree. Stop remaining callbacks on
/// the first failure; still render so earlier atom mutations paint.
fn apply_callback_batch(
    writer: &Mutex<TcpStream>,
    pending: &Pending,
    next_id: &AtomicU64,
    event_tx: &async_channel::Sender<HostEvent>,
    calls: Vec<crate::protocol::CallbackCall>,
    seq: Option<u64>,
) {
    let flags = crate::protocol::defer_render_flags(calls.len());
    let mut failed = None;
    for (call, defer) in calls.into_iter().zip(flags) {
        let request = if defer {
            crate::protocol::callback_rpc(call.id, call.value, true)
        } else {
            crate::protocol::callback_request(call.id, call.value)
        };
        match rpc(writer, pending, next_id, request) {
            Ok(resp) => {
                if let Some(err) = callback_failed(&resp) {
                    failed = Some(err);
                    break;
                }
            }
            Err(err) => {
                let _ = event_tx.send_blocking(HostEvent::Error(err.to_string()));
                return;
            }
        }
    }
    // Always fetch a tree here: input submit attaches `seq` to this
    // response, and handlers that do not touch an r/atom still need a paint.
    // Multi-callback RPCs used defer-render so this is the only export-tree
    // for the native action. Stop remaining callbacks on first failure;
    // still render so earlier atom mutations paint, then surface the error.
    match rpc(writer, pending, next_id, json!({"op": "render"}))
        .and_then(|value| parse_tree(&value))
    {
        Ok((node, themes)) => {
            let _ = event_tx.send_blocking(HostEvent::tree(node, seq, themes));
        }
        Err(err) => {
            let _ = event_tx.send_blocking(HostEvent::Error(err.to_string()));
        }
    }
    if let Some(err) = failed {
        let _ = event_tx.send_blocking(HostEvent::Error(err));
    }
}

fn connect_to_clojure() -> Result<TcpStream> {
    let host = std::env::var("CLJ_GPUI_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = std::env::var("CLJ_GPUI_PORT")
        .context("CLJ_GPUI_PORT is not set. Start the app with `clj -M:dev my.app/app`.")?;
    let addr = format!("{host}:{port}");
    println!("[host] connecting to Clojure at {addr}");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match TcpStream::connect(&addr) {
            Ok(stream) => {
                stream.set_nodelay(true)?;
                println!("[host] connected");
                return Ok(stream);
            }
            Err(err) => {
                if Instant::now() >= deadline {
                    return Err(err).context(format!("could not connect to Clojure at {addr}"));
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn attach(stream: TcpStream) -> Result<ClojureHost> {
    let reader_stream = stream.try_clone()?;
    let writer = Arc::new(Mutex::new(stream.try_clone()?));
    let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
    let next_id = Arc::new(AtomicU64::new(1));
    let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
    let (event_tx, event_rx) = async_channel::unbounded::<HostEvent>();
    let (ready_tx, ready_rx) = mpsc::channel::<(u16, String)>();
    thread::Builder::new()
        .name("clj-gpui-reader".into())
        .spawn({
            let pending = pending.clone();
            let event_tx = event_tx.clone();
            move || {
                let mut lines = BufReader::new(reader_stream).lines();
                while let Some(Ok(line)) = lines.next() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let value: Value = match serde_json::from_str(&line) {
                        Ok(v) => v,
                        Err(err) => {
                            eprintln!("[host] invalid JSON from Clojure: {err}: {line}");
                            continue;
                        }
                    };
                    let op = value.get("op").and_then(Value::as_str).unwrap_or("");
                    match op {
                        "ready" => {
                            let version = value
                                .get("protocol-version")
                                .and_then(Value::as_u64)
                                .unwrap_or(0);
                            if version != PROTOCOL_VERSION {
                                let msg = format!(
                                    "protocol version mismatch: Clojure={version} host={PROTOCOL_VERSION}"
                                );
                                eprintln!("[host] {msg}");
                                let _ = event_tx.send_blocking(HostEvent::Error(msg));
                                continue;
                            }
                            let nrepl = value.get("nrepl").and_then(Value::as_u64).unwrap_or(0) as u16;
                            let app = value
                                .get("app")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown")
                                .to_string();
                            let _ = ready_tx.send((nrepl, app.clone()));
                            let _ = event_tx.send_blocking(HostEvent::Ready {
                                nrepl_port: nrepl,
                                app,
                            });
                        }
                        "request-render" => {
                            let _ = event_tx.send_blocking(HostEvent::RenderRequested);
                        }
                        "pick-directory" => {
                            let request_id = value
                                .get("request-id")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            let title = value
                                .get("title")
                                .and_then(Value::as_str)
                                .map(str::to_string);
                            let _ = event_tx.send_blocking(HostEvent::PickDirectory {
                                request_id,
                                title,
                            });
                        }
                        "reveal-path" => {
                            if let Some(path) = value.get("path").and_then(Value::as_str) {
                                let _ = event_tx.send_blocking(HostEvent::RevealPath {
                                    path: path.to_string(),
                                });
                            }
                        }
                        "open-path" => {
                            if let Some(path) = value.get("path").and_then(Value::as_str) {
                                let _ = event_tx.send_blocking(HostEvent::OpenPath {
                                    path: path.to_string(),
                                });
                            }
                        }
                        "capture-preview" => {
                            let request_id = value
                                .get("request-id")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            if !request_id.is_empty() {
                                let _ = event_tx.send_blocking(HostEvent::CapturePreview {
                                    request_id,
                                });
                            }
                        }
                        "response" | "" => {
                            if let Some(id) = value.get("id").and_then(Value::as_u64)
                                && let Some(tx) = pending.lock().unwrap().remove(&id) {
                                    let _ = tx.send(value);
                                }
                        }
                        other => {
                            eprintln!("[host] ignored Clojure message op={other}");
                        }
                    }
                }
                println!("[host] Clojure socket closed");
            }
        })?;

    let (nrepl_port, app) = ready_rx
        .recv_timeout(Duration::from_secs(30))
        .context("timed out waiting for Clojure :ready")?;
    println!(
        "[host] Clojure ready app={app} protocol={PROTOCOL_VERSION} nREPL=127.0.0.1:{nrepl_port}"
    );

    thread::Builder::new()
        .name("clj-gpui-worker".into())
        .spawn({
            let writer = writer.clone();
            let pending = pending.clone();
            let next_id = next_id.clone();
            let event_tx = event_tx.clone();
            move || {
                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        Cmd::Shutdown => break,
                        Cmd::Provider {
                            id,
                            params,
                            response,
                        } => {
                            if response.is_closed() {
                                continue;
                            }
                            let writer = writer.clone();
                            let pending = pending.clone();
                            let next_id = next_id.clone();
                            // Provider requests must never block editing or ordinary callbacks.
                            thread::spawn(move || {
                                let result = rpc(
                                    &writer,
                                    &pending,
                                    &next_id,
                                    json!({"op":"provider", "provider-id":id, "params":params}),
                                )
                                .and_then(|reply| match callback_failed(&reply) {
                                    Some(error) => Err(anyhow::anyhow!(error)),
                                    None => Ok(reply.get("value").cloned().unwrap_or(Value::Null)),
                                })
                                .map_err(|error| error.to_string());
                                let _ = response.send_blocking(result);
                            });
                        }
                        Cmd::DirectoryPicked {
                            request_id,
                            path,
                            error,
                            cancelled,
                        } => {
                            let mut request = json!({
                                "op": "directory-picked",
                                "request-id": request_id,
                                "cancelled": cancelled,
                            });
                            if let Some(path) = path {
                                request["path"] = json!(path);
                            }
                            if let Some(error) = error {
                                request["error"] = json!(error);
                            }
                            if let Err(err) = rpc(&writer, &pending, &next_id, request) {
                                let _ = event_tx.send_blocking(HostEvent::Error(err.to_string()));
                            }
                        }
                        Cmd::PreviewCaptured { request_id, png } => {
                            let mut request = json!({
                                "op": "preview-captured",
                                "request-id": request_id,
                            });
                            if let Some(png) = png {
                                request["png"] = json!(png);
                            }
                            if let Err(err) = rpc(&writer, &pending, &next_id, request) {
                                let _ = event_tx.send_blocking(HostEvent::Error(err.to_string()));
                            }
                        }
                        other => match other {
                            Cmd::Shutdown
                            | Cmd::Provider { .. }
                            | Cmd::DirectoryPicked { .. }
                            | Cmd::PreviewCaptured { .. } => {
                                unreachable!()
                            }
                            Cmd::Render => {
                                let result =
                                    rpc(&writer, &pending, &next_id, json!({"op": "render"}))
                                        .and_then(|value| parse_tree(&value))
                                        .map(|(node, themes)| HostEvent::tree(node, None, themes));
                                send_event(&event_tx, result);
                            }
                            Cmd::Callback { id, value, seq } => {
                                apply_callback_batch(
                                    &writer,
                                    &pending,
                                    &next_id,
                                    &event_tx,
                                    vec![crate::protocol::CallbackCall { id, value }],
                                    seq,
                                );
                            }
                            Cmd::CallbackBatch { callbacks, seq } => {
                                apply_callback_batch(
                                    &writer, &pending, &next_id, &event_tx, callbacks, seq,
                                );
                            }
                            Cmd::Reload => {
                                let result =
                                    rpc(&writer, &pending, &next_id, json!({"op": "reload"}))
                                        .and_then(|value| parse_tree(&value))
                                        .map(|(node, themes)| HostEvent::tree(node, None, themes));
                                send_event(&event_tx, result);
                            }
                        },
                    }
                }
            }
        })?;

    Ok(ClojureHost {
        nrepl_port,
        app,
        cmd_tx,
        event_rx,
    })
}

pub fn start() -> Result<ClojureHost> {
    attach(connect_to_clojure()?)
}

pub fn protocol_test() -> Result<()> {
    println!("[host] running protocol test (no GPUI window)");
    let host = start()?;
    host.cmd_tx.send(Cmd::Render)?;

    let started = Instant::now();
    let mut tree = None;
    while started.elapsed() < Duration::from_secs(30) {
        match host.event_rx.recv_blocking() {
            Ok(HostEvent::Tree(t, _, _)) => {
                tree = Some(t);
                break;
            }
            Ok(HostEvent::Ready { .. }) => continue,
            Ok(
                HostEvent::RenderRequested
                | HostEvent::PickDirectory { .. }
                | HostEvent::RevealPath { .. }
                | HostEvent::OpenPath { .. }
                | HostEvent::CapturePreview { .. },
            ) => continue,
            Ok(HostEvent::Error(err)) => bail!("Clojure error: {err}"),
            Err(err) => bail!("bridge closed: {err}"),
        }
    }
    let tree = tree.context("did not receive a UI tree")?;
    println!("[host] received Clojure UI tree");
    if !tree.contains_text("clj-gpui") {
        bail!("tree did not contain label 'clj-gpui': {tree:?}");
    }
    if !tree.contains_text("Count: 0") {
        bail!("tree did not contain initial count: {tree:?}");
    }

    let mut hover_provider = None;
    crate::overlay::walk_nodes(&tree, "root", &mut |node, _| {
        if node.id.as_deref() == Some("provider-test") {
            hover_provider = node
                .lsp
                .as_ref()
                .and_then(|lsp| lsp.get("hover"))
                .and_then(Value::as_str)
                .map(str::to_string);
        }
    });
    let hover_provider = hover_provider.context("missing test editor provider")?;
    let request_provider = |await_count: Option<u64>| -> Result<_> {
        let (response, receiver) = async_channel::bounded(1);
        host.cmd_tx.send(Cmd::Provider {
            id: hover_provider.clone(),
            params: json!({"text":"hello","offset":5,"await-count":await_count}),
            response,
        })?;
        Ok(receiver)
    };
    let initial = request_provider(None)?
        .recv_blocking()?
        .map_err(anyhow::Error::msg)?;
    if initial["contents"]["value"] != "hello:0" {
        bail!("unexpected initial provider result: {initial}");
    }
    // The provider waits for the button mutation. A blocking reader or command
    // worker would deadlock here instead of delivering the callback below.
    let waiting_provider = request_provider(Some(1))?;

    let plus = tree
        .find_button("+")
        .and_then(|node| node.on_click.clone())
        .context("no '+' button with a callback id")?;
    println!("[host] invoking Clojure callback {plus}");
    host.cmd_tx.send(Cmd::Callback {
        id: plus,
        value: None,
        seq: None,
    })?;

    let started = Instant::now();
    let mut updated = None;
    while started.elapsed() < Duration::from_secs(30) {
        match host.event_rx.recv_blocking() {
            Ok(HostEvent::Tree(t, _, _)) => {
                updated = Some(t);
                break;
            }
            Ok(HostEvent::Error(err)) => bail!("Clojure error after click: {err}"),
            Ok(_) => continue,
            Err(err) => bail!("bridge closed: {err}"),
        }
    }
    let updated = updated.context("did not receive a tree after callback")?;
    if !updated.contains_text("Count: 1") {
        bail!("atom did not update after Clojure callback: {updated:?}");
    }
    println!("[host] atom updated and tree rerendered (Count: 1)");
    let waited = waiting_provider
        .recv_blocking()?
        .map_err(anyhow::Error::msg)?;
    if waited["contents"]["value"] != "hello:1" {
        bail!("provider did not observe callback: {waited}");
    }
    let fresh = request_provider(None)?
        .recv_blocking()?
        .map_err(anyhow::Error::msg)?;
    if fresh["contents"]["value"] != "hello:1" {
        bail!("provider id retained a stale function: {fresh}");
    }
    println!(
        "[host] asynchronous providers use current Clojure functions without blocking callbacks"
    );

    host.cmd_tx.send(Cmd::Reload)?;
    let started = Instant::now();
    let mut reloaded = false;
    while started.elapsed() < Duration::from_secs(30) {
        match host.event_rx.recv_blocking() {
            Ok(HostEvent::Tree(t, _, _)) => {
                if t.contains_text("Count: 1") {
                    reloaded = true;
                    break;
                }
            }
            Ok(HostEvent::Error(err)) => bail!("reload failed: {err}"),
            Ok(_) => continue,
            Err(err) => bail!("bridge closed: {err}"),
        }
    }
    if !reloaded {
        bail!("reload did not preserve defonce state");
    }
    println!("[host] reload preserved defonce atom state");
    println!("[host] protocol test passed");
    Ok(())
}

#[cfg(test)]
#[path = "overlay_regression_tests.rs"]
mod overlay_regression_tests;

#[cfg(test)]
mod tree_error_tests {
    use super::*;

    #[test]
    fn invalid_tree_reports_nested_field_and_type_in_the_error_event() {
        for (tree, field, expected) in [
            (
                json!({"type": "window", "children": [
                    {"type": "button", "disabled": null}
                ]}),
                "children[0].disabled",
                "invalid type: null, expected a boolean",
            ),
            (
                json!({"type": "select", "options": [
                    {"id": "first", "disabled": "false"}
                ]}),
                "options[0].disabled",
                "expected a boolean",
            ),
            (
                json!({"type": "window", "children": [
                    {"type": "label", "font-size": "large"}
                ]}),
                "children[0].font-size",
                "expected f32",
            ),
        ] {
            let (tx, rx) = async_channel::unbounded();
            send_event(
                &tx,
                parse_tree(&json!({"ok": true, "tree": tree}))
                    .map(|(node, themes)| HostEvent::tree(node, None, themes)),
            );
            let HostEvent::Error(message) = rx.recv_blocking().unwrap() else {
                panic!("invalid tree must send an error event");
            };
            assert!(message.contains(field), "{message}");
            assert!(message.contains(expected), "{message}");
        }
    }

    #[test]
    fn valid_booleans_and_optional_nulls_keep_their_protocol_meaning() {
        let (node, _) = parse_tree(&json!({"ok": true, "tree": {
            "type": "window", "children": [
                {"type": "button"},
                {"type": "button", "disabled": false},
                {"type": "button", "disabled": true},
                {"type": "button", "focus-ring": null}
            ]
        }}))
        .unwrap();
        assert!(!node.children[0].disabled);
        assert!(!node.children[1].disabled);
        assert!(node.children[2].disabled);
        assert_eq!(node.children[3].focus_ring, None);
    }
}
