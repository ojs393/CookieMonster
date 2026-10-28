use serde::{Deserialize, Serialize};

pub const DEFAULT_MODE: Mode = Mode::Observe;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Observe,
    Moderate,
    CookieMonster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CookieAction {
    Set,
    Send,
    Js,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CookieParty {
    First,
    Third,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CookieBucket {
    Necessary,
    Preferences,
    Analytics,
    Advertising,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionKind {
    Allow,
    Deny,
    Strip,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CookieAttributes {
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
    pub max_age: Option<i64>,
    pub domain: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CookieEvent {
    pub id: String,
    pub ts_ms: u64,
    pub action: CookieAction,
    pub party: CookieParty,
    pub scheme: String,
    pub host: String,
    pub name: String,
    pub value_redacted: bool,
    pub attributes: CookieAttributes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub kind: DecisionKind,
    pub reason: String,
}
