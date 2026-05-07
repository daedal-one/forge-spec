use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use crate::model::registry::Redirect;

#[derive(Debug, Deserialize)]
struct RedirectsFile {
    redirect: Option<Vec<RedirectEntry>>,
}

#[derive(Debug, Deserialize)]
struct RedirectEntry {
    from: String,
    to: String,
}

/// Load redirects from a `_redirects.toml` file.
pub fn load_redirects(path: &Path) -> Result<Vec<Redirect>> {
    let content = std::fs::read_to_string(path)?;
    let file: RedirectsFile = toml::from_str(&content)?;
    Ok(file
        .redirect
        .unwrap_or_default()
        .into_iter()
        .map(|r| Redirect {
            from: r.from,
            to: r.to,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_redirects_toml() {
        let content = r#"
[[redirect]]
from = "REQ:auth/session-timeout"
to   = "REQ:auth/session-expiry"

[[redirect]]
from = "REQ:auth/session-expiry#timeout"
to   = "REQ:auth/session-expiry#timeout-policy"
"#;
        let file: RedirectsFile = toml::from_str(content).unwrap();
        let redirects = file.redirect.unwrap();
        assert_eq!(redirects.len(), 2);
        assert_eq!(redirects[0].from, "REQ:auth/session-timeout");
        assert_eq!(redirects[0].to, "REQ:auth/session-expiry");
    }
}
