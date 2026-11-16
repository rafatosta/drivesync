//! Cached diagnostics from the application's shared synchronization session.
use adw::gtk;
pub(in crate::ui::developer) fn build(context: &crate::application::Context) -> gtk::Box {
    super::super::pipeline::build(context, false)
}
