//! Blank Overview page; no engine operations are connected.
use adw::gtk;

pub(in crate::ui::developer) fn build() -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, 0)
}
