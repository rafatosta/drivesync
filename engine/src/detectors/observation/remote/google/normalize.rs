//! Maps provider metadata to evidence without assuming absent attributes are false.
use super::*;
fn evidence<T>(value: Option<T>) -> Evidence<T> {
    value.map(Evidence::Known).unwrap_or(Evidence::NotCollected)
}
fn number(value: Option<String>) -> Evidence<u64> {
    match value {
        Some(value) => value
            .parse()
            .map(Evidence::Known)
            .unwrap_or(Evidence::Unavailable),
        None => Evidence::NotCollected,
    }
}
fn checksum(value: Option<String>) -> Evidence<Checksum> {
    match value {
        Some(value) if value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Evidence::Known(Checksum {
                algorithm: "sha256".into(),
                value: value.to_ascii_lowercase(),
            })
        }
        Some(_) => Evidence::Unavailable,
        None => Evidence::NotCollected,
    }
}
pub(super) fn observation(
    file: GoogleFile,
    config: &RemoteObserverConfig,
    coverage: ObservationCoverage,
) -> Result<RemoteObservation, RemoteObserverError> {
    let id = file
        .id
        .map(|id| RemoteId(id.0))
        .filter(|id| !id.0.is_empty())
        .ok_or(RemoteObserverError::IncompleteCoverage)?;
    let observed_at = super::super::super::scanner::now();
    let nonbinary = file
        .mime_type
        .as_ref()
        .is_some_and(|mime| mime.0.starts_with("application/vnd.google-apps."));
    Ok(RemoteObservation {
        account_id: config.account_id.clone(),
        provider_id: config.provider_id.clone(),
        sync_root_id: config.sync_root_id.clone(),
        state: FileState::Present,
        remote_id: Some(id.clone()),
        name: evidence(file.name),
        parents: evidence(
            file.parents
                .map(|parents| parents.into_iter().map(|id| RemoteId(id.0)).collect()),
        ),
        mime_type: evidence(file.mime_type.map(|m| m.0)),
        size: if nonbinary {
            Evidence::NotApplicable
        } else {
            number(file.size)
        },
        checksum: if nonbinary {
            Evidence::NotApplicable
        } else {
            checksum(file.sha256_checksum)
        },
        version: number(file.version),
        modified_time: match file.modified_time {
            Some(value) => timestamp(&value)
                .map(Evidence::Known)
                .unwrap_or(Evidence::Unavailable),
            None => Evidence::NotCollected,
        },
        trashed: evidence(file.trashed),
        capabilities: evidence(file.capabilities.map(|c| RemoteCapabilities {
            can_download: evidence(c.can_download),
            can_modify_content: evidence(c.can_edit),
            can_rename: evidence(c.can_rename),
            can_move_item_within_drive: evidence(c.can_move_item_within_drive),
            can_trash: evidence(c.can_trash),
        })),
        observed_at,
        coverage,
        page_token: None,
        event: Some(ObservationEvent {
            origin: ObservationOrigin::Remote,
            kind: ObservationKind::Snapshot,
            observed_at,
            old_path: None,
            new_path: None,
            remote_id: Some(id),
            move_cookie: None,
        }),
    })
}
/// Drive emits UTC RFC3339 timestamps. Reject other layouts rather than fabricating a time.
fn timestamp(text: &str) -> Option<DateTime> {
    let (date, time) = text.strip_suffix('Z')?.split_once('T')?;
    if date.len() != 10 {
        return None;
    }
    let mut date = date.split('-');
    let year: i32 = date.next()?.parse().ok()?;
    let month: i32 = date.next()?.parse().ok()?;
    let day: i32 = date.next()?.parse().ok()?;
    if date.next().is_some() {
        return None;
    }
    let (clock, fraction) = time.split_once('.').unwrap_or((time, ""));
    if clock.len() != 8 || (time.contains('.') && fraction.is_empty()) {
        return None;
    }
    let mut clock = clock.split(':');
    let hour: i32 = clock.next()?.parse().ok()?;
    let minute: i32 = clock.next()?.parse().ok()?;
    let second: i32 = clock.next()?.parse().ok()?;
    if clock.next().is_some()
        || !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || !(0..24).contains(&hour)
        || !(0..60).contains(&minute)
        || !(0..60).contains(&second)
        || fraction.len() > 9
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = year - 1900;
    tm.tm_mon = month - 1;
    tm.tm_mday = day;
    tm.tm_hour = hour;
    tm.tm_min = minute;
    tm.tm_sec = second;
    let seconds = unsafe { libc::timegm(&mut tm) };
    if tm.tm_year != year - 1900 || tm.tm_mon != month - 1 || tm.tm_mday != day {
        return None;
    }
    let millis = format!("{fraction:0<3}").get(..3)?.parse::<i64>().ok()?;
    Some(DateTime {
        unix_millis: seconds.checked_mul(1000)?.checked_add(millis)?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utc_precision_and_invalid_calendar_dates() {
        assert_eq!(
            timestamp("1970-01-01T00:00:00.123456Z")
                .unwrap()
                .unix_millis,
            123
        );
        assert_eq!(timestamp("1970-01-01T00:00:00Z").unwrap().unix_millis, 0);
        assert!(timestamp("2026-02-30T00:00:00Z").is_none());
        assert!(timestamp("2026-01-01T00:00:00+03:00").is_none());
    }
}
