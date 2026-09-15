use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub chrome_port: u16,
    pub chrome_target_id: Option<String>,
    pub editor_mode: String,
    pub oauth_port: u16,
    pub selected_blog_id: Option<String>,
    pub image_timeout_ms: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            chrome_port: 9222,
            chrome_target_id: None,
            editor_mode: "ide".into(),
            oauth_port: 8123,
            selected_blog_id: None,
            image_timeout_ms: 120_000,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    pub current: std::sync::RwLock<AppSettings>,
}

impl SettingsStore {
    pub fn new(appdata_dir: &PathBuf) -> Self {
        let path = appdata_dir.join("settings.json");
        let current = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            path,
            current: std::sync::RwLock::new(current),
        }
    }

    pub fn get(&self) -> AppSettings {
        self.current.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn update(&self, patch: serde_json::Value) -> AppSettings {
        let mut cur = self.get();
        if let Some(merged) = serde_json::to_value(&cur)
            .ok()
            .and_then(|mut base| {
                if let serde_json::Value::Object(m) = patch {
                    if let Some(b) = base.as_object_mut() {
                        for (k, v) in m {
                            b.insert(k, v);
                        }
                    }
                }
                serde_json::from_value(base).ok()
            })
        {
            cur = merged;
        }
        *self.current.write().unwrap() = cur.clone();
        if let Ok(data) = serde_json::to_string_pretty(&cur) {
            let _ = std::fs::write(&self.path, data);
        }
        cur
    }
}

pub fn app_data_dir() -> PathBuf {
    let base = std::env::var("BLOGGER_PANEL_DATA").ok().map(PathBuf::from).unwrap_or_else(|| {
        dirs_repl()
    });
    base.join("blogger-poster")
}

fn dirs_repl() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("Library").join("Application Support");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(apd) = std::env::var("APPDATA") {
            return PathBuf::from(apd);
        }
    }
    PathBuf::from(".")
}

pub fn scenarios_dir() -> PathBuf {
    std::env::var("BLOGGER_SCENARIOS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            if root.join("scripts/scenarios").exists() {
                root.join("scripts/scenarios")
            } else {
                root.join("scenarios")
            }
        })
}