use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use blogger_cdp::{CdpConnection, CdpController, Target};
use tokio::sync::Mutex;

use crate::error::{BridgeError, Result};

pub use blogger_client::BloggerService;

#[derive(Clone)]
pub struct BridgeCore {
    pub blogger: BloggerService,
    pub chrome: ChromeService,
    pub scenarios_dir: PathBuf,
    pub runtime: Arc<tokio::runtime::Runtime>,
}

static CORE: OnceLock<BridgeCore> = OnceLock::new();

pub fn init(appdata_dir: &Path, scenarios_dir: &Path) -> Result<()> {
    if CORE.get().is_some() {
        return Ok(());
    }
    let blogger = BloggerService::new(appdata_dir);
    let chrome = ChromeService::default();
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| BridgeError::other(format!("tokio runtime: {e}")))?,
    );
    std::fs::create_dir_all(scenarios_dir)?;
    CORE.set(BridgeCore {
        blogger,
        chrome,
        scenarios_dir: scenarios_dir.to_path_buf(),
        runtime,
    })
    .map_err(|_| BridgeError::other("bridge already initialized".to_string()))?;
    log::info!("python bridge initialized");
    Ok(())
}

pub fn core() -> Result<&'static BridgeCore> {
    CORE.get().ok_or(BridgeError::NotInitialized)
}

/// Run a future on the bridge's dedicated runtime from a non-async context
/// (e.g. a pyo3 call). Do NOT call this from inside that same runtime.
pub fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    core().expect("bridge").runtime.block_on(fut)
}

/// A live CDP session to a dedicated Blogger tab managed by the bridge.
#[derive(Default, Clone)]
pub struct ChromeService {
    inner: Arc<Mutex<SessionState>>,
}

#[derive(Default)]
struct SessionState {
    controller: Option<CdpController>,
    conn: Option<CdpConnection>,
    target_id: Option<String>,
}

impl ChromeService {
    pub async fn connect(&self, port: u16, prefer_target_id: Option<String>) -> Result<()> {
        let controller = CdpController::new(port);
        if !controller.is_connected().await {
            return Err(BridgeError::other(format!(
                "chrome not reachable at port {port}; start it with --remote-debugging-port={port} and a dedicated profile"
            )));
        }
        let mut st = self.inner.lock().await;
        st.controller = Some(controller);
        st.conn = None;
        st.target_id = prefer_target_id;
        Ok(())
    }

    pub async fn connected(&self) -> bool {
        match self.inner.lock().await.controller.as_ref() {
            Some(c) => c.is_connected().await,
            None => false,
        }
    }

    fn controller(&self) -> Result<CdpController> {
        self.inner
            .try_lock()
            .map_err(|_| BridgeError::other("chrome session busy".to_string()))?
            .controller
            .clone()
            .ok_or_else(|| BridgeError::other("chrome not connected".to_string()))
    }

    pub async fn list_tabs(&self) -> Result<Vec<Target>> {
        let c = self.controller()?;
        let all = c.list_targets().await?;
        Ok(c.pages(&all).cloned().collect())
    }

    pub async fn open_tab(&self, url: &str) -> Result<Target> {
        let c = self.controller()?;
        let t = c.open_tab(url).await?;
        self.attach_to(t.id.clone()).await?;
        Ok(t)
    }

    pub async fn close_tab(&self, target_id: &str) -> Result<()> {
        let c = self.controller()?;
        c.close_tab(target_id).await?;
        let mut st = self.inner.lock().await;
        if st.target_id.as_deref() == Some(target_id) {
            st.target_id = None;
            st.conn = None;
        }
        Ok(())
    }

    pub async fn attach_to(&self, target_id: String) -> Result<()> {
        let c = self.controller()?;
        let targets = c.list_targets().await?;
        let t = targets
            .iter()
            .find(|t| t.id == target_id)
            .ok_or_else(|| BridgeError::other(format!("target {target_id} not found")))?
            .clone();
        let conn = c.attach(&t).await?;
        let mut st = self.inner.lock().await;
        st.conn = Some(conn);
        st.target_id = Some(target_id);
        Ok(())
    }

    fn resolve_target(&self, st: &mut SessionState, target_id: Option<&str>) -> String {
        target_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| st.target_id.clone().unwrap_or_default())
    }

    /// Ensure the session points at a target and return a mutable handle to the
    /// connection for the duration of one call.
    async fn conn(
        &self,
        target_id: Option<&str>,
    ) -> Result<tokio::sync::MutexGuard<'_, SessionState>> {
        let mut st = self.inner.lock().await;
        let want = self.resolve_target(&mut st, target_id);
        if st.conn.is_none() || st.target_id.as_deref() != Some(want.as_str()) {
            // need (re)connect
            let targets = st
                .controller
.clone()
            .ok_or_else(|| BridgeError::other("chrome not connected".to_string()))?
                .list_targets()
                .await?;
            let c = st
                .controller
                .clone()
                .ok_or_else(|| BridgeError::other("chrome not connected".to_string()))?;
            let target = if want.is_empty() {
                c.pages(&targets).next().cloned().ok_or_else(|| {
                    BridgeError::other("no page target available in chrome".to_string())
                })?
            } else {
                targets
                    .iter()
                    .find(|t| t.id == want)
                    .cloned()
                    .ok_or_else(|| BridgeError::other(format!("target {want} not found")))?
            };
            let conn = c.attach(&target).await?;
            st.conn = Some(conn);
            st.target_id = Some(target.id);
        }
        Ok(st)
    }

    pub fn current_target(&self) -> Option<String> {
        self.inner
            .try_lock()
            .ok()
            .and_then(|st| st.target_id.clone())
    }

    pub async fn eval(&self, target_id: Option<&str>, js: &str) -> Result<serde_json::Value> {
        let mut st = self.conn(target_id).await?;
        let conn = st.conn.as_mut().expect("connected");
        Ok(conn.eval_js(js).await?)
    }

    /// Open (or focus) the Blogger compose editor for a blog, optionally
    /// pre-filling the draft title/content. Returns the target id.
    pub async fn ensure_blogger_editor(
        &self,
        blog_id: &str,
        editor_mode: &str,
        draft_title: Option<&str>,
        draft_content: Option<&str>,
    ) -> Result<String> {
        let base = match editor_mode {
            "classic" => format!("https://www.blogger.com/blog/post/edit/{blog_id}"),
            _ => format!("https://www.blogger.com/blog/post/edit/preview/{blog_id}"),
        };
        let target = self.open_tab(&base).await?;
        self.wait_ready(target.id.clone()).await?;

        if let (Some(title), Some(content)) = (draft_title, draft_content) {
            let js = format!(
                r#"(async () => {{
                    const titleSel = ['input[placeholder*="Title"]', 'input[placeholder*="title"]', 'input[title*="Title"]'];
                    for (const s of titleSel) {{
                        const t = document.querySelector(s);
                        if (t) {{ t.focus(); t.value = {title}; }}
                    }}
                    const body = document.querySelector('[contenteditable="true"]');
                    if (body) {{
                        body.focus();
                        document.execCommand('selectAll', false, null);
                        document.execCommand('insertHTML', false, {content});
                    }}
                    return true;
                }})()"#,
                title = serde_json::to_string(title)?,
                content = serde_json::to_string(content)?,
            );
            let _ = self.eval(Some(&target.id), &js).await;
        }
        Ok(target.id)
    }

    pub async fn wait_ready(&self, target_id: String) -> Result<()> {
        let mut st = self.conn(Some(&target_id)).await?;
        st.conn
            .as_mut()
            .expect("connected")
            .wait_js(
                r"document.readyState === 'complete' && !!document.querySelector('body')",
                90_000,
            )
            .await?;
        Ok(())
    }

    /// Upload a local image through the Blogger composer in the current tab and
    /// return its public URL. This is the "upload ngầm" core.
    pub async fn upload_image(
        &self,
        target_id: Option<&str>,
        local_path: &str,
        timeout_ms: u64,
    ) -> Result<String> {
        let path = std::path::Path::new(local_path);
        if !path.exists() {
            return Err(BridgeError::other(format!("file not found: {local_path}")));
        }
        let mut st = self.conn(target_id).await?;
        let conn = st.conn.as_mut().expect("connected");

        // 1. snapshot existing CDN images.
        let before = conn
            .eval_js(
                r#"(() => {
  const out = [];
  document.querySelectorAll('img').forEach(i => { const s = (i.getAttribute('src')||''); if (s.includes('blogger.googleusercontent.com')) out.push(s); });
  return JSON.stringify(out);
})()"#,
            )
            .await?
            .as_str()
            .unwrap_or("[]")
            .to_string();

        // 2. make sure a file input is present.
        let file_selector = Self::find_file_input(conn, 15_000).await?;

        // 3. push the local file into the input (fires upload).
        conn.set_file_input(&file_selector, path).await?;

        // 4. best-effort auto-confirm for the Blogger insert dialog.
        let _ = conn
            .eval_js(
                r#"(() => {
  const btns = Array.from(document.querySelectorAll('button, [role="button"]'));
  const confirm = btns.find(b => { const t = (b.textContent||'').trim().toLowerCase(); return /select|insert|add|ok|chèn|thêm|chọn|lựa chọn/.test(t) && t.length < 20 && b.offsetParent !== null; });
  if (confirm) { confirm.click(); return true; }
  return false;
})()"#,
            )
            .await;

        // 5. wait until a brand-new CDN image shows up.
        let before_json = serde_json::to_string(&before)?;
        let expr = format!(
            r#"(() => {{
  const before = new Set({before_json});
  const all = [];
  document.querySelectorAll('img').forEach(i => {{
    const s = (i.getAttribute('src')||'');
    if (s.includes('blogger.googleusercontent.com') && !before.has(s)) all.push(s);
  }});
  return all;
}})()"#
        );
        let result = conn.wait_js(&expr, timeout_ms).await?;

        let urls: Vec<String> = result
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        urls.into_iter().next().ok_or_else(|| {
            BridgeError::other("upload finished but no public image URL was found".to_string())
        })
    }

    async fn find_file_input(conn: &mut CdpConnection, timeout_ms: u64) -> Result<String> {
        log::info!("searching for a file input on the blog editor page");
        for sel in [
            "input[type=file]",
            "input[accept*='image']",
            "input[type=file]:not([style*='display: none'])",
        ] {
            if let Ok(v) = conn.wait_selector(sel, timeout_ms).await {
                if v.is_boolean() && v == true {
                    return Ok(sel.to_string());
                }
            }
        }
        Err(BridgeError::other(
            "no file input found; open the Blogger image uploader first".to_string(),
        ))
    }
}