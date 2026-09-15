use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::State;

use crate::settings::{AppSettings, SettingsStore};

type CmdResult<T> = Result<T, String>;

fn ie<T: std::fmt::Display>(e: T) -> String {
    e.to_string()
}

fn core() -> Result<&'static blogger_python::BridgeCore, String> {
    blogger_python::bridge_core().map_err(ie)
}

// ---------------------------------------------------------------- DTOs

#[derive(Serialize)]
pub struct TagInfo {
    pub name: String,
    pub source: String,
}

#[derive(Serialize)]
pub struct TabInfo {
    pub id: String,
    pub title: String,
    pub url: String,
    pub target_type: Option<String>,
}

impl From<&blogger_cdp::Target> for TabInfo {
    fn from(t: &blogger_cdp::Target) -> Self {
        Self {
            id: t.id.clone(),
            title: t.title.clone(),
            url: t.url.clone(),
            target_type: t.target_type.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct PostRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub published: Option<String>,
    pub updated: Option<String>,
    pub is_draft: bool,
    pub labels: Vec<String>,
    pub url: Option<String>,
    pub snippet: String,
}

impl From<&blogger_client::Post> for PostRow {
    fn from(p: &blogger_client::Post) -> Self {
        let cleaned = p
            .content
            .replace("</p>", " ")
            .replace("</div>", " ")
            .replace("</h1>", " ")
            .replace("</h2>", " ")
            .replace("</h3>", " ");
        let plain = strip_html(&cleaned);
        Self {
            id: p.id.clone(),
            title: p.title.clone(),
            status: p.status.clone(),
            published: p.published.clone(),
            updated: p.updated.clone(),
            is_draft: p.is_draft(),
            labels: p.labels.clone(),
            url: p.url.clone(),
            snippet: plain.chars().take(220).collect(),
        }
    }
}

fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------- settings

#[tauri::command]
pub fn get_settings(settings: State<'_, SettingsStore>) -> AppSettings {
    settings.get()
}

#[tauri::command]
pub fn update_settings(
    settings: State<'_, SettingsStore>,
    patch: serde_json::Value,
) -> AppSettings {
    settings.update(patch)
}

// ---------------------------------------------------------------- chrome

#[tauri::command]
pub async fn chrome_connect(port: u16, settings: State<'_, SettingsStore>) -> CmdResult<()> {
    let c = core()?;
    c.chrome
        .connect(port, settings.get().chrome_target_id.clone())
        .await
        .map_err(ie)
}

#[tauri::command]
pub async fn chrome_status() -> CmdResult<serde_json::Value> {
    let c = core()?;
    let connected = c.chrome.connected().await;
    let tabs = if connected {
        c.chrome.list_tabs().await.map(|t| t.iter().map(TabInfo::from).collect::<Vec<_>>()).unwrap_or_default()
    } else {
        Vec::new()
    };
    Ok(json!({
        "connected": connected,
        "tabs": tabs,
    }))
}

/// Launch Chrome with a dedicated editor profile + remote-debugging-port.
#[tauri::command]
pub async fn chrome_launch(port: u16, settings: State<'_, SettingsStore>) -> CmdResult<()> {
    use std::process::Command;

    let appdata = crate::settings::app_data_dir();
    let profile = appdata.join("profiles").join(format!("editor-{port}"));
    std::fs::create_dir_all(&profile).map_err(ie)?;

    let candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ];
    let chrome = candidates.iter().find(|p| std::path::Path::new(p).exists()).map(|s| s.to_string());

    let mut cmd = match chrome {
        Some(path) => {
            let mut c = Command::new(&path);
            c.arg(format!("--remote-debugging-port={port}"))
                .arg(format!("--user-data-dir={}", profile.display()))
                .arg("--remote-allow-origins=*")
                .arg("--no-first-run")
                .arg("--new-window")
                .arg("about:blank");
            c
        }
        None => {
            #[cfg(target_os = "macos")]
            {
                let mut c = Command::new("open");
                c.arg("-a").arg("Google Chrome").arg("--args")
                    .arg(format!("--remote-debugging-port={port}"))
                    .arg(format!("--user-data-dir={}", profile.display()))
                    .arg("--remote-allow-origins=*")
                    .arg("--no-first-run");
                c
            }
            #[cfg(not(target_os = "macos"))]
            {
                let mut c = Command::new("google-chrome");
                c.arg(format!("--remote-debugging-port={port}"))
                    .arg(format!("--user-data-dir={}", profile.display()))
                    .arg("--remote-allow-origins=*");
                c
            }
        }
    };

    cmd.spawn().map_err(ie)?;

    // wait until the debugging endpoint answers
    let c = core()?;
    let mut ok = false;
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if c.chrome.connected().await {
            ok = true;
            break;
        }
    }
    if !ok {
        return Err("Chrome started but the debugging port did not open".into());
    }
    // remember the target set
    let _ = settings.get();
    let c = core()?;
    c.chrome.connect(port, None).await.map_err(ie)?;
    Ok(())
}

#[tauri::command]
pub async fn chrome_list_tabs() -> CmdResult<Vec<TabInfo>> {
    let c = core()?;
    let tabs = c.chrome.list_tabs().await.map_err(ie)?;
    Ok(tabs.iter().map(TabInfo::from).collect())
}

#[tauri::command]
pub async fn chrome_close_tab(target_id: String) -> CmdResult<()> {
    let c = core()?;
    c.chrome.close_tab(&target_id).await.map_err(ie)
}

/// Open the Blogger compose editor for a blog (returns the chrome target id).
#[tauri::command]
pub async fn chrome_open_editor(
    blog_id: String,
    draft_title: Option<String>,
    draft_content: Option<String>,
    settings: State<'_, SettingsStore>,
) -> CmdResult<String> {
    let c = core()?;
    let mode = settings.get().editor_mode;
    c.chrome
        .ensure_blogger_editor(&blog_id, &mode, draft_title.as_deref(), draft_content.as_deref())
        .await
        .map_err(ie)
}

#[tauri::command]
pub async fn chrome_evaluate(target_id: Option<String>, js: String) -> CmdResult<serde_json::Value> {
    let c = core()?;
    c.chrome.eval(target_id.as_deref(), &js).await.map_err(ie)
}

// ---------------------------------------------------------------- image upload

#[tauri::command]
pub async fn upload_image(
    path: String,
    target_id: Option<String>,
    settings: State<'_, SettingsStore>,
) -> CmdResult<String> {
    let c = core()?;
    let timeout = settings.get().image_timeout_ms;
    c.chrome
        .upload_image(target_id.as_deref(), &path, timeout)
        .await
        .map_err(ie)
}

// ---------------------------------------------------------------- oauth

#[tauri::command]
pub async fn oauth_get_config() -> CmdResult<serde_json::Value> {
    let c = core()?;
    let cfg = c.blogger.oauth_config().await;
    Ok(json!({
        "client_id": cfg.client_id,
        "client_secret": cfg.client_secret,
        "redirect_uri": cfg.redirect_uri,
        "configured": cfg.is_valid(),
    }))
}

#[tauri::command]
pub async fn oauth_set_config(client_id: String, client_secret: String, redirect_uri: Option<String>) -> CmdResult<()> {
    let c = core()?;
    let cfg = blogger_client::OAuthConfig {
        client_id,
        client_secret,
        redirect_uri: redirect_uri.unwrap_or_else(|| "http://127.0.0.1:8123/callback".into()),
    };
    c.blogger.set_oauth_config(cfg).await;
    Ok(())
}

/// Kicks off the Google OAuth flow: start a loopback callback server and open
/// the consent page in the controlled Chrome tab. The completion happens in the
/// background; poll `oauth_status` until `has_tokens`.
#[tauri::command]
pub async fn oauth_login_start() -> CmdResult<serde_json::Value> {
    use blogger_client::oauth::start_oauth_listener;
    let c = core()?;
    let cfg = c.blogger.oauth_config().await;
    if !cfg.is_valid() {
        return Err("OAuth client_id/client_secret chưa được cấu hình".into());
    }
    let state = format!("bp{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
    let auth_url = cfg.authorize_url(&state);
    let (_cb, rx) = start_oauth_listener(8123).await.map_err(ie)?;

    // open consent in the editor chrome profile
    let target = match c.chrome.open_tab(&auth_url).await {
        Ok(t) => t,
        Err(_) => {
            // Chrome not controlled yet: tell the user to open the url manually
            return Ok(json!({
                "url": auth_url,
                "target_id": null,
                "manual": true,
            }));
        }
    };

    let blogger = c.blogger.clone();
    tokio::spawn(async move {
        match rx.await {
            Ok(code) if !code.is_empty() => {
                log::info!("oauth callback received, exchanging code");
                let _ = blogger.complete_oauth(&code).await;
            }
            Ok(_) => log::warn!("oauth callback failed (no code)"),
            Err(e) => log::warn!("oauth callback channel closed: {e}"),
        }
    });

    Ok(json!({
        "url": auth_url,
        "target_id": target.id,
        "manual": false,
    }))
}

#[tauri::command]
pub async fn oauth_complete(code: String) -> CmdResult<()> {
    let c = core()?;
    c.blogger.complete_oauth(&code).await.map_err(ie)
}

#[tauri::command]
pub async fn oauth_status() -> CmdResult<serde_json::Value> {
    let c = core()?;
    let st = c.blogger.oauth_status().await;
    Ok(json!({
        "configured": st.configured,
        "has_tokens": st.has_tokens,
        "expired": st.expired,
        "has_refresh": st.has_refresh,
    }))
}

#[tauri::command]
pub async fn oauth_logout() -> CmdResult<()> {
    let c = core()?;
    c.blogger.clear_tokens().await;
    Ok(())
}

// ---------------------------------------------------------------- blogger

#[tauri::command]
pub async fn blogger_list_blogs(refresh: bool) -> CmdResult<Vec<blogger_client::Blog>> {
    let c = core()?;
    c.blogger.list_blogs(refresh).await.map_err(ie)
}

#[tauri::command]
pub async fn blogger_get_blog(blog_id: String, refresh: bool) -> CmdResult<blogger_client::Blog> {
    let c = core()?;
    c.blogger.get_blog(&blog_id, refresh).await.map_err(ie)
}

#[tauri::command]
pub async fn blogger_tags(
    blog_id: String,
    refresh: bool,
    settings: State<'_, SettingsStore>,
) -> CmdResult<Vec<TagInfo>> {
    let c = core()?;
    let blog_id = if blog_id.is_empty() {
        settings.get().selected_blog_id.clone().unwrap_or_default()
    } else {
        blog_id
    };
    if blog_id.is_empty() {
        return Ok(Vec::new());
    }
    if refresh {
        let _ = c.blogger.sync_labels(&blog_id).await;
    }
    let tags = c.blogger.cache().label_sources(&blog_id).await.map_err(ie)?;
    Ok(tags
        .into_iter()
        .map(|(name, source)| TagInfo { name, source })
        .collect())
}

#[tauri::command]
pub async fn blogger_add_tag(
    blog_id: String,
    name: String,
    settings: State<'_, SettingsStore>,
) -> CmdResult<Vec<String>> {
    let c = core()?;
    let blog_id = if blog_id.is_empty() {
        settings.get().selected_blog_id.clone().unwrap_or_default()
    } else {
        blog_id
    };
    if blog_id.is_empty() {
        return Err("chưa chọn blog".into());
    }
    c.blogger.add_tag(&blog_id, &name).await.map_err(ie)
}

#[tauri::command]
pub async fn blogger_remove_tag(
    blog_id: String,
    name: String,
    settings: State<'_, SettingsStore>,
) -> CmdResult<()> {
    let c = core()?;
    let blog_id = if blog_id.is_empty() {
        settings.get().selected_blog_id.clone().unwrap_or_default()
    } else {
        blog_id
    };
    if blog_id.is_empty() {
        return Ok(());
    }
    c.blogger.remove_tag(&blog_id, &name).await.map_err(ie)
}

#[tauri::command]
pub async fn blogger_settings(blog_id: String, refresh: bool) -> CmdResult<blogger_client::Blog> {
    let c = core()?;
    c.blogger.get_blog(&blog_id, refresh).await.map_err(ie)
}

#[tauri::command]
pub async fn blogger_posts(
    blog_id: String,
    refresh: bool,
) -> CmdResult<Vec<PostRow>> {
    let posts = blogger_posts_raw(blog_id, refresh).await?;
    Ok(posts.iter().map(PostRow::from).collect())
}

async fn blogger_posts_raw(blog_id: String, refresh: bool) -> CmdResult<Vec<blogger_client::Post>> {
    let c = core()?;
    c.blogger.list_posts(&blog_id, refresh).await.map_err(ie)
}

#[tauri::command]
pub async fn blogger_get_post(
    blog_id: String,
    post_id: String,
    refresh: bool,
) -> CmdResult<blogger_client::Post> {
    let c = core()?;
    c.blogger.get_post(&blog_id, &post_id, refresh).await.map_err(ie)
}

#[tauri::command]
pub async fn blogger_create_post(
    blog_id: String,
    title: String,
    content: String,
    labels: Vec<String>,
    is_draft: bool,
) -> CmdResult<blogger_client::Post> {
    let c = core()?;
    let input = blogger_client::PostInput { title, content, labels };
    c.blogger.create_post(&blog_id, &input, is_draft).await.map_err(ie)
}

#[tauri::command]
pub async fn blogger_update_post(
    blog_id: String,
    post_id: String,
    title: String,
    content: String,
    labels: Vec<String>,
) -> CmdResult<blogger_client::Post> {
    let c = core()?;
    let input = blogger_client::PostInput { title, content, labels };
    c.blogger.update_post(&blog_id, &post_id, &input).await.map_err(ie)
}

#[tauri::command]
pub async fn blogger_publish_post(blog_id: String, post_id: String) -> CmdResult<blogger_client::Post> {
    let c = core()?;
    c.blogger.publish_post(&blog_id, &post_id).await.map_err(ie)
}

#[tauri::command]
pub async fn blogger_revert_post(blog_id: String, post_id: String) -> CmdResult<blogger_client::Post> {
    let c = core()?;
    c.blogger.revert_post(&blog_id, &post_id).await.map_err(ie)
}

#[tauri::command]
pub async fn blogger_delete_post(blog_id: String, post_id: String) -> CmdResult<()> {
    let c = core()?;
    c.blogger.delete_post(&blog_id, &post_id).await.map_err(ie)
}

/// Refresh the local cache (tags / settings / posts) for a blog from Blogger.
#[tauri::command]
pub async fn blogger_refresh_cache(blog_id: String, kinds: Vec<String>) -> CmdResult<()> {
    let c = core()?;
    for kind in kinds {
        match kind.as_str() {
            "tags" => {
                let _ = c.blogger.sync_labels(&blog_id).await;
            }
            "blog" => {
                let _ = c.blogger.get_blog(&blog_id, true).await;
            }
            "posts" => {
                let _ = c.blogger.list_posts(&blog_id, true).await;
            }
            _ => {}
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- python scenarios

#[derive(Serialize, Deserialize)]
pub struct ScenarioInfo {
    pub name: String,
    pub description: String,
    pub file: String,
    #[serde(default)]
    pub params: Vec<serde_json::Value>,
}

#[tauri::command]
pub async fn scenarios_list() -> CmdResult<Vec<ScenarioInfo>> {
    let list = tokio::task::spawn_blocking(blogger_python::scenarios::list_scenarios)
        .await
        .map_err(ie)?
        .map_err(ie)?;
    serde_json::from_str(&list).map_err(ie)
}

#[tauri::command]
pub async fn scenario_run(name: String, params: serde_json::Value) -> CmdResult<serde_json::Value> {
    let params_str = serde_json::to_string(&params).map_err(ie)?;
    let out = tokio::task::spawn_blocking(move || {
        blogger_python::scenarios::run_scenario(&name, Some(&params_str))
    })
    .await
    .map_err(ie)?
    .map_err(ie)?;
    serde_json::from_str(&out).map_err(ie)
}

#[tauri::command]
pub async fn python_selftest() -> CmdResult<String> {
    let out = tokio::task::spawn_blocking(blogger_python::scenarios::self_test)
        .await
        .map_err(ie)?
        .map_err(ie)?;
    Ok(out.join(", "))
}