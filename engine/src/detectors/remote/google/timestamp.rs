//! Strict UTC timestamps used only as creation evidence; unsupported forms stay unknown.
/// Parses Google's normalized UTC RFC3339 form without timezone or locale state.
pub(super) fn utc_millis(value: &str) -> Option<i64> {
    if !value.is_ascii() || !value.ends_with('Z') || value.len() < 20 {
        return None;
    }
    let bytes = value.as_bytes();
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    fn number(value: &str) -> Option<i64> {
        if !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        value.parse().ok()
    }
    let year = number(&value[..4])?;
    let month = number(&value[5..7])?;
    let day = number(&value[8..10])?;
    let hour = number(&value[11..13])?;
    let minute = number(&value[14..16])?;
    let second = number(&value[17..19])?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => return None,
    };
    if year < 1970 || !(1..=days).contains(&day) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let fraction = &value[19..value.len() - 1];
    let millis = if fraction.is_empty() {
        0
    } else {
        let digits = fraction.strip_prefix('.')?;
        if digits.is_empty()
            || digits.len() > 9
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let mut milliseconds = digits.chars().take(3).collect::<String>();
        while milliseconds.len() < 3 {
            milliseconds.push('0');
        }
        number(&milliseconds)?
    };
    // Gregorian civil date to days since 1970, keeping arithmetic in integer UTC.
    let year = year - i64::from(month <= 2);
    let era = year / 400;
    let yoe = year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * adjusted_month + 2) / 5 + day - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    Some(((days * 24 + hour) * 60 + minute) * 60000 + second * 1000 + millis)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_normalized_utc_and_rejects_ambiguous_or_invalid_dates() {
        assert_eq!(utc_millis("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            utc_millis("2000-02-29T12:34:56.123456Z"),
            Some(951827696123)
        );
        for date in [
            "2025-02-29T00:00:00Z",
            "2026-11-13T00:00:00+01:00",
            "2026-13-01T00:00:00Z",
            "2026-11-13T24:00:00Z",
            "garbage",
            "ééééééééééééééééééééZ",
        ] {
            assert_eq!(utc_millis(date), None);
        }
    }
}
