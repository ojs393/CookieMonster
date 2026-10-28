use serde::{Deserialize, Serialize};

use crate::types::CookieBucket;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowlistEntry {
    pub host: String,
    pub allowed_buckets: Option<Vec<CookieBucket>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Allowlist {
    pub entries: Vec<AllowlistEntry>,
}

impl Allowlist {
    pub fn new(entries: Vec<AllowlistEntry>) -> Self {
        Self { entries }
    }

    pub fn decision_for(&self, host: &str, bucket: CookieBucket) -> Option<bool> {
        let entry = self
            .entries
            .iter()
            .find(|entry| host_matches(&entry.host, host))?;
        Some(match &entry.allowed_buckets {
            None => true,
            Some(buckets) => buckets.contains(&bucket),
        })
    }
}

pub fn host_matches(pattern: &str, host: &str) -> bool {
    let pattern = pattern.trim().trim_start_matches('.').to_lowercase();
    let host = host.trim().trim_start_matches('.').to_lowercase();
    if pattern.is_empty() || host.is_empty() {
        return false;
    }
    host == pattern || host.ends_with(&format!(".{pattern}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subdomain_matches_apex() {
        assert!(host_matches("github.com", "github.com"));
        assert!(host_matches("github.com", "www.github.com"));
        assert!(!host_matches("github.com", "evilgithub.com"));
    }
}
