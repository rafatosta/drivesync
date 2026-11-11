//! Values exchanged with the caller; identifiers are opaque to presentation.
/// Provider-assigned identifier, never parsed by the picker.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemoteFolderId(pub String);
/// A folder and its optional immediate parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFolder {
    pub id: RemoteFolderId,
    pub name: String,
    pub parent_id: Option<RemoteFolderId>,
}
/// Known path ordered from the configured root through the selected folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFolderLocation {
    pub folder: RemoteFolder,
    pub path: Vec<RemoteFolder>,
}
/// Caller-supplied identity and eligible navigation root.
#[derive(Debug, Clone)]
pub struct RemoteFolderPickerConfig {
    pub provider_name: String,
    /// SVG source supplied by the caller, rather than a theme icon name.
    pub provider_icon_svg: String,
    pub account_label: String,
    pub root: RemoteFolder,
}
/// Terminal response; emitted once per picker instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteFolderPickerResult {
    Selected(RemoteFolderLocation),
    Cancelled,
}
