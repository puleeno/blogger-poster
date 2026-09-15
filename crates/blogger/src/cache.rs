use std::path::Path;
use std::sync::Arc;

use rusqlite::{Connection, params};
use tokio::sync::Mutex;

use crate::error::Result;
use crate::models::{Blog, Post};

#[derive(Clone)]
pub struct Cache {
    conn: Arc<Mutex<Connection>>,
}

impl Cache {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;

            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS blogs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                url TEXT NOT NULL DEFAULT '',
                data TEXT NOT NULL,
                fetched_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS posts (
                blog_id TEXT NOT NULL,
                id TEXT NOT NULL,
                data TEXT NOT NULL,
                updated TEXT,
                fetched_at TEXT NOT NULL,
                PRIMARY KEY (blog_id, id)
            );

            CREATE TABLE IF NOT EXISTS labels (
                blog_id TEXT NOT NULL,
                name TEXT NOT NULL,
                source TEXT NOT NULL DEFAULT 'local',
                synced INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (blog_id, name)
            );
            "#,
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn memory() -> Result<Self> {
        Self::open(Path::new(":memory:"))
    }

    // ---------------- meta ----------------

    pub async fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub async fn get_meta(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare("SELECT value FROM meta WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    // ---------------- blogs ----------------

    pub async fn upsert_blog(&self, blog: &Blog) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO blogs (id, name, url, data, fetched_at) VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, url = excluded.url, data = excluded.data, fetched_at = datetime('now')",
            params![blog.id, blog.name, blog.url, serde_json::to_string(blog)?],
        )?;
        Ok(())
    }

    pub async fn list_blogs(&self) -> Result<Vec<Blog>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare("SELECT data FROM blogs ORDER BY name")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows
            .filter_map(|r| r.ok())
            .filter_map(|d| serde_json::from_str(&d).ok())
            .collect())
    }

    pub async fn get_blog(&self, id: &str) -> Result<Option<Blog>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare("SELECT data FROM blogs WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(serde_json::from_str(&row.get::<_, String>(0)?)?)),
            None => Ok(None),
        }
    }

    // ---------------- posts ----------------

    pub async fn upsert_post(&self, blog_id: &str, post: &Post) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO posts (blog_id, id, data, updated, fetched_at) VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(blog_id, id) DO UPDATE SET data = excluded.data, updated = excluded.updated, fetched_at = datetime('now')",
            params![blog_id, post.id, serde_json::to_string(post)?, post.updated.clone().unwrap_or_default()],
        )?;
        Ok(())
    }

    pub async fn list_posts(&self, blog_id: &str) -> Result<Vec<Post>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare("SELECT data FROM posts WHERE blog_id = ?1 ORDER BY rowid DESC")?;
        let rows = stmt.query_map(params![blog_id], |row| row.get::<_, String>(0))?;
        Ok(rows
            .filter_map(|r| r.ok())
            .filter_map(|d| serde_json::from_str(&d).ok())
            .collect())
    }

    pub async fn get_post(&self, blog_id: &str, id: &str) -> Result<Option<Post>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare("SELECT data FROM posts WHERE blog_id = ?1 AND id = ?2")?;
        let mut rows = stmt.query(params![blog_id, id])?;
        match rows.next()? {
            Some(row) => Ok(Some(serde_json::from_str(&row.get::<_, String>(0)?)?)),
            None => Ok(None),
        }
    }

    pub async fn delete_post(&self, blog_id: &str, id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM posts WHERE blog_id = ?1 AND id = ?2", params![blog_id, id])?;
        Ok(())
    }

    // ---------------- labels / tags ----------------

    /// All tags currently known for a blog (synced from Blogger + locally added).
    pub async fn list_labels(&self, blog_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().await;
        let mut stmt =
            conn.prepare("SELECT name FROM labels WHERE blog_id = ?1 ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map(params![blog_id], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Merge a batch of labels pulled from Blogger. Existing local-only labels are kept.
    pub async fn merge_blogger_labels(&self, blog_id: &str, names: &[String]) -> Result<()> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "INSERT INTO labels (blog_id, name, source, synced) VALUES (?1, ?2, 'blogger', 1)
             ON CONFLICT(blog_id, name) DO UPDATE SET synced = 1, source = CASE WHEN labels.source = 'local' THEN 'local' ELSE 'blogger' END",
        )?;
        for name in names {
            if !name.trim().is_empty() {
                stmt.execute(params![blog_id, name.trim()])?;
            }
        }
        Ok(())
    }

    /// Add a locally-defined tag. It stays until it appears on Blogger (kept so nothing is lost).
    pub async fn add_local_label(&self, blog_id: &str, name: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR IGNORE INTO labels (blog_id, name, source, synced) VALUES (?1, ?2, 'local', 0)",
            params![blog_id, name.trim()],
        )?;
        Ok(())
    }

    pub async fn remove_label(&self, blog_id: &str, name: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "DELETE FROM labels WHERE blog_id = ?1 AND name = ?2",
            params![blog_id, name],
        )?;
        Ok(())
    }

    pub async fn label_sources(&self, blog_id: &str) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().await;
        let mut stmt =
            conn.prepare("SELECT name, source FROM labels WHERE blog_id = ?1 ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map(params![blog_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}