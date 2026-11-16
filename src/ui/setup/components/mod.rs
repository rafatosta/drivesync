//! Setup-only components. Consumers use data and events, never internal widgets.
mod folder_choice;
mod provider_choice;
mod summary;
pub(super) use folder_choice::FolderChoice;
pub(super) use provider_choice::ProviderChoice;
pub(super) use summary::Summary;
