//! Reproducible `__DATE__`, `__TIME__` and `__TIMESTAMP__`
//! (`spec/driver.md` "Reproducible time").

/// The fixed time to use, in seconds since the Unix epoch, if any:
/// `-fkiln-reproducible-time` (epoch 0), else `KILN_REPRODUCIBLE_TIME`
/// (a non-number means 0), else `SOURCE_DATE_EPOCH` (as clang).
pub fn reproducible_epoch(flag: bool) -> Option<i64> {
    if flag {
        return Some(0);
    }
    if let Ok(v) = std::env::var("KILN_REPRODUCIBLE_TIME") {
        return Some(v.trim().parse().unwrap_or(0));
    }
    std::env::var("SOURCE_DATE_EPOCH").ok().and_then(|v| v.trim().parse().ok())
}

const MONTHS: [&str; 12] =
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];

/// The UTC civil time of `epoch`: `(year, month 1..=12, day, h, m, s,
/// weekday name)`.
fn civil(epoch: i64) -> (i64, usize, i64, i64, i64, i64, &'static str) {
    let days = epoch.div_euclid(86_400);
    let secs = epoch.rem_euclid(86_400);
    // Howard Hinnant's `civil_from_days`.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let wd = DAYS[usize::try_from(days.rem_euclid(7)).unwrap_or(0)];
    let month = usize::try_from(m).unwrap_or(1);
    (y, month, d, secs / 3600, secs / 60 % 60, secs % 60, wd)
}

/// `(__DATE__, __TIME__, __TIMESTAMP__)` spellings (without quotes) for
/// `epoch`: `Jan  1 1970`, `00:00:00`, `Thu Jan  1 00:00:00 1970`.
pub fn spellings(epoch: i64) -> (String, String, String) {
    let (y, mo, d, h, mi, s, wd) = civil(epoch);
    let mon = MONTHS[mo - 1];
    let time = format!("{h:02}:{mi:02}:{s:02}");
    (format!("{mon} {d:>2} {y}"), time.clone(), format!("{wd} {mon} {d:>2} {time} {y}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epochs() {
        assert_eq!(
            spellings(0),
            ("Jan  1 1970".into(), "00:00:00".into(), "Thu Jan  1 00:00:00 1970".into())
        );
        // 2024-02-29 13:45:07 UTC, a Thursday.
        assert_eq!(
            spellings(1_709_214_307),
            ("Feb 29 2024".into(), "13:45:07".into(), "Thu Feb 29 13:45:07 2024".into())
        );
    }
}
