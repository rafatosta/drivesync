//! Validates DriveSync link files and opens Workspace URLs through the desktop.
use gtk::{gio, glib};
use std::{ffi::OsStr, fs};

const FORMAT_MARKER: &str = "DRIVESYNC-LINK/1";
const MAX_LINK_FILE_SIZE: u64 = 16 * 1024;

/// Opens one `.dslink` file using the system's default handler for HTTPS URLs.
///
/// The file is treated as untrusted data. Only HTTPS links to supported Google
/// Docs, Sheets, or Slides paths are accepted; errors never print the private URL.
pub(crate) fn launch(path: Option<&OsStr>) -> glib::ExitCode {
    let result = path.ok_or(()).and_then(read_link);
    match result {
        Ok(url) => match gio::AppInfo::launch_default_for_uri(
            &url,
            None::<&gio::AppLaunchContext>,
        ) {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(_) => report_failure(),
        },
        Err(()) => report_failure(),
    }
}

fn report_failure() -> glib::ExitCode {
    eprintln!("DriveSync could not open this Google document link.");
    glib::ExitCode::FAILURE
}

fn read_link(path: &OsStr) -> Result<String, ()> {
    let metadata = fs::metadata(path).map_err(|_| ())?;
    if !metadata.is_file() || metadata.len() > MAX_LINK_FILE_SIZE {
        return Err(());
    }
    let contents = fs::read_to_string(path).map_err(|_| ())?;
    let mut lines = contents.lines();
    if lines.next() != Some(FORMAT_MARKER) {
        return Err(());
    }
    let link = lines.next().ok_or(())?;
    if lines.next().is_some() || !valid_workspace_link(link) {
        return Err(());
    }
    Ok(link.to_owned())
}

fn valid_workspace_link(link: &str) -> bool {
    let Ok(url) = url::Url::parse(link) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str() == Some("docs.google.com")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && ["/document/", "/spreadsheets/", "/presentation/"]
            .iter()
            .any(|path| url.path().contains(path))
        && url.path().contains("/d/")
        && !link.chars().any(char::is_control)
}
