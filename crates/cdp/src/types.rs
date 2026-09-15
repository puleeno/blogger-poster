use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Target {
    pub id: String,
    #[serde(rename = "type")]
    pub target_type: Option<String>,
    pub url: String,
    pub title: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub ws_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrowserInfo {
    #[serde(rename = "Browser")]
    pub browser: String,
    #[serde(rename = "Protocol-Version")]
    pub protocol: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub ws_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CdpEvent {
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct CdpResponse {
    pub id: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
}