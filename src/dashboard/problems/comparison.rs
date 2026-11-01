//! Literal read-only previews and separate temporary copies for external viewers.
use adw::{gtk, prelude::*};
use drivesync_engine::{ComparisonContent, VersionComparison};

/// A bounded linear diff: equal edges surround one replaced region. It does not
/// claim a minimal edit script or normalize line endings/terminal newlines.
fn text_diff(local: &[u8], remote: &[u8]) -> Option<String> {
    if local.len() + remote.len() > 256 * 1024 {
        return None;
    }
    let local = std::str::from_utf8(local).ok()?;
    let remote = std::str::from_utf8(remote).ok()?;
    if local
        .chars()
        .chain(remote.chars())
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return None;
    }
    let a: Vec<_> = local.split_inclusive('\n').collect();
    let b: Vec<_> = remote.split_inclusive('\n').collect();
    if a.len() + b.len() > 4000 {
        return None;
    }
    let prefix = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let mut result = String::from("− Local only   + Drive only   = Unchanged\nChanged regions may include unchanged interior lines.\n\n");
    for (mark, lines) in [
        ('=', &a[..prefix]),
        ('−', &a[prefix..a.len() - suffix]),
        ('+', &b[prefix..b.len() - suffix]),
        ('=', &a[a.len() - suffix..]),
    ] {
        for line in lines {
            result.push(mark);
            result.push(' ');
            result.push_str(&line.replace('\r', "␍"));
            if !line.ends_with('\n') {
                result.push_str("\n[No newline at end of file]\n");
            }
        }
    }
    if local == remote {
        result
            .push_str("\nVerified bytes are identical. This does not resolve the pending item.\n");
    }
    Some(result)
}

fn label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .use_markup(false)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .selectable(true)
        .xalign(0.0)
        .build()
}

/// Only bounded PNG dimensions are decoded in-process; other images retain the
/// metadata/external-viewer fallback instead of unbounded raster allocation.
fn png_preview(bytes: &[u8]) -> Option<gtk::Picture> {
    if bytes.get(..8)? != b"\x89PNG\r\n\x1a\n" || bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
    let height = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 4_000_000 {
        return None;
    }
    let texture = gtk::gdk::Texture::from_bytes(&gtk::glib::Bytes::from(bytes)).ok()?;
    let picture = gtk::Picture::for_paintable(&texture);
    picture.set_can_shrink(true);
    picture.set_size_request(1, 180);
    picture.set_alternative_text(Some("Snapshot of the observed version; use the labeled metadata and external viewer for details."));
    Some(picture)
}

/// Creates a private temporary copy. Its lifetime is independent of the window
/// and application launch; the OS may clean it later. Originals are never opened.
fn external_copy(bytes: &[u8], suffix: &str) -> Result<gtk::gio::File, gtk::glib::Error> {
    let (file, stream) =
        gtk::gio::File::new_tmp(Some(format!("drivesync-comparison-XXXXXX.{suffix}")))?;
    let output = stream.output_stream();
    let (_, error) = output.write_all(bytes, gtk::gio::Cancellable::NONE)?;
    if let Some(error) = error {
        return Err(error);
    }
    stream.close(gtk::gio::Cancellable::NONE)?;
    Ok(file)
}

fn open_button(
    parent: &adw::Window,
    bytes: &[u8],
    name: &str,
    mime: &str,
    status: &gtk::Label,
) -> gtk::Button {
    let button = gtk::Button::with_label(&format!("Open {name} copy in an application…"));
    let bytes = bytes.to_vec();
    let suffix = match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "application/pdf" => "pdf",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx",
        "application/vnd.oasis.opendocument.text" => "odt",
        "application/vnd.oasis.opendocument.spreadsheet" => "ods",
        "application/vnd.oasis.opendocument.presentation" => "odp",
        _ if std::str::from_utf8(&bytes).is_ok() && !bytes.contains(&0) => "txt",
        _ => "bin",
    };
    let parent = parent.downgrade();
    let status = status.clone();
    button.connect_clicked(move |button| {
        if parent.upgrade().is_none() { return; }
        button.set_sensitive(false);
        status.set_label("Preparing a separate temporary snapshot…");
        let bytes = bytes.clone();
        let button = button.clone();
        let parent = parent.clone();
        let status = status.clone();
        crate::flow::work(
            move || external_copy(&bytes, suffix).map_err(|_| drivesync_engine::EngineError::DownloadIo),
            move |result| {
                let Some(parent) = parent.upgrade().filter(|w| w.is_visible()) else {
                    if let Ok(file) = result {
                        std::thread::spawn(move || { let _ = file.delete(gtk::gio::Cancellable::NONE); });
                    }
                    return;
                };
                match result {
                    Err(error) => {
                        button.set_sensitive(true);
                        status.set_label(&format!("Could not prepare a copy: {error}. Original versions remain unchanged."));
                    }
                    Ok(file) => {
                        status.set_label(&format!("Temporary snapshot: {}. Edits to this copy are not synchronized. The system may clean it later.", file.path().map(|p|p.display().to_string()).unwrap_or_default()));
                        gtk::FileLauncher::new(Some(&file)).launch(Some(&parent), gtk::gio::Cancellable::NONE, move |result| {
                            button.set_sensitive(true);
                            if let Err(error) = result { status.set_label(&format!("The application did not confirm opening the copy: {error}. Original versions remain unchanged.")); }
                        });
                    }
                }
            },
        );
    });
    button
}

pub(super) fn show(parent: &adw::Window, account: &str, comparison: &VersionComparison) {
    let window = adw::Window::builder()
        .title("Compare observed versions")
        .default_width(720)
        .default_height(600)
        .modal(true)
        .transient_for(parent)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let content = crate::flow::content();
    let record = &comparison.record;
    let modified = record
        .local_modified_unix
        .and_then(|seconds| gtk::glib::DateTime::from_unix_utc(seconds).ok())
        .and_then(|date| date.format("%Y-%m-%d %H:%M:%S UTC").ok())
        .map(|date| date.to_string())
        .unwrap_or_else(|| "unknown".into());
    content.append(&label(&format!("Account: {account}\nFile: {}\nType: {}\nLocal: {} bytes · observed modification time: {}\nDrive: {} bytes · revision: {}\nDrive modification time: unavailable\n\nThese are verified snapshots. A later change requires a new comparison. Close to decide later; choose local, Drive or both in the problem center, where consequences are confirmed and versions revalidated.",record.relative_path.as_ref().map(|p|p.display().to_string()).unwrap_or_default(),comparison.mime_type,comparison.local_size,modified,comparison.remote_size,record.remote_version.map(|v|v.to_string()).unwrap_or_else(||"unknown".into()))));
    match &comparison.content {
        ComparisonContent::MetadataOnly => content.append(&label("Content preview and external copies unavailable: at least one version exceeds 1 MiB. No content was truncated. Existing versions remain pending.")),
        ComparisonContent::Available { local, remote } => {
            if let Some(diff) = text_diff(local, remote) {
                let view = gtk::TextView::builder().editable(false).cursor_visible(true).monospace(true).wrap_mode(gtk::WrapMode::WordChar).build();
                view.buffer().set_text(&diff);
                view.update_property(&[gtk::accessible::Property::Label("Line differences: local minus, Drive plus, unchanged equals")]);
                content.append(&view);
            } else {
                content.append(&label("Text diff unavailable for this format, encoding or display limit. Compare metadata or open separate copies."));
                for (name, bytes) in [("Local", local), ("Drive", remote)] {
                    content.append(&label(name));
                    if let Some(picture) = png_preview(bytes) { content.append(&picture); }
                    else { content.append(&label("Thumbnail unavailable.")); }
                }
            }
            let status = label("Opening uses a separate private temporary snapshot. Changes in an external application do not resolve this conflict or change synchronized originals.");
            content.append(&status);
            for (name, bytes) in [("local", local), ("Drive", remote)] {
                content.append(&open_button(&window, bytes, name, &comparison.mime_type, &status));
            }
        }
    }
    toolbar.set_content(Some(&crate::flow::scroll(&content, 760)));
    window.set_content(Some(&toolbar));
    window.present();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_diff_preserves_unicode_endings_and_literal_markup_with_bounded_work() {
        let diff = text_diff("same\nolá <a>\nend\n".as_bytes(), b"same\nnew\nend\n").unwrap();
        assert!(diff.contains("− olá <a>\n+ new\n= end"));
        assert!(text_diff(b"a\n", b"a").unwrap().contains("No newline"));
        assert!(text_diff(b"a\r\n", b"a\n").unwrap().contains('␍'));
        assert!(text_diff(b"", b"").unwrap().contains("identical"));
        assert!(text_diff(b"\xff", b"x").is_none());
        assert!(text_diff(b"a\0", b"x").is_none());
        assert!(text_diff(&vec![b'x'; 256 * 1024 + 1], b"").is_none());
        assert!(text_diff(&b"x\n".repeat(4001), b"").is_none());
    }
    #[test]
    fn external_snapshot_is_private_independent_and_preserves_exact_bytes() {
        use std::os::unix::fs::PermissionsExt;
        let file = external_copy(b"observed\0bytes", "bin").unwrap();
        let path = file.path().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"observed\0bytes");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        file.delete(gtk::gio::Cancellable::NONE).unwrap();
    }
    #[test]
    #[ignore = "requires an isolated GTK display"]
    fn comparison_windows_render_text_png_and_fallback_at_narrow_width() {
        adw::init().unwrap();
        let parent = adw::Window::new();
        let record = drivesync_engine::ConflictRecord {
            conflict_id: "a".repeat(64),
            remote_id: Some("file".into()),
            remote_parent_id: Some("root".into()),
            remote_name: Some("<report>&.txt".into()),
            relative_path: Some("<report>&.txt".into()),
            diagnosis: drivesync_engine::ChangeKind::ConcurrentChanges,
            status: drivesync_engine::ConflictStatus::Pending,
            local_size: Some(5),
            local_modified_unix: None,
            remote_size: Some(6),
            local_fingerprint: Some(format!("{}:1:2", "b".repeat(64))),
            remote_version: Some(2),
            outcomes: vec![],
        };
        let pixels = gtk::glib::Bytes::from_static(&[255, 0, 0, 255]);
        let texture =
            gtk::gdk::MemoryTexture::new(1, 1, gtk::gdk::MemoryFormat::R8g8b8a8, &pixels, 4);
        let png = texture.save_to_png_bytes();
        assert!(png_preview(&png).is_some());
        let mut huge = png.to_vec();
        huge[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(png_preview(&huge).is_none());
        assert!(png_preview(b"not an image").is_none());
        for (name, content) in [
            (
                "text",
                ComparisonContent::Available {
                    local: b"same\n<local>\n".to_vec(),
                    remote: b"same\nDrive\n".to_vec(),
                },
            ),
            (
                "png",
                ComparisonContent::Available {
                    local: png.to_vec(),
                    remote: png.to_vec(),
                },
            ),
            (
                "binary",
                ComparisonContent::Available {
                    local: vec![0, 255],
                    remote: vec![1, 255],
                },
            ),
            ("metadata", ComparisonContent::MetadataOnly),
        ] {
            let comparison = VersionComparison {
                record: record.clone(),
                mime_type: "image/png".into(),
                local_size: 5,
                remote_size: 6,
                content,
            };
            show(&parent, "test@example.invalid", &comparison);
            let window = gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|w| w.downcast::<adw::Window>().ok())
                .find(|w| w.title().as_deref() == Some("Compare observed versions"))
                .unwrap();
            window.set_default_size(360, 640);
            let frame = gtk::glib::MainLoop::new(None, false);
            let finish = frame.clone();
            gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
                finish.quit()
            });
            frame.run();
            assert!(window.width() <= 360, "{name}: {}", window.width());
            if let Ok(output) = std::env::var("DRIVESYNC_COMPARISON_UI_OUTPUT") {
                std::fs::create_dir_all(&output).unwrap();
                let snapshot = gtk::Snapshot::new();
                gtk::WidgetPaintable::new(Some(&window)).snapshot(
                    &snapshot,
                    window.width() as f64,
                    window.height() as f64,
                );
                window
                    .renderer()
                    .unwrap()
                    .render_texture(snapshot.to_node().unwrap(), None)
                    .save_to_png(
                        std::path::Path::new(&output).join(format!("comparison-{name}.png")),
                    )
                    .unwrap();
            }
            window.close();
        }
        parent.close();
    }
}
