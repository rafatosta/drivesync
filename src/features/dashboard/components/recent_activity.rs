//! Last completed report and expandable observed path history. No timestamps or
//! transfer byte totals are invented when the engine does not provide them.
use adw::{gtk, prelude::*};
use drivesync_engine::DownloadReport;

/// Replaces the activity region from one account report. Historical observations
/// remain informational and never become resolution commands.
pub(in crate::features::dashboard) fn present(container: &gtk::Box, report: &DownloadReport) {
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
    let summary = crate::ui::row(
        &gettextrs::gettext("Latest completed check"),
        &report_summary(report),
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

/// Keeps all completed-work counters together without claiming pending items succeeded.
fn report_summary(report: &DownloadReport) -> String {
    let mut text = gettextrs::gettext(
        "{downloaded} downloaded · {updated} updated · {uploaded} uploaded · {unchanged} unchanged",
    )
    .replace("{downloaded}", &report.downloaded.to_string())
    .replace("{updated}", &report.updated.to_string())
    .replace("{uploaded}", &report.uploaded.to_string())
    .replace("{unchanged}", &report.unchanged.to_string());
    for (count, label) in [
        (
            report.created_remote_folders,
            "{count} folders created in Drive",
        ),
        (report.recovered, "{count} recovered"),
        (report.skipped, "{count} skipped"),
        (report.attention().len(), "{count} need attention"),
    ] {
        if count > 0 {
            text.push_str(" · ");
            text.push_str(&gettextrs::gettext(label).replace("{count}", &count.to_string()));
        }
    }
    if report.linked > 0 {
        text.push('\n');
        text.push_str(&gettextrs::gettext("{count} browser shortcuts (Docs, Sheets and Slides: browser only; no offline editing, export or conversion)").replace("{count}", &report.linked.to_string()));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_keeps_extended_counts_and_browser_only_limitations() {
        let report = DownloadReport {
            downloaded: 2,
            updated: 3,
            uploaded: 4,
            unchanged: 5,
            created_remote_folders: 6,
            recovered: 7,
            linked: 8,
            skipped: 9,
            issues: vec![drivesync_engine::DownloadIssue {
                relative_path: None,
                error: drivesync_engine::EngineError::DownloadConflict,
            }],
            ..Default::default()
        };
        let text = report_summary(&report);
        for expected in [
            "2 downloaded",
            "3 updated",
            "4 uploaded",
            "5 unchanged",
            "6 folders created in Drive",
            "7 recovered",
            "8 browser shortcuts",
            "9 skipped",
            "1 need attention",
            "no offline editing, export or conversion",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
    }
}
