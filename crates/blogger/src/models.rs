use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Blog {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub published: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, rename = "posts")]
    pub posts: Option<BlogCounts>,
    #[serde(default, rename = "pages")]
    pub pages: Option<BlogCounts>,
    #[serde(default)]
    pub locale: Option<BlogLocale>,
    #[serde(default)]
    pub custom_meta_data: Option<String>,
}

/// Extra settings Blogger exposes through `blogs.get`: drafts default etc.
impl Blog {
    pub fn posts_default_draft(&self) -> bool {
        self.posts.as_ref().map(|p| p.is_draft).unwrap_or(false)
    }
    pub fn pages_default_draft(&self) -> bool {
        self.pages.as_ref().map(|p| p.is_draft).unwrap_or(false)
    }
    pub fn post_count(&self) -> i64 {
        self.posts.as_ref().map(|p| p.total_items).unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlogCounts {
    #[serde(default)]
    pub total_items: i64,
    #[serde(default)]
    pub is_draft: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlogLocale {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub variant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PostsList {
    #[serde(default)]
    pub items: Vec<Post>,
    #[serde(default)]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlogsList {
    #[serde(default)]
    pub items: Vec<Blog>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Post {
    pub id: String,
    #[serde(default)]
    pub blog: Option<BlogRef>,
    #[serde(default)]
    pub published: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub self_link: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub author: Option<Author>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_status() -> String {
    "LIVE".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlogRef {
    pub id: String,
    pub name: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Author {
    pub id: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub url: Option<String>,
    pub image: Option<AuthorImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthorImage {
    pub url: Option<String>,
}

/// Payload used when creating / updating a post through the API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PostInput {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

impl Post {
    pub fn is_draft(&self) -> bool {
        self.status == "DRAFT"
    }
}