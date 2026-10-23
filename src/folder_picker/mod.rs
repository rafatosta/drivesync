//! Drive folder browser and selection dialog.
//!
//! Builds the native dialog separately from the browsing state and pagination
//! logic. Folder metadata comes from the engine; no transfers occur here.
mod controller;
mod view;

pub use controller::show;

/// Static presentation fixture; it never contacts Drive or selects real data.
#[cfg(test)]
pub(crate) fn preview(parent: Option<&adw::gtk::Window>) -> adw::Window {
    use adw::prelude::*;
    let view = view::build(parent);
    view.up.set_sensitive(false);
    view.breadcrumbs.append(&view::current_location("My Drive"));
    view.status.set_visible(false);
    for name in [
        "Arquivos Backup",
        "Concursos e Seleções",
        "Estudos",
        "Livros",
        "Mestrado UEFS",
        "Meus Arquivos",
        "Plano de estudo",
        "Revisão Bibliográfica",
    ] {
        view.list.append(&view::folder_row(name));
    }
    view.choose.set_sensitive(true);
    view.all.set_sensitive(true);
    view.window
}

#[cfg(test)]
pub(crate) fn verify_selection() {
    controller::verify_selection();
}
