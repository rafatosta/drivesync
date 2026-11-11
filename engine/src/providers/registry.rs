//! Public catalog of provider implementations shipped by this engine.
//!
//! This catalog describes capabilities; it does not create sessions or expose
//! credentials. Provider-specific operations remain in their owning modules.
use crate::ProviderId;

/// Supported authorization entry point for a registered provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMethod {
    /// Browser authorization using the engine's Google connection API.
    Google,
}

/// Presentation metadata and the available connection capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredProvider {
    pub id: ProviderId,
    pub name: String,
    /// Bundled SVG source, independent of filesystem paths and UI toolkits.
    pub icon_svg: &'static str,
    pub connection_method: ConnectionMethod,
}

/// Returns the registered implementations, without network or credential I/O.
/// The returned vector is owned by the caller and cannot mutate the registry.
pub fn list_providers() -> Vec<RegisteredProvider> {
    vec![RegisteredProvider {
        id: ProviderId("google".into()),
        name: "Google Drive".into(),
        icon_svg: include_str!("google/assets/drive.svg"),
        connection_method: ConnectionMethod::Google,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_has_unique_identifiers_and_complete_metadata() {
        let providers = list_providers();
        let mut ids = std::collections::HashSet::new();
        for provider in &providers {
            assert!(!provider.id.0.trim().is_empty());
            assert!(ids.insert(&provider.id));
            assert!(!provider.name.trim().is_empty());
            assert!(provider.icon_svg.contains("<svg"));
            assert!(provider.icon_svg.contains("http://www.w3.org/2000/svg"));
        }
        let mut copy = list_providers();
        copy.clear();
        assert_eq!(list_providers(), providers);
    }
}
