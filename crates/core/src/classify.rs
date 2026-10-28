use crate::types::{CookieBucket, CookieEvent};

pub fn classify(event: &CookieEvent) -> CookieBucket {
    let name = event.name.as_str();
    if matches_name(name, &["_ga", "_gid", "_gat"]) {
        CookieBucket::Analytics
    } else if matches_name(name, &["IDE", "_fbp", "fr", "_gcl_au"]) {
        CookieBucket::Advertising
    } else if matches_name(
        name,
        &["session", "sid", "csrf", "csrftoken", "__Host-session", "__Secure-session"],
    ) {
        CookieBucket::Necessary
    } else if matches_name(name, &["lang", "locale", "theme", "tz"]) {
        CookieBucket::Preferences
    } else {
        CookieBucket::Unknown
    }
}

fn matches_name(name: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| {
        name == *p || name.starts_with(&format!("{p}_")) || name.starts_with(&format!("{p}-"))
    })
}
