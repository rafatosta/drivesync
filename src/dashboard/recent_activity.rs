//! Last completed report and expandable observed path history. No timestamps or
//! transfer byte totals are invented when the engine does not provide them.
use adw::{gtk, prelude::*};
use drivesync_engine::DownloadReport;

/// Replaces the activity region from one account report. Historical observations
/// remain informational and never become resolution commands.
pub(super) fn present(container: &gtk::Box, report: &DownloadReport) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    let equivalent: Vec<_> = report
        .reconciliation
        .iter()
        .filter(|entry| {
            entry.kind == drivesync_engine::ChangeKind::Equivalent
                && !report
                    .issues
                    .iter()
                    .any(|issue| issue.relative_path == entry.relative_path)
        })
        .collect();
    container.set_visible(true);

    let group = adw::PreferencesGroup::builder()
        .title(gettextrs::gettext("Recent activity"))
        .build();
    let summary = crate::flow::row(
        &gettextrs::gettext("Latest completed check"),
        &format!(
            "{} downloaded · {} updated · {} uploaded · {} unchanged",
            report.downloaded, report.updated, report.uploaded, report.unchanged
        ),
        "document-open-recent-symbolic",
    );
    summary.set_subtitle_lines(0);
    group.add(&summary);
    let details = adw::ExpanderRow::builder()
        .title("View activity")
        .subtitle(if report.previous_paths.is_empty() {
            "Equivalent content was verified. No replacement was needed."
        } else {
            "Previous local copies are kept. No action is needed."
        })
        .use_markup(false)
        .build();
    details.set_subtitle_lines(0);
    for activity in report.previous_paths.iter().take(50) {
        let name = activity
            .previous_path
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Previous location".into());
        let subtitle = activity
            .current_path
            .as_deref()
            .map(|path| {
                format!(
                    "Now at {} on Google Drive. Previous local copies are kept.",
                    path.to_string_lossy()
                )
            })
            .unwrap_or_else(|| {
                "No longer listed in the selected Google Drive folder. Local copies are kept."
                    .into()
            });
        let row = adw::ActionRow::builder()
            .title(&name)
            .subtitle(&subtitle)
            .use_markup(false)
            .build();
        row.set_title_lines(0);
        row.set_subtitle_lines(0);
        row.set_tooltip_text(Some(&format!("{name}\n{subtitle}")));
        row.add_prefix(&gtk::Image::from_icon_name("document-open-recent-symbolic"));
        details.add_row(&row);
    }
    for entry in equivalent.iter().take(50) {
        let name = entry
            .relative_path
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Item".into());
        let row = adw::ActionRow::builder()
            .title(&name)
            .subtitle(entry.kind.description())
            .use_markup(false)
            .build();
        row.set_title_lines(0);
        row.set_subtitle_lines(0);
        details.add_row(&row);
    }
    if equivalent.len() > 50 {
        details.add_row(
            &adw::ActionRow::builder()
                .title(format!(
                    "{} additional equivalent files observed",
                    equivalent.len() - 50
                ))
                .build(),
        );
    }
    if report.previous_paths.len() > 50 {
        details.add_row(
            &adw::ActionRow::builder()
                .title(format!(
                    "{} additional previous paths recorded",
                    report.previous_paths.len() - 50
                ))
                .build(),
        );
    }
    if !report.previous_paths.is_empty() || !equivalent.is_empty() {
        group.add(&details);
    }
    container.append(&group);
}
