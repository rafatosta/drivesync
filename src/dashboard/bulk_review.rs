//! Explicit, cancellable review; scheduler dispatch owns the one-attempt approval.
use adw::{gtk, prelude::*};
use drivesync_engine::{BulkImpact, EngineError, FolderConfiguration};
use std::time::Duration;

pub(super) fn button(job: crate::automatic::Controller) -> gtk::Button {
    let button = gtk::Button::with_label("Review bulk changes…");
    button.set_widget_name("review-bulk-changes");
    button.set_visible(false);
    let clicked = job.clone();
    button.connect_clicked(move |button| {
        let Some(Err(EngineError::BulkReviewRequired(review))) = clicked.snapshot().result else {
            return;
        };
        let Some(folder) = clicked.folder() else {
            return;
        };
        let parent = button
            .root()
            .and_then(|root| root.downcast::<gtk::Window>().ok());
        let dialog = adw::MessageDialog::new(parent.as_ref(), Some("Review bulk changes"), None);
        let details = gtk::Label::builder()
            .label(description(review.impacts(), &folder))
            .use_markup(false)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .selectable(true)
            .xalign(0.0)
            .build();
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(160)
            .max_content_height(360)
            .propagate_natural_height(true)
            .child(&details)
            .build();
        dialog.set_extra_child(Some(&scroll));
        dialog.add_responses(&[
            ("cancel", "Keep paused"),
            ("continue", "Continue supported transfers"),
        ]);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let confirmed = clicked.clone();
        dialog.connect_response(Some("continue"), move |_, _| {
            confirmed.request_reviewed(review.clone())
        });
        dialog.present();
    });
    let weak = button.downgrade();
    gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
        let Some(button) = weak.upgrade() else {
            return gtk::glib::ControlFlow::Break;
        };
        let snapshot = job.snapshot();
        button.set_visible(
            !job.running()
                && matches!(
                    snapshot.result,
                    Some(Err(EngineError::BulkReviewRequired(_)))
                ),
        );
        gtk::glib::ControlFlow::Continue
    });
    button
}

/// Show quantities and scope without rendering provider-controlled markup.
fn description(impacts: &[BulkImpact], folder: &FolderConfiguration) -> String {
    let mut body = format!("Google Drive folder: {}\nLocal folder: {}\n\nPotentially destructive changes were observed:\n", folder.remote.name, folder.local_display.display());
    for impact in impacts.iter().take(20) {
        let name = if impact.folder.as_os_str().is_empty() {
            "Selected root".into()
        } else {
            impact.folder.to_string_lossy()
        };
        let percent = impact.affected as f64 * 100.0 / impact.population.max(1) as f64;
        body.push_str(&format!(
            "\n{name}: {} of {} confirmed items ({percent:.1}%)",
            impact.affected, impact.population
        ));
    }
    if impacts.len() > 20 {
        body.push_str(&format!(
            "\n{} additional affected folders",
            impacts.len() - 20
        ));
    }
    body.push_str("\n\nContinue only if you recognize these changes. DriveSync will check the observations again and stop if they changed. This confirmation applies to one attempt. Existing per-file protections remain active; deletion and remote overwrite stay unavailable. Missing local copies may be downloaded again. Keeping paused leaves existing data in place.");
    body
}
