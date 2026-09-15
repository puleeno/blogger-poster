use blogger_client::models::{Blog, Post};
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bridge::{block_on, core};
use crate::error::BridgeError;

pyo3::create_exception!(blogger_automation, PythonBridgeError, PyException);

pub(crate) fn to_pyerr(e: BridgeError) -> PyErr {
    PyErr::new::<PythonBridgeError, _>(e.to_string())
}

pub(crate) fn cvt<E: Into<BridgeError>>(e: E) -> PyErr {
    to_pyerr(e.into())
}

// ---------------------------------------------------------------- py classes

#[pyclass]
#[derive(Clone)]
pub struct BlogInfo {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub description: String,
    #[pyo3(get)]
    pub url: String,
    #[pyo3(get)]
    pub status: Option<String>,
    #[pyo3(get)]
    pub posts_count: i64,
    #[pyo3(get)]
    pub posts_default_draft: bool,
    #[pyo3(get)]
    pub pages_default_draft: bool,
}

impl From<&Blog> for BlogInfo {
    fn from(b: &Blog) -> Self {
        BlogInfo {
            id: b.id.clone(),
            name: b.name.clone(),
            description: b.description.clone(),
            url: b.url.clone(),
            status: b.status.clone(),
            posts_count: b.post_count(),
            posts_default_draft: b.posts_default_draft(),
            pages_default_draft: b.pages_default_draft(),
        }
    }
}

#[pyclass]
#[derive(Clone)]
pub struct BlogPost {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub content: String,
    #[pyo3(get)]
    pub status: String,
    #[pyo3(get)]
    pub url: Option<String>,
    #[pyo3(get)]
    pub published: Option<String>,
    #[pyo3(get)]
    pub updated: Option<String>,
    #[pyo3(get)]
    pub is_draft: bool,
    #[pyo3(get)]
    pub labels: Vec<String>,
}

impl From<&Post> for BlogPost {
    fn from(p: &Post) -> Self {
        BlogPost {
            id: p.id.clone(),
            title: p.title.clone(),
            content: p.content.clone(),
            status: p.status.clone(),
            url: p.url.clone(),
            published: p.published.clone(),
            updated: p.updated.clone(),
            is_draft: p.is_draft(),
            labels: p.labels.clone(),
        }
    }
}

#[pyclass]
#[derive(Clone)]
pub struct TabInfo {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub url: String,
    #[pyo3(get)]
    pub target_type: Option<String>,
}

impl From<&blogger_cdp::Target> for TabInfo {
    fn from(t: &blogger_cdp::Target) -> Self {
        TabInfo {
            id: t.id.clone(),
            title: t.title.clone(),
            url: t.url.clone(),
            target_type: t.target_type.clone(),
        }
    }
}

#[pyclass]
#[derive(Clone)]
pub struct OAuthState {
    #[pyo3(get)]
    pub configured: bool,
    #[pyo3(get)]
    pub has_tokens: bool,
    #[pyo3(get)]
    pub expired: bool,
    #[pyo3(get)]
    pub has_refresh: Option<String>,
}

// ---------------------------------------------------------------- helpers

fn json_to_py<'py>(py: Python<'py>, v: &serde_json::Value) -> PyResult<Bound<'py, PyAny>> {
    let json_mod = py.import("json")?;
    let s = serde_json::to_string(v).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let py_string = s.into_pyobject(py)?;
    json_mod.call_method1("loads", (py_string,))
}

// ---------------------------------------------------------------- version / misc

#[pyfunction]
fn version() -> PyResult<String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[pyfunction]
fn app_data_dir() -> PyResult<String> {
    let c = core().map_err(cvt)?;
    let p = c.blogger.appdata_dir();
    Ok(p.to_string_lossy().to_string())
}

#[pyfunction]
fn log_info(msg: String) {
    log::info!("[python] {msg}");
}

// ---------------------------------------------------------------- chrome

#[pyfunction]
fn chrome_connect(port: u16, target_id: Option<String>) -> PyResult<bool> {
    let c = core().map_err(cvt)?;
    block_on(c.chrome.connect(port, target_id)).map_err(cvt)?;
    Ok(true)
}

#[pyfunction]
fn chrome_connected() -> PyResult<bool> {
    let c = core().map_err(cvt)?;
    Ok(block_on(c.chrome.connected()))
}

#[pyfunction]
fn chrome_list_tabs() -> PyResult<Vec<TabInfo>> {
    let c = core().map_err(cvt)?;
    // avoid borrowing across await: bridge method returns future
    let xs = block_on(async { c.chrome.list_tabs().await }).map_err(cvt)?;
    Ok(xs.iter().map(TabInfo::from).collect())
}

#[pyfunction]
fn chrome_open_tab(url: String) -> PyResult<TabInfo> {
    let c = core().map_err(cvt)?;
    let t = block_on(c.chrome.open_tab(&url)).map_err(cvt)?;
    Ok(TabInfo::from(&t))
}

#[pyfunction]
fn chrome_attach(target_id: String) -> PyResult<()> {
    let c = core().map_err(cvt)?;
    block_on(c.chrome.attach_to(target_id)).map_err(cvt)
}

#[pyfunction]
fn chrome_close_tab(target_id: String) -> PyResult<()> {
    let c = core().map_err(cvt)?;
    block_on(c.chrome.close_tab(&target_id)).map_err(cvt)
}

#[pyfunction]
fn chrome_eval(target_id: Option<String>, js: String) -> PyResult<String> {
    let c = core().map_err(cvt)?;
    let v = block_on(c.chrome.eval(target_id.as_deref(), &js)).map_err(cvt)?;
    serde_json::to_string(&v).map(Into::into).map_err(|e| PyValueError::new_err(e.to_string()))
}

#[pyfunction]
fn chrome_upload_image(
    target_id: Option<String>,
    local_path: String,
    timeout_ms: u64,
) -> PyResult<String> {
    let c = core().map_err(cvt)?;
    block_on(async { c.chrome.upload_image(target_id.as_deref(), &local_path, timeout_ms).await })
        .map_err(cvt)
}

#[pyfunction]
fn chrome_ensure_editor(
    blog_id: String,
    editor_mode: Option<String>,
    draft_title: Option<String>,
    draft_content: Option<String>,
) -> PyResult<String> {
    let c = core().map_err(cvt)?;
    block_on(async {
        c.chrome
            .ensure_blogger_editor(
                &blog_id,
                editor_mode.as_deref().unwrap_or("ide"),
                draft_title.as_deref(),
                draft_content.as_deref(),
            )
            .await
    })
    .map_err(cvt)
}

#[pyfunction]
fn chrome_wait_ready(target_id: String) -> PyResult<()> {
    let c = core().map_err(cvt)?;
    block_on(c.chrome.wait_ready(target_id)).map_err(cvt)
}

// ---------------------------------------------------------------- oauth

#[pyfunction]
fn oauth_set_config(client_id: String, client_secret: String, redirect_uri: Option<String>) -> PyResult<()> {
    let c = core().map_err(cvt)?;
    let cfg = blogger_client::OAuthConfig {
        client_id,
        client_secret,
        redirect_uri: redirect_uri.unwrap_or_else(|| "http://127.0.0.1:8123/callback".into()),
    };
    block_on(c.blogger.set_oauth_config(cfg));
    Ok(())
}

#[pyfunction]
fn oauth_status(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let c = core().map_err(cvt)?;
    let st = block_on(c.blogger.oauth_status());
    let json = serde_json::json!({
        "configured": st.configured,
        "has_tokens": st.has_tokens,
        "expired": st.expired,
        "has_refresh": st.has_refresh,
    });
    json_to_py(py, &json)
}

#[pyfunction]
fn oauth_complete(code: String) -> PyResult<()> {
    let c = core().map_err(cvt)?;
    block_on(c.blogger.complete_oauth(&code)).map_err(cvt)
}

#[pyfunction]
fn oauth_clear() -> PyResult<()> {
    let c = core().map_err(cvt)?;
    block_on(c.blogger.clear_tokens());
    Ok(())
}

// ---------------------------------------------------------------- blogger

#[pyfunction]
fn blogger_list_blogs(refresh: bool) -> PyResult<Vec<BlogInfo>> {
    let c = core().map_err(cvt)?;
    let blogs = block_on(c.blogger.list_blogs(refresh)).map_err(cvt)?;
    Ok(blogs.iter().map(BlogInfo::from).collect())
}

#[pyfunction]
fn blogger_get_blog(blog_id: String, refresh: bool) -> PyResult<BlogInfo> {
    let c = core().map_err(cvt)?;
    let b = block_on(c.blogger.get_blog(&blog_id, refresh)).map_err(cvt)?;
    Ok(BlogInfo::from(&b))
}

#[pyfunction]
fn blogger_tags(blog_id: String, refresh: bool) -> PyResult<Vec<String>> {
    let c = core().map_err(cvt)?;
    block_on(c.blogger.tags(&blog_id, refresh)).map_err(cvt)
}

#[pyfunction]
fn blogger_add_tag(blog_id: String, name: String) -> PyResult<Vec<String>> {
    let c = core().map_err(cvt)?;
    block_on(c.blogger.add_tag(&blog_id, &name)).map_err(cvt)
}

#[pyfunction]
fn blogger_remove_tag(blog_id: String, name: String) -> PyResult<()> {
    let c = core().map_err(cvt)?;
    block_on(c.blogger.remove_tag(&blog_id, &name)).map_err(cvt)
}

#[pyfunction]
fn blogger_settings(py: Python<'_>, blog_id: String, refresh: bool) -> PyResult<Bound<'_, PyAny>> {
    let c = core().map_err(cvt)?;
    let blog = block_on(c.blogger.get_blog(&blog_id, refresh)).map_err(cvt)?;
    let json = serde_json::to_value(&blog).map_err(cvt)?;
    json_to_py(py, &json)
}

#[pyfunction]
fn blogger_list_posts(blog_id: String, refresh: bool) -> PyResult<Vec<BlogPost>> {
    let c = core().map_err(cvt)?;
    let posts = block_on(c.blogger.list_posts(&blog_id, refresh)).map_err(cvt)?;
    Ok(posts.iter().map(BlogPost::from).collect())
}

#[pyfunction]
fn blogger_get_post(blog_id: String, post_id: String, refresh: bool) -> PyResult<BlogPost> {
    let c = core().map_err(cvt)?;
    let p = block_on(c.blogger.get_post(&blog_id, &post_id, refresh)).map_err(cvt)?;
    Ok(BlogPost::from(&p))
}

#[pyfunction]
fn blogger_create_post(
    blog_id: String,
    title: String,
    content: String,
    labels: Vec<String>,
    is_draft: bool,
) -> PyResult<BlogPost> {
    let c = core().map_err(cvt)?;
    let input = blogger_client::PostInput {
        title,
        content,
        labels,
    };
    let p = block_on(c.blogger.create_post(&blog_id, &input, is_draft)).map_err(cvt)?;
    Ok(BlogPost::from(&p))
}

#[pyfunction]
fn blogger_update_post(
    blog_id: String,
    post_id: String,
    title: String,
    content: String,
    labels: Vec<String>,
) -> PyResult<BlogPost> {
    let c = core().map_err(cvt)?;
    let input = blogger_client::PostInput {
        title,
        content,
        labels,
    };
    let p = block_on(c.blogger.update_post(&blog_id, &post_id, &input)).map_err(cvt)?;
    Ok(BlogPost::from(&p))
}

#[pyfunction]
fn blogger_publish_post(blog_id: String, post_id: String) -> PyResult<BlogPost> {
    let c = core().map_err(cvt)?;
    let p = block_on(c.blogger.publish_post(&blog_id, &post_id)).map_err(cvt)?;
    Ok(BlogPost::from(&p))
}

#[pyfunction]
fn blogger_revert_post(blog_id: String, post_id: String) -> PyResult<BlogPost> {
    let c = core().map_err(cvt)?;
    let p = block_on(c.blogger.revert_post(&blog_id, &post_id)).map_err(cvt)?;
    Ok(BlogPost::from(&p))
}

#[pyfunction]
fn blogger_delete_post(blog_id: String, post_id: String) -> PyResult<()> {
    let c = core().map_err(cvt)?;
    block_on(c.blogger.delete_post(&blog_id, &post_id)).map_err(cvt)
}

// ---------------------------------------------------------------- scenarios

#[pyfunction]
fn scenario_dir() -> PyResult<String> {
    let c = core().map_err(cvt)?;
    Ok(c.scenarios_dir.to_string_lossy().to_string())
}

#[pyfunction]
fn scenario_list() -> PyResult<String> {
    crate::scenarios::list_scenarios().map_err(cvt)
}

#[pyfunction]
fn scenario_run(name: String, params_json: Option<String>) -> PyResult<String> {
    crate::scenarios::run_scenario(&name, params_json.as_deref()).map_err(cvt)
}

// ---------------------------------------------------------------- register

pub fn register(m: &Bound<'_, PyModule>) -> pyo3::PyResult<()> {
    m.add("BridgeError", m.py().get_type::<PythonBridgeError>())?;
    m.add_class::<BlogInfo>()?;
    m.add_class::<BlogPost>()?;
    m.add_class::<TabInfo>()?;
    m.add_class::<OAuthState>()?;

    macro_rules! add {
        ($($f:ident),* $(,)?) => {
            $(m.add_function(wrap_pyfunction!($f, m)?)?;)*
        };
    }
    add![
        version, app_data_dir, log_info, chrome_connect, chrome_connected, chrome_list_tabs,
        chrome_open_tab, chrome_attach, chrome_close_tab, chrome_eval, chrome_upload_image,
        chrome_ensure_editor, chrome_wait_ready, oauth_set_config, oauth_status, oauth_complete,
        oauth_clear, blogger_list_blogs, blogger_get_blog, blogger_tags, blogger_add_tag,
        blogger_remove_tag, blogger_settings, blogger_list_posts, blogger_get_post,
        blogger_create_post, blogger_update_post, blogger_publish_post, blogger_revert_post,
        blogger_delete_post, scenario_dir, scenario_list, scenario_run
    ];
    Ok(())
}