use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use crate::error::{Error, Result};
use crate::types::{BrowserInfo, CdpEvent, Target};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Sink = futures_util::stream::SplitSink<Ws, Message>;

/// Attaches to an already-running Chrome exposing a DevTools debugging port.
#[derive(Clone)]
pub struct CdpController {
    port: u16,
    http: reqwest::Client,
}

impl CdpController {
    pub fn new(port: u16) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("reqwest client");
        Self { port, http }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Quick health-check against `http://127.0.0.1:{port}/json/version`.
    pub async fn is_connected(&self) -> bool {
        matches!(
            self.http
                .get(format!("{}/json/version", self.base()))
                .send()
                .await,
            Ok(r) if r.status().is_success()
        )
    }

    pub async fn version(&self) -> Result<BrowserInfo> {
        let r = self.http.get(format!("{}/json/version", self.base())).send().await?;
        if !r.status().is_success() {
            return Err(Error::NotConnected(self.port));
        }
        Ok(r.json().await?)
    }

    pub async fn list_targets(&self) -> Result<Vec<Target>> {
        let r = self.http.get(format!("{}/json", self.base())).send().await?;
        if !r.status().is_success() {
            return Err(Error::NotConnected(self.port));
        }
        Ok(r.json().await?)
    }

    /// Open a new tab (reuses an existing tab whose url is the same if found).
    pub async fn open_tab(&self, url: &str) -> Result<Target> {
        if let Ok(targets) = self.list_targets().await {
            if let Some(existing) = targets
                .iter()
                .find(|t| t.target_type.as_deref() == Some("page") && t.url == url && t.ws_url.is_some())
            {
                return Ok(existing.clone());
            }
        }
        let u = format!("{}/json/new?{}", self.base(), urlencoding::encode(url));
        let r = self.http.get(&u).send().await?;
        if !r.status().is_success() {
            return Err(Error::NotConnected(self.port));
        }
        Ok(r.json().await?)
    }

    pub async fn close_tab(&self, target_id: &str) -> Result<()> {
        let r = self
            .http
            .get(format!("{}/json/close/{}", self.base(), target_id))
            .send()
            .await?;
        if !r.status().is_success() {
            return Err(Error::Other(format!("failed to close tab {target_id}")));
        }
        Ok(())
    }

    pub async fn attach(&self, target: &Target) -> Result<CdpConnection> {
        let ws_url = target
            .ws_url
            .clone()
            .ok_or_else(|| Error::Other("target has no websocket url".into()))?;
        CdpConnection::connect(&ws_url).await
    }

    pub fn pages<'a>(&self, targets: &'a [Target]) -> impl Iterator<Item = &'a Target> {
        targets
            .iter()
            .filter(|t| t.target_type.as_deref() == Some("page") && t.ws_url.is_some())
    }

    pub async fn attach_any_page(&self) -> Result<(Target, CdpConnection)> {
        let targets = self.list_targets().await?;
        let page = self
            .pages(&targets)
            .next()
            .ok_or(Error::NoTarget)?
            .clone();
        let conn = self.attach(&page).await?;
        Ok((page, conn))
    }
}

/// A live DevTools websocket session to a single target (page).
pub struct CdpConnection {
    sink: Sink,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>,
    events: mpsc::UnboundedReceiver<CdpEvent>,
    next_id: u64,
}

impl CdpConnection {
    pub async fn connect(ws_url: &str) -> Result<Self> {
        let (ws, _) = connect_async(ws_url).await?;
        let (sink, mut stream) = ws.split();

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>> = Arc::new(Mutex::new(HashMap::new()));
        let (events_tx, events_rx) = mpsc::unbounded_channel();

        let p = pending.clone();
        let ev = events_tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                let text = match msg {
                    Ok(Message::Text(t)) => t.to_string(),
                    Ok(_) => continue,
                    Err(e) => {
                        log::debug!("cdp websocket closed: {e}");
                        break;
                    }
                };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };

                if let Some(id) = v.get("id").and_then(|i| i.as_u64()) {
                    if let Some(tx) = p.lock().await.remove(&id) {
                        let result = match v.get("error") {
                            Some(err) => Err(Error::cdp("<call>", err.to_string())),
                            None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                        };
                        let _ = tx.send(result);
                    }
                } else if let Some(method) = v.get("method").and_then(|m| m.as_str()) {
                    let _ = ev.send(CdpEvent {
                        method: method.to_string(),
                        params: v.get("params").cloned().unwrap_or(Value::Null),
                    });
                }
            }
        });

        let mut conn = Self {
            sink,
            pending,
            events: events_rx,
            next_id: 0,
        };
        conn.enable_domains().await?;
        Ok(conn)
    }

    async fn enable_domains(&mut self) -> Result<()> {
        for method in ["Runtime.enable", "Page.enable", "DOM.enable", "Network.enable"] {
            self.call(method, Value::Null).await?;
        }
        Ok(())
    }

    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.call_timeout(method, params, Duration::from_secs(30)).await
    }

    pub async fn call_timeout(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let msg = serde_json::to_string(&json!({ "id": id, "method": method, "params": params }))?;
        self.sink.send(Message::Text(msg.into())).await?;

        tokio::time::timeout(timeout, rx)
            .await
            .map_err(|_| Error::Timeout(timeout.as_millis() as u64))?
            .map_err(|_| Error::Other("cdp channel closed".into()))?
    }

    /// Run an expression in the page and return its value.
    pub async fn eval_js(&mut self, expression: &str) -> Result<Value> {
        let res = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                    "userGesture": true,
                }),
            )
            .await?;
        if let Some(ex) = res.get("exceptionDetails") {
            let text = ex.get("text").and_then(|t| t.as_str()).unwrap_or("exception");
            let desc = ex.get("exception").and_then(|e| e.get("description")).and_then(|d| d.as_str());
            return Err(Error::Js(format!("{text}: {}", desc.unwrap_or(""))));
        }
        Ok(res
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// Poll an expression until it returns a truthy value.
    pub async fn wait_js(&mut self, expression: &str, timeout_ms: u64) -> Result<Value> {
        let start = std::time::Instant::now();
        loop {
            if start.elapsed().as_millis() as u64 >= timeout_ms {
                return Err(Error::Timeout(timeout_ms));
            }
            let v = self.eval_js(expression).await?;
            if js_truthy(&v) {
                return Ok(v);
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// Poll a JS selector/expression until present, returns result.
    pub async fn wait_selector(&mut self, css: &str, timeout_ms: u64) -> Result<Value> {
        let expr = format!(
            "(() => {{ const el = document.querySelector({}); if (!el) return null; return true; }})()",
            serde_json::to_string(css)?
        );
        self.wait_js(&expr, timeout_ms).await
    }

    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        self.call("Page.navigate", json!({ "url": url })).await?;
        self.wait_js(r"document.readyState === 'complete'", 60_000).await?;
        Ok(())
    }

    /// Programmatically click an element. Some pages report click as user gesture.
    pub async fn click_selector(&mut self, css: &str) -> Result<Value> {
        let expr = format!(
            "(() => {{ const el = document.querySelector({}); if (!el) return null; el.click(); return true; }})()",
            serde_json::to_string(css)?
        );
        self.eval_js(&expr).await
    }

    pub async fn js_on(&mut self, js: &str, field: &str) -> Option<Value> {
        self.eval_js(js).await.ok().and_then(|v| v.get(field).cloned())
    }

    /// Set the target of a file input to the given path, triggering `change`.
    pub async fn set_file_input(&mut self, css: &str, path: &Path) -> Result<()> {
        let doc = self
            .call("DOM.getDocument", json!({ "depth": -1, "pierce": true }))
            .await?;
        let root = doc["root"]["nodeId"]
            .as_i64()
            .ok_or_else(|| Error::Other("DOM.getDocument returned no nodeId".into()))?;
        let q = self
            .call("DOM.querySelector", json!({ "nodeId": root, "selector": css }))
            .await?;
        let node = q["nodeId"].as_i64().filter(|n| *n > 0).ok_or(Error::NoTarget)?;
        self.call(
            "DOM.setFileInputFiles",
            json!({
                "nodeId": node,
                "files": [path.to_string_lossy().to_string()],
            }),
        )
        .await?;
        Ok(())
    }

    /// Try a list of CSS selectors until an element exists, then set the file.
    pub async fn set_file_input_any(&mut self, selectors: &[&str], path: &Path) -> Result<()> {
        for (i, sel) in selectors.iter().enumerate() {
            let found = self
                .eval_js(&format!(
                    "!!document.querySelector({})",
                    serde_json::to_string(sel)?
                ))
                .await?;
            if found.is_boolean() && found == true {
                return self.set_file_input(sel, path).await.with_context_str(format!(
                    "set_file_input({sel}, selector-index {i}) failed"
                ));
            }
        }
        Err(Error::NoTarget)
    }

    pub async fn screenshot(&mut self) -> Result<String> {
        let res = self
            .call(
                "Page.captureScreenshot",
                json!({ "format": "png", "captureBeyondViewport": false }),
            )
            .await?;
        let b64 = res["data"].as_str().ok_or_else(|| Error::Other("no screenshot data".into()))?;
        Ok(format!("data:image/png;base64,{b64}"))
    }

    pub async fn wait_event(&mut self, method: &str, timeout_ms: u64) -> Result<CdpEvent> {
        let fut = async {
            loop {
                match self.events.recv().await {
                    Some(ev) if ev.method == method => return Ok(ev),
                    Some(_) => continue,
                    None => return Err(Error::Other("event channel closed".into())),
                }
            }
        };
        tokio::time::timeout(Duration::from_millis(timeout_ms), fut)
            .await
            .map_err(|_| Error::Timeout(timeout_ms))?
    }
}

impl Drop for CdpConnection {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.pending.try_lock() {
            for (_, tx) in guard.drain() {
                let _ = tx.send(Err(Error::Other("connection dropped".into())));
            }
        }
    }
}

fn js_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::String(s) => !s.is_empty(),
        Value::Number(_) => v.as_f64().map(|n| n != 0.0).unwrap_or(true),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

trait ResultCtx<T> {
    fn with_context_str(self, ctx: String) -> Result<T>;
}
impl<T> ResultCtx<T> for Result<T> {
    fn with_context_str(self, ctx: String) -> Result<T> {
        self.map_err(|e| match e {
            Error::Cdp { method, message } => Error::Cdp {
                method,
                message: format!("{ctx}: {message}"),
            },
            other => Error::Other(format!("{ctx}: {other}")),
        })
    }
}