// Shared by build.rs and the runtime: identical validation for every source.
use serde::{Deserialize, Serialize};

/// Public Desktop client metadata. Not a user credential; never log its contents.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthConfig {
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
}
impl OAuthConfig {
    pub fn validate(self) -> Result<Self, &'static str> {
        let suffix = ".apps.googleusercontent.com";
        let prefix = self
            .client_id
            .strip_suffix(suffix)
            .ok_or("invalid client ID")?;
        if prefix.is_empty()
            || !prefix
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || self
                .client_secret
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.trim() != s || s.chars().any(char::is_control))
        {
            return Err("invalid Desktop client metadata");
        }
        Ok(self)
    }
    pub fn parse(data: &str) -> Result<Self, &'static str> {
        if data.len() > 64 * 1024 {
            return Err("client configuration too large");
        }
        // Downloaded Google metadata has additional fields, but only the two
        // public Desktop fields are copied into the binary. Never embed input JSON.
        let value: serde_json::Value =
            serde_json::from_str(data).map_err(|_| "invalid client JSON")?;
        let config = if let Some(installed) = value.get("installed") {
            if value.get("web").is_some() {
                return Err("expected Desktop client");
            }
            Self {
                client_id: installed
                    .get("client_id")
                    .and_then(|v| v.as_str())
                    .ok_or("missing client ID")?
                    .into(),
                client_secret: match installed.get("client_secret") {
                    None | Some(serde_json::Value::Null) => None,
                    Some(v) => Some(v.as_str().ok_or("invalid client secret")?.into()),
                },
            }
        } else {
            serde_json::from_value(value).map_err(|_| "expected Desktop client metadata")?
        };
        config.validate()
    }
    // Explicit overrides fail closed; a broken override must not switch identity.
    pub fn select(
        id: Option<String>,
        secret: Option<String>,
        file: Option<&str>,
        bundled: &str,
    ) -> Result<Self, &'static str> {
        if let Some(client_id) = id {
            return Self {
                client_id,
                client_secret: secret,
            }
            .validate();
        }
        if secret.is_some() {
            return Err("client secret override requires client ID");
        }
        Self::parse(file.unwrap_or(bundled))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const CLIENT: &str = r#"{"client_id":"123-test.apps.googleusercontent.com","client_secret":"public-compatibility"}"#;
    #[test]
    fn selection_and_fail_closed() {
        assert!(OAuthConfig::select(None, None, None, CLIENT).is_ok());
        assert!(OAuthConfig::select(None, None, Some(CLIENT), "").is_ok());
        let c = OAuthConfig::select(
            Some("override.apps.googleusercontent.com".into()),
            None,
            Some("broken"),
            CLIENT,
        )
        .unwrap();
        assert!(c.client_secret.is_none());
        assert_eq!(c.client_id, "override.apps.googleusercontent.com");
        assert!(OAuthConfig::select(None, None, Some("broken"), CLIENT).is_err());
        assert!(OAuthConfig::select(Some("".into()), None, None, CLIENT).is_err());
        assert!(OAuthConfig::select(None, Some("orphan".into()), None, CLIENT).is_err());
        assert!(OAuthConfig::select(None, None, None, "").is_err());
    }
    #[test]
    fn desktop_only_and_sanitized() {
        let c = OAuthConfig::parse(r#"{"installed":{"client_id":"123.apps.googleusercontent.com","client_secret":"compat","auth_uri":"untrusted","refresh_token":"must-not-embed"}}"#).unwrap();
        let encoded = serde_json::to_string(&c).unwrap();
        assert!(!encoded.contains("must-not-embed"));
        assert!(!encoded.contains("untrusted"));
        for data in [
            r#"{"web":{"client_id":"123.apps.googleusercontent.com"}}"#,
            r#"{"client_id":".apps.googleusercontent.com"}"#,
            r#"{"client_id":" bad.apps.googleusercontent.com"}"#,
            r#"{"client_id":"123.apps.googleusercontent.com","access_token":"x"}"#,
            r#"{"client_id":"123.apps.googleusercontent.com","client_secret":""}"#,
        ] {
            assert!(OAuthConfig::parse(data).is_err());
        }
        assert!(OAuthConfig::parse(&" ".repeat(65537)).is_err());
    }
}
