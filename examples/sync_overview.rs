//! Isolated interactive preview. No application services or engine are initialized.
#[path = "../src/ui/components/sync_overview/mod.rs"]
mod sync_overview;
use adw::prelude::*;
use std::{rc::Rc, time::SystemTime};
use sync_overview::{SyncOverview, SyncOverviewState};

fn main() -> gtk::glib::ExitCode {
    // Initialize locale before GTK starts threads. The preview accepts a local catalog.
    unsafe {
        gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
    }
    gettextrs::bindtextdomain(
        "drivesync",
        std::env::var("DRIVESYNC_PREVIEW_LOCALEDIR").unwrap_or_else(|_| "/app/share/locale".into()),
    )
    .unwrap();
    gettextrs::bind_textdomain_codeset("drivesync", "UTF-8").unwrap();
    gettextrs::textdomain("drivesync").unwrap();
    let app = adw::Application::builder()
        .application_id("com.rtosta.DriveSync.SyncOverviewPreview")
        .build();
    app.connect_activate(|app| {
        let view = SyncOverview::new();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_start(24);
        content.set_margin_end(24);
        content.set_margin_bottom(24);
        let state =
            gtk::DropDown::from_strings(&["Verificando", "Sincronizando", "Concluído", "Atenção"]);
        content.append(&state);
        content.append(view.widget());
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let fields: Vec<_> = [
            ("Itens verificados", 2480.0),
            ("Itens sincronizados", 1248.0),
            ("Total de itens", 1560.0),
            ("Itens restantes", 312.0),
            ("Bytes restantes", 840_000_000.0),
            ("Progresso (%)", 80.0),
            ("Pendências", 3.0),
        ]
        .into_iter()
        .map(|(name, initial)| {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let known = gtk::CheckButton::with_label(name);
            known.set_active(true);
            known.set_hexpand(true);
            let value = gtk::SpinButton::with_range(
                0.0,
                if name == "Progresso (%)" {
                    100.0
                } else {
                    1_000_000_000.0
                },
                1.0,
            );
            value.set_value(initial);
            row.append(&known);
            row.append(&value);
            controls.append(&row);
            (known, value)
        })
        .collect();
        fields[1].0.set_sensitive(false);
        let last = gtk::CheckButton::with_label("Informar última sincronização (agora)");
        last.set_active(true);
        controls.append(&last);
        content.append(&controls);
        let dark = gtk::CheckButton::with_label("Tema escuro");
        dark.connect_toggled(|button| {
            adw::StyleManager::default().set_color_scheme(if button.is_active() {
                adw::ColorScheme::ForceDark
            } else {
                adw::ColorScheme::ForceLight
            })
        });
        content.append(&dark);
        let response = gtk::Label::new(Some(
            "Prévia isolada • valores fornecidos pelo demonstrador",
        ));
        response.set_wrap(true);
        content.append(&response);
        view.connect_attention_requested(move || {
            response.set_label("Sinal recebido: atenção solicitada")
        });
        let update: Rc<dyn Fn()> = Rc::new({
            let view = view.clone();
            let fields = fields.clone();
            let state = state.clone();
            fields[1].0.set_sensitive(false);
            let last = last.clone();
            move || {
                let value = |i: usize| fields[i].0.is_active().then(|| fields[i].1.value() as u64);
                view.set_state(match state.selected() {
                    0 => SyncOverviewState::Checking {
                        checked_items: value(0),
                    },
                    1 => SyncOverviewState::Syncing {
                        completed_items: fields[1].1.value() as u64,
                        total_items: value(2),
                        remaining_items: value(3),
                        remaining_bytes: value(4),
                        progress: value(5).map(|v| v as f64 / 100.0),
                    },
                    2 => SyncOverviewState::Completed {
                        last_sync: last.is_active().then(SystemTime::now),
                    },
                    _ => SyncOverviewState::Attention {
                        pending_items: value(6),
                    },
                });
            }
        });
        // Weak captures keep the controls from retaining their own callback graph.
        for (known, value) in &fields {
            let weak = Rc::downgrade(&update);
            known.connect_toggled(move |_| {
                if let Some(f) = weak.upgrade() {
                    f();
                }
            });
            let weak = Rc::downgrade(&update);
            value.connect_value_changed(move |_| {
                if let Some(f) = weak.upgrade() {
                    f();
                }
            });
        }
        let weak = Rc::downgrade(&update);
        state.connect_selected_notify(move |_| {
            if let Some(f) = weak.upgrade() {
                f();
            }
        });
        let weak = Rc::downgrade(&update);
        last.connect_toggled(move |_| {
            if let Some(f) = weak.upgrade() {
                f();
            }
        });
        update();
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&content)
            .build();
        toolbar.set_content(Some(&scroll));
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("SyncOverview — prévia isolada")
            .default_width(540)
            .default_height(780)
            .content(&toolbar)
            .build();
        window.connect_destroy(move |_| {
            let _ = &update;
        });
        window.present();
    });
    app.run()
}
