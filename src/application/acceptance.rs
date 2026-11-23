//! Explicit Devel-only installed-package observation. Normal launches have no diagnostic side effects.
use super::Context;
use adw::{
    gtk::{self, glib},
    prelude::*,
};
fn labels(widget: &gtk::Widget, out: &mut Vec<serde_json::Value>) {
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        out.push(serde_json::json!({"widget":widget.widget_name().to_string(),"text":label.text().to_string(),"visible":widget.is_mapped()}));
    }
    let mut child = widget.first_child();
    while let Some(node) = child {
        labels(&node, out);
        child = node.next_sibling();
    }
}
pub(super) fn install(app: &adw::Application, window: &adw::ApplicationWindow, context: &Context) {
    if !super::APP_ID.ends_with(".Devel") {
        return;
    }
    let Some(output) =
        std::env::var_os("DRIVESYNC_STAGE11_GUI_REPORT").map(std::path::PathBuf::from)
    else {
        return;
    };
    if output != glib::user_data_dir().join("drivesync/stage11-acceptance") {
        return;
    }
    let app = app.clone();
    let window = window.clone();
    let context = context.clone();
    glib::timeout_add_local_once(std::time::Duration::from_secs(15), move || {
        glib::spawn_future_local(async move {
            let engine = context
                .engine
                .borrow()
                .clone()
                .expect("real Engine restoration must finish");
            let available = engine
                .list_accounts(drivesync_engine::ListAccountsRequest { provider: None })
                .await
                .unwrap();
            assert_eq!(
                available.accounts.len(),
                1,
                "installed acceptance is limited to the isolated test account"
            );
            let account = &available.accounts[0];
            assert_eq!(account.email.as_deref(), Some("rtosta.coc@gmail.com"));
            let mut audit = None;
            for _ in 0..60 {
                if let Ok(value) = engine.synchronization_audit(account.id.clone()).await {
                    let complete = value.initial_complete;
                    audit = Some(value);
                    if complete {
                        break;
                    }
                }
                glib::timeout_future(std::time::Duration::from_secs(1)).await;
            }
            assert!(window.is_visible() && window.is_realized());
            std::fs::create_dir_all(&output).unwrap();
            // Native AdwDialogHost allocations can lag an asynchronous page replacement.
            glib::timeout_future(std::time::Duration::from_millis(300)).await;
            let child = window.child().expect("installed window has real content");
            child.allocate(window.width(), window.height(), -1, None);
            let paintable = gtk::WidgetPaintable::new(Some(&window));
            let snapshot = gtk::Snapshot::new();
            paintable.snapshot(&snapshot, window.width() as f64, window.height() as f64);
            let node = snapshot.to_node().unwrap_or_else(|| {
                let snapshot = gtk::Snapshot::new();
                window.snapshot_child(&child, &snapshot);
                snapshot.to_node().expect("installed window renders")
            });
            let background = gtk::Snapshot::new();
            let shade = if adw::StyleManager::default().is_dark() {
                0.14
            } else {
                0.98
            };
            background.append_color(
                &gtk::gdk::RGBA::new(shade, shade, shade, 1.0),
                &gtk::graphene::Rect::new(0.0, 0.0, window.width() as f32, window.height() as f32),
            );
            background.append_node(&node);
            let node = background.to_node().unwrap();
            window
                .renderer()
                .unwrap()
                .render_texture(&node, None)
                .save_to_png(output.join("installed-window.png"))
                .unwrap();
            let mut texts = vec![];
            labels(window.upcast_ref(), &mut texts);
            let result = serde_json::json!({"application_id":super::APP_ID,"account":account.email,"visible":window.is_visible(),"width":window.width(),"height":window.height(),"initial_complete":audit.as_ref().is_some_and(|a|a.initial_complete),"audit":format!("{audit:?}"),"status":format!("{:?}",engine.synchronization_status(account.id.clone()).await.unwrap()),"labels":texts});
            std::fs::write(
                output.join("installed-report.json"),
                serde_json::to_vec_pretty(&result).unwrap(),
            )
            .unwrap();
            engine
                .stop_synchronization(account.id.clone())
                .await
                .unwrap();
            app.quit();
        });
    });
}
