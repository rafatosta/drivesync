//! First-run presentation, reusing the shared connection action.
mod page;
use adw::prelude::*;
/// Creates welcome without loading metadata or initiating consent.
pub(crate) fn build() -> adw::ToolbarView {
    let view = page::build();
    view.connect.set_action_name(Some("connection.open"));
    view.toolbar
}
