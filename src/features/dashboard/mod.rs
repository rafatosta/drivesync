//! Dashboard page, account views, components and explicit confirmation dialogs.
mod account_view;
mod components;
mod dialogs;
mod page;
pub(crate) mod presentation;
mod synchronization_view;

pub(crate) use page::{build, build_selected};
#[cfg(test)]
pub(crate) use page::{verify_download_progress, verify_download_worker_failure};
