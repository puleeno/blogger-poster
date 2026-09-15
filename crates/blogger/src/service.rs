use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::cache::Cache;
use crate::error::{Error, Result};
use crate::models::{Blog, BlogsList, Post, PostInput, PostsList};
use crate::oauth::{OAuthConfig, TokenPair, exchange_code, refresh_token};

const API_BASE: &str = "https://www.googleapis.com/blogger/v3";

/// Thread-safe façade over the Blogger API v3 + local cache + OAuth tokens.
#[derive(Clone)]
pub struct BloggerService {
    http: reqwest::Client,
    cache: Cache,
    config: Arc<Mutex<OAuthConfig>>,
    tokens: Arc<Mutex<Option<TokenPair>>>,
    config_path: PathBuf,
    tokens_path: PathBuf,
    appdata_dir: PathBuf,
}

impl BloggerService {
    pub fn new(appdata_dir: &std::path::Path) -> Self {
        let _ = std::fs::create_dir_all(appdata_dir);
        let cache = Cache::open(&appdata_dir.join("blogger.db")).expect("open cache");
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(40))
            .build()
            .expect("reqwest client");

        let service = Self {
            http,
            cache,
            config: Arc::new(Mutex::new(OAuthConfig::default())),
            tokens: Arc::new(Mutex::new(None)),
            config_path: appdata_dir.join("oauth.json"),
            tokens_path: appdata_dir.join("tokens.json"),
            appdata_dir: appdata_dir.to_path_buf(),
        };
        service.load_persistence();
        service
    }

    pub fn appdata_dir(&self) -> &std::path::Path {
        &self.appdata_dir
    }

    fn load_persistence(&self) {
        if let Ok(data) = std::fs::read_to_string(&self.config_path) {
            if let Ok(cfg) = serde_json::from_str::<OAuthConfig>(&data) {
                let c = self.config.clone();
                tokio::spawn(async move { *c.lock().await = cfg; });
            }
        }
        if let Ok(data) = std::fs::read_to_string(&self.tokens_path) {
            if let Ok(tok) = serde_json::from_str::<TokenPair>(&data) {
                let t = self.tokens.clone();
                tokio::spawn(async move { *t.lock().await = Some(tok); });
            }
        }
    }

    fn persist_config(&self, cfg: &OAuthConfig) {
        if let Ok(data) = serde_json::to_string_pretty(cfg) {
            let _ = std::fs::write(&self.config_path, data);
        }
    }

    fn persist_tokens(&self, tok: &TokenPair) {
        if let Ok(data) = serde_json::to_string_pretty(tok) {
            let _ = std::fs::write(&self.tokens_path, data);
        }
    }

    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    // ---------------- OAuth ----------------

    pub async fn oauth_config(&self) -> OAuthConfig {
        self.config.lock().await.clone()
    }

    pub async fn set_oauth_config(&self, cfg: OAuthConfig) {
        *self.config.lock().await = cfg.clone();
        self.persist_config(&cfg);
    }

    pub async fn oauth_status(&self) -> OAuthStatus {
        let cfg = self.config.lock().await.clone();
        let tok = self.tokens.lock().await.clone();
        OAuthStatus {
            configured: cfg.is_valid(),
            has_tokens: tok.is_some(),
            expired: tok.as_ref().map(|t| t.is_expired()).unwrap_or(false),
            has_refresh: tok.as_ref().and_then(|t| t.refresh_token.clone()),
        }
    }

    pub async fn complete_oauth(&self, code: &str) -> Result<()> {
        let cfg = self.config.lock().await.clone();
        if !cfg.is_valid() {
            return Err(Error::oauth("oauth client not configured"));
        }
        let tok = exchange_code(&self.http, &cfg, code).await?;
        *self.tokens.lock().await = Some(tok.clone());
        self.persist_tokens(&tok);
        Ok(())
    }

    pub async fn clear_tokens(&self) {
        *self.tokens.lock().await = None;
        let _ = std::fs::remove_file(&self.tokens_path);
    }

    async fn ensure_token(&self) -> Result<String> {
        let cfg = self.config.lock().await.clone();
        let token = {
            let t = self.tokens.lock().await;
            match t.clone() {
                Some(tok) if tok.is_expired() => {
                    let refresh = tok.refresh_token.clone();
                    if let Some(rt) = refresh {
                        let new = refresh_token(&self.http, &cfg, &rt).await?;
                        let mut t = self.tokens.lock().await;
                        *t = Some(new.clone());
                        self.persist_tokens(&new);
                        new.access_token
                    } else {
                        return Err(Error::oauth("access token expired, re-login required"));
                    }
                }
                Some(tok) => tok.access_token,
                None => return Err(Error::oauth("not authenticated yet")),
            }
        };
        Ok(token)
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<Value> {
        for attempt in 0..3 {
            let token = self.ensure_token().await?;
            let url = format!("{API_BASE}{path}");
            let mut req = self
                .http
                .request(reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET), &url)
                .bearer_auth(&token);
            if !query.is_empty() {
                req = req.query(query);
            }
            if let Some(b) = &body {
                req = req.json(b);
            }
            let res = req.send().await?;
            let status = res.status();
            let text = res.text().await?;

            if status.as_u16() == 401 {
                // force a refresh then retry once
                self.refresh_for_retry().await?;
                continue;
            }
            if status.is_success() {
                if text.trim().is_empty() {
                    return Ok(Value::Null);
                }
                return serde_json::from_str(&text).map_err(Into::into);
            }
            if attempt < 2 && is_retriable(status.as_u16()) {
                tokio::time::sleep(Duration::from_millis(400 * (attempt as u64 + 1))).await;
                continue;
            }
            return Err(Error::Api { status: status.as_u16(), body: text });
        }
        Err(Error::Other("request failed unexpectedly".into()))
    }

    async fn refresh_for_retry(&self) -> Result<()> {
        let cfg = self.config.lock().await.clone();
        let refresh = {
            let t = self.tokens.lock().await;
            t.clone().and_then(|x| x.refresh_token.clone())
        };
        if let Some(rt) = refresh {
            let new = refresh_token(&self.http, &cfg, &rt).await?;
            *self.tokens.lock().await = Some(new.clone());
            self.persist_tokens(&new);
            Ok(())
        } else {
            Err(Error::oauth("access token rejected and no refresh token available"))
        }
    }

    /// Pull every label used by posts on Blogger into the local cache.

    // ---------------- Blogs ----------------

    pub async fn list_blogs(&self, refresh: bool) -> Result<Vec<Blog>> {
        let cached = self.cache.list_blogs().await?;
        if !refresh && !cached.is_empty() {
            return Ok(cached);
        }
        let v = self
            .request("GET", "/users/self/blogs", &[], None)
            .await?;
        let list: BlogsList = serde_json::from_value(v)?;
        for b in &list.items {
            self.cache.upsert_blog(b).await?;
        }
        Ok(list.items)
    }

    pub async fn get_blog(&self, blog_id: &str, refresh: bool) -> Result<Blog> {
        if !refresh {
            if let Some(b) = self.cache.get_blog(blog_id).await? {
                return Ok(b);
            }
        }
        let v = self
            .request("GET", &format!("/blogs/{blog_id}"), &[], None)
            .await?;
        let blog: Blog = serde_json::from_value(v)?;
        self.cache.upsert_blog(&blog).await?;
        Ok(blog)
    }

    // ---------------- Posts ----------------

    pub async fn list_posts(&self, blog_id: &str, refresh: bool) -> Result<Vec<Post>> {
        if !refresh {
            let cached = self.cache.list_posts(blog_id).await?;
            if !cached.is_empty() {
                return Ok(cached);
            }
        }
        let mut all: Vec<Post> = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let mut query = vec![
                ("status", "DRAFT,LIVE,SCHEDULED"),
                ("maxResults", "100"),
                ("fetchBodies", "true"),
            ];
            let mut extra: Vec<(&str, &str)> = Vec::new();
            if let Some(pt) = &page_token {
                extra.push(("pageToken", pt));
            }
            query.append(&mut extra);
            let v = self
                .request(
                    "GET",
                    &format!("/blogs/{blog_id}/posts"),
                    &query,
                    None,
                )
                .await?;
            let list: PostsList = serde_json::from_value(v)?;
            for p in &list.items {
                self.cache.upsert_post(blog_id, p).await?;
            }
            all.extend(list.items);
            match list.next_page_token {
                Some(t) => page_token = Some(t),
                None => break,
            }
        }
        Ok(all)
    }

    pub async fn get_post(&self, blog_id: &str, post_id: &str, refresh: bool) -> Result<Post> {
        if !refresh {
            if let Some(p) = self.cache.get_post(blog_id, post_id).await? {
                return Ok(p);
            }
        }
        let v = self
            .request("GET", &format!("/blogs/{blog_id}/posts/{post_id}"), &[], None)
            .await?;
        let post: Post = serde_json::from_value(v)?;
        self.cache.upsert_post(blog_id, &post).await?;
        Ok(post)
    }

    /// Create a new post. `is_draft` controls whether it is immediately LIVE.
    pub async fn create_post(
        &self,
        blog_id: &str,
        input: &PostInput,
        is_draft: bool,
    ) -> Result<Post> {
        let body = json!({
            "title": input.title,
            "content": input.content,
            "labels": input.labels,
        });
        let query = [("isDraft", if is_draft { "true" } else { "false" })];
        let v = self
            .request("POST", &format!("/blogs/{blog_id}/posts"), &query, Some(body))
            .await?;
        let post: Post = serde_json::from_value(v)?;
        self.cache.upsert_post(blog_id, &post).await?;
        self.cache.merge_blogger_labels(blog_id, &post.labels).await?;
        Ok(post)
    }

    pub async fn update_post(
        &self,
        blog_id: &str,
        post_id: &str,
        input: &PostInput,
    ) -> Result<Post> {
        let body = json!({
            "title": input.title,
            "content": input.content,
            "labels": input.labels,
        });
        let v = self
            .request(
                "PATCH",
                &format!("/blogs/{blog_id}/posts/{post_id}"),
                &[],
                Some(body),
            )
            .await?;
        let post: Post = serde_json::from_value(v)?;
        self.cache.upsert_post(blog_id, &post).await?;
        self.cache.merge_blogger_labels(blog_id, &post.labels).await?;
        Ok(post)
    }

    pub async fn publish_post(&self, blog_id: &str, post_id: &str) -> Result<Post> {
        let v = self
            .request(
                "POST",
                &format!("/blogs/{blog_id}/posts/{post_id}/publish"),
                &[("isDraft", "true")],
                None,
            )
            .await?;
        let post: Post = serde_json::from_value(v)?;
        self.cache.upsert_post(blog_id, &post).await?;
        Ok(post)
    }

    pub async fn revert_post(&self, blog_id: &str, post_id: &str) -> Result<Post> {
        let v = self
            .request(
                "POST",
                &format!("/blogs/{blog_id}/posts/{post_id}/revert"),
                &[],
                None,
            )
            .await?;
        let post: Post = serde_json::from_value(v)?;
        self.cache.upsert_post(blog_id, &post).await?;
        Ok(post)
    }

    pub async fn delete_post(&self, blog_id: &str, post_id: &str) -> Result<()> {
        self.request(
            "DELETE",
            &format!("/blogs/{blog_id}/posts/{post_id}"),
            &[],
            None,
        )
        .await?;
        self.cache.delete_post(blog_id, post_id).await?;
        Ok(())
    }

    // ---------------- Labels / tags ----------------

    /// Pull every label used by posts on Blogger into the local cache.
    pub async fn sync_labels(&self, blog_id: &str) -> Result<Vec<String>> {
        let mut all: Vec<String> = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let mut query = vec![
                ("status", "DRAFT,LIVE,SCHEDULED"),
                ("maxResults", "500"),
                ("fetchBodies", "false"),
            ];
            if let Some(pt) = &page_token {
                query.push(("pageToken", pt));
            }
            let v = self
                .request("GET", &format!("/blogs/{blog_id}/posts"), &query, None)
                .await?;
            let list: PostsList = serde_json::from_value(v)?;
            for p in &list.items {
                for label in &p.labels {
                    if !all.contains(label) {
                        all.push(label.clone());
                    }
                }
                // keep drafts' labels discoverable.
                self.cache.merge_blogger_labels(blog_id, &p.labels).await?;
            }
            match list.next_page_token {
                Some(t) => page_token = Some(t),
                None => break,
            }
        }
        Ok(all)
    }

    /// Cached tags. When `refresh` is true a full Blogger sync runs first.
    pub async fn tags(&self, blog_id: &str, refresh: bool) -> Result<Vec<String>> {
        if refresh {
            let _ = self.sync_labels(blog_id).await;
        }
        self.cache.list_labels(blog_id).await
    }

    /// Add a tag locally; kept even if not yet present on Blogger. "Tự append".
    pub async fn add_tag(&self, blog_id: &str, name: &str) -> Result<Vec<String>> {
        for part in name.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()) {
            self.cache.add_local_label(blog_id, part).await?;
        }
        self.cache.list_labels(blog_id).await
    }

    pub async fn remove_tag(&self, blog_id: &str, name: &str) -> Result<()> {
        self.cache.remove_label(blog_id, name).await
    }

    }

/// Snapshot describing current OAuth state (used by settings UI).
#[derive(Debug, Clone, serde::Serialize)]
pub struct OAuthStatus {
    pub configured: bool,
    pub has_tokens: bool,
    pub expired: bool,
    pub has_refresh: Option<String>,
}

fn is_retriable(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}