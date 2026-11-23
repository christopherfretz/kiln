//! Local time for `__TIMESTAMP__` (WP-2331, spec/pp.md "Builtin
//! macros"): clang spells the current file's modification time with
//! `localtime`, so the zone is the C library's: the `TZ` environment
//! variable's, else `/etc/localtime`. A zone is a `TZif` file (RFC 8536,
//! versions 1 to 4) or a POSIX `TZ` string (POSIX.1-2017 §8.3, also the
//! `TZif` footer that covers the times after its last transition). Anything
//! unreadable is UTC, as glibc treats an unknown zone.

use std::path::Path;

/// Where named zones live (glibc's default `TZDIR`).
const ZONEINFO: &str = "/usr/share/zoneinfo";

/// A time zone: UTC offsets by time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TimeZone {
    /// Transition times (seconds since the epoch, ascending) and the
    /// offset in effect from each.
    transitions: Vec<(i64, i32)>,
    /// The offset before the first transition.
    initial: i32,
    /// The rule after the last transition (or for every time, with no
    /// transitions): a POSIX `TZ` string's.
    rule: Option<Rule>,
}

/// A POSIX `TZ` rule: standard time, and daylight saving time between
/// two dates of each year.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rule {
    /// The standard time's offset east of UTC, in seconds.
    std: i32,
    /// Daylight saving time: its offset, and when it starts and ends (in
    /// local standard and local daylight time respectively).
    dst: Option<(i32, Date, Date)>,
}

/// A rule's date and its local time of day (seconds, may be negative or
/// past a day, RFC 8536 §3.3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Date {
    day: Day,
    time: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Day {
    /// `Jn`: day 1..=365, February 29 never counted.
    Julian(u16),
    /// `n`: day 0..=365, February 29 counted in leap years.
    Zero(u16),
    /// `Mm.w.d`: weekday `d` (0 = Sunday) of week `w` (1..=5, 5 = the
    /// last) of month `m`.
    Month { month: u8, week: u8, weekday: u8 },
}

impl TimeZone {
    /// The C library's zone for `tz`, the `TZ` variable's value (`None`:
    /// unset).
    pub(crate) fn load(tz: Option<&str>) -> TimeZone {
        let Some(tz) = tz else {
            return Self::from_file(Path::new("/etc/localtime")).unwrap_or_default();
        };
        let name = tz.strip_prefix(':').unwrap_or(tz);
        if name.is_empty() {
            return TimeZone::default();
        }
        let file = if name.starts_with('/') {
            Self::from_file(Path::new(name))
        } else if name.split('/').any(|c| c == "..") {
            None
        } else {
            Self::from_file(&Path::new(ZONEINFO).join(name))
        };
        file.or_else(|| {
            parse_rule(name.as_bytes()).map(|rule| TimeZone {
                rule: Some(rule),
                initial: rule.std,
                ..TimeZone::default()
            })
        })
        .unwrap_or_default()
    }

    fn from_file(path: &Path) -> Option<TimeZone> {
        Self::from_tzif(&std::fs::read(path).ok()?)
    }

    /// Parses `TZif` data (RFC 8536 §3).
    pub(crate) fn from_tzif(data: &[u8]) -> Option<TimeZone> {
        let (v1, rest) = Block::parse(data, 4)?;
        if data[4] == 0 {
            return v1.zone(None);
        }
        // Version 2 and later: a second header and block with 64-bit
        // times, then the footer.
        let at = data.len() - rest.len();
        let (v2, rest) = Block::parse(&data[at..], 8)?;
        let footer = rest.strip_prefix(b"\n")?;
        let end = footer.iter().position(|&b| b == b'\n')?;
        let footer = &footer[..end];
        let rule = if footer.is_empty() { None } else { Some(parse_rule(footer)?) };
        v2.zone(rule)
    }

    /// The offset east of UTC in effect at `t` (seconds since the epoch).
    pub(crate) fn offset_at(&self, t: i64) -> i32 {
        let after = self.transitions.partition_point(|&(at, _)| at <= t);
        match (after, &self.rule) {
            (n, Some(rule)) if n == self.transitions.len() => rule.offset_at(t),
            (0, _) => self.initial,
            (n, _) => self.transitions[n - 1].1,
        }
    }
}

/// One `TZif` data block.
struct Block<'d> {
    times: &'d [u8],
    indices: &'d [u8],
    types: &'d [u8],
    width: usize,
}

impl<'d> Block<'d> {
    /// The header at the start of `data` and its block of `width`-byte
    /// times; what follows.
    fn parse(data: &'d [u8], width: usize) -> Option<(Block<'d>, &'d [u8])> {
        if data.len() < 44 || &data[..4] != b"TZif" {
            return None;
        }
        let count = |i: usize| -> Option<usize> {
            let b = data.get(20 + 4 * i..24 + 4 * i)?;
            usize::try_from(u32::from_be_bytes(b.try_into().ok()?)).ok()
        };
        let (isut, isstd, leap, time, typ, chars) =
            (count(0)?, count(1)?, count(2)?, count(3)?, count(4)?, count(5)?);
        if typ == 0 {
            return None;
        }
        let body = &data[44..];
        let mut at = 0usize;
        let mut take = |n: usize| -> Option<&'d [u8]> {
            let s = body.get(at..at.checked_add(n)?)?;
            at += n;
            Some(s)
        };
        let times = take(time.checked_mul(width)?)?;
        let indices = take(time)?;
        let types = take(typ.checked_mul(6)?)?;
        take(chars)?;
        take(leap.checked_mul(width + 4)?)?;
        take(isstd)?;
        take(isut)?;
        Some((Block { times, indices, types, width }, &body[at..]))
    }

    /// The offset of local time type `i`.
    fn offset(&self, i: usize) -> Option<i32> {
        let b = self.types.get(6 * i..6 * i + 4)?;
        Some(i32::from_be_bytes(b.try_into().ok()?))
    }

    fn zone(&self, rule: Option<Rule>) -> Option<TimeZone> {
        let mut transitions = Vec::with_capacity(self.indices.len());
        for (i, &ty) in self.indices.iter().enumerate() {
            let b = &self.times[i * self.width..(i + 1) * self.width];
            let t = if self.width == 8 {
                i64::from_be_bytes(b.try_into().ok()?)
            } else {
                i64::from(i32::from_be_bytes(b.try_into().ok()?))
            };
            transitions.push((t, self.offset(usize::from(ty))?));
        }
        // RFC 8536 §3.2: before the first transition, local time type 0.
        Some(TimeZone { transitions, initial: self.offset(0)?, rule })
    }
}

impl Rule {
    fn offset_at(&self, t: i64) -> i32 {
        let Some((dst, start, end)) = self.dst else { return self.std };
        let year = year_of(t.saturating_add(i64::from(self.std)));
        // The start is given in standard time, the end in daylight time.
        let on = start.at(year).saturating_sub(i64::from(self.std));
        let off = end.at(year).saturating_sub(i64::from(dst));
        let in_dst = if on < off { on <= t && t < off } else { !(off <= t && t < on) };
        if in_dst { dst } else { self.std }
    }
}

impl Date {
    /// This date in `year`, as local seconds since the epoch.
    fn at(self, year: i64) -> i64 {
        let jan1 = days_from_civil(year, 1, 1);
        let leap = is_leap(year);
        let day = match self.day {
            Day::Julian(n) => {
                let n = i64::from(n) - 1;
                // Day 59 (0-based) and later are after February 28.
                jan1 + n + i64::from(leap && n >= 59)
            }
            Day::Zero(n) => jan1 + i64::from(n),
            Day::Month { month, week, weekday } => {
                let first = days_from_civil(year, i64::from(month), 1);
                // 1970-01-01 was a Thursday (weekday 4).
                let wd_first = (first + 4).rem_euclid(7);
                let mut d = first + (i64::from(weekday) - wd_first).rem_euclid(7);
                d += 7 * (i64::from(week) - 1);
                let next = if month == 12 {
                    days_from_civil(year + 1, 1, 1)
                } else {
                    days_from_civil(year, i64::from(month) + 1, 1)
                };
                while d >= next {
                    d -= 7;
                }
                d
            }
        };
        day.saturating_mul(86_400).saturating_add(i64::from(self.time))
    }
}

fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

/// Howard Hinnant's `days_from_civil`.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil year of local time `t`.
fn year_of(t: i64) -> i64 {
    crate::builtin::civil_from_days(t.div_euclid(86_400)).0
}

/// Parses a POSIX `TZ` string: `std offset [dst [offset] [,start[/time],end[/time]]]`.
fn parse_rule(s: &[u8]) -> Option<Rule> {
    let mut p = Cursor { s, at: 0 };
    p.name()?;
    let std = -p.offset()?;
    if p.done() {
        return Some(Rule { std, dst: None });
    }
    p.name()?;
    let dst = if p.peek().is_some_and(|c| c == b',') || p.done() {
        std.checked_add(3600)?
    } else {
        -p.offset()?
    };
    // glibc's default rule without one (the US rule since 2007).
    let (start, end) = if p.done() {
        (
            Date { day: Day::Month { month: 3, week: 2, weekday: 0 }, time: 7200 },
            Date { day: Day::Month { month: 11, week: 1, weekday: 0 }, time: 7200 },
        )
    } else {
        p.eat(b',')?;
        let start = p.date()?;
        p.eat(b',')?;
        let end = p.date()?;
        (start, end)
    };
    p.done().then_some(Rule { std, dst: Some((dst, start, end)) })
}

struct Cursor<'s> {
    s: &'s [u8],
    at: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn done(&self) -> bool {
        self.at == self.s.len()
    }

    fn eat(&mut self, c: u8) -> Option<()> {
        (self.peek() == Some(c)).then(|| self.at += 1)
    }

    /// A zone abbreviation: three or more letters, or `<...>` quoted.
    fn name(&mut self) -> Option<()> {
        let start = self.at;
        if self.eat(b'<').is_some() {
            while self.peek()? != b'>' {
                self.at += 1;
            }
            self.at += 1;
            return (self.at - start >= 5).then_some(());
        }
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.at += 1;
        }
        (self.at - start >= 3).then_some(())
    }

    fn number(&mut self) -> Option<i32> {
        let start = self.at;
        let mut n = 0i32;
        while let Some(c) = self.peek().filter(u8::is_ascii_digit) {
            n = n.checked_mul(10)?.checked_add(i32::from(c - b'0'))?;
            self.at += 1;
        }
        (self.at > start).then_some(n)
    }

    /// `[+-]hh[:mm[:ss]]`, in seconds.
    fn offset(&mut self) -> Option<i32> {
        let neg = match self.peek() {
            Some(b'-') => {
                self.at += 1;
                true
            }
            Some(b'+') => {
                self.at += 1;
                false
            }
            _ => false,
        };
        let mut secs = self.number()?.checked_mul(3600)?;
        if self.eat(b':').is_some() {
            secs = secs.checked_add(self.number()?.checked_mul(60)?)?;
            if self.eat(b':').is_some() {
                secs = secs.checked_add(self.number()?)?;
            }
        }
        Some(if neg { -secs } else { secs })
    }

    /// `Jn`, `n` or `Mm.w.d`, then `[/time]` (default 02:00:00).
    fn date(&mut self) -> Option<Date> {
        let day = match self.peek()? {
            b'J' => {
                self.at += 1;
                let n = self.number()?;
                Day::Julian(u16::try_from(n).ok().filter(|n| (1..=365).contains(n))?)
            }
            b'M' => {
                self.at += 1;
                let month = self.number()?;
                self.eat(b'.')?;
                let week = self.number()?;
                self.eat(b'.')?;
                let weekday = self.number()?;
                let ok = (1..=12).contains(&month) && (1..=5).contains(&week) && weekday <= 6;
                if !ok {
                    return None;
                }
                Day::Month {
                    month: u8::try_from(month).ok()?,
                    week: u8::try_from(week).ok()?,
                    weekday: u8::try_from(weekday).ok()?,
                }
            }
            _ => Day::Zero(u16::try_from(self.number()?).ok().filter(|&n| n <= 365)?),
        };
        let time = if self.eat(b'/').is_some() { self.offset()? } else { 7200 };
        Some(Date { day, time })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NY: &str = "EST5EDT,M3.2.0,M11.1.0";

    /// 2026-11-22 21:05:00 EST (UTC-5), 2026-07-04 12:00:00 EDT (UTC-4).
    const NOV: i64 = 1_795_399_500;
    const JUL: i64 = 1_783_180_800;

    #[test]
    fn posix_rules() {
        let ny = TimeZone::load(Some(NY));
        assert_eq!(ny.offset_at(NOV), -5 * 3600);
        assert_eq!(ny.offset_at(JUL), -4 * 3600);
        // 2026-03-08 02:00 EST is 07:00 UTC: DST starts there.
        let start = days_from_civil(2026, 3, 8) * 86_400 + 7 * 3600;
        assert_eq!(ny.offset_at(start - 1), -5 * 3600);
        assert_eq!(ny.offset_at(start), -4 * 3600);
        // 2026-11-01 02:00 EDT is 06:00 UTC: DST ends there.
        let end = days_from_civil(2026, 11, 1) * 86_400 + 6 * 3600;
        assert_eq!(ny.offset_at(end - 1), -4 * 3600);
        assert_eq!(ny.offset_at(end), -5 * 3600);
        // A southern zone (DST over the new year), quoted names, a
        // Julian and a zero-based day.
        let au = TimeZone::load(Some("AEST-10AEDT,M10.1.0,M4.1.0/3"));
        assert_eq!(au.offset_at(JUL), 10 * 3600);
        assert_eq!(au.offset_at(NOV), 11 * 3600);
        let q = TimeZone::load(Some("<+0330>-3:30"));
        assert_eq!(q.offset_at(NOV), 3 * 3600 + 1800);
        let j = TimeZone::load(Some("XST3XDT,J60/0,300/0"));
        assert_eq!(j.offset_at(days_from_civil(2024, 3, 1) * 86_400 + 3 * 3600), -2 * 3600);
        assert_eq!(j.offset_at(days_from_civil(2024, 2, 29) * 86_400 + 3 * 3600), -3 * 3600);
        // Empty or unreadable: UTC.
        assert_eq!(TimeZone::load(Some("")).offset_at(NOV), 0);
        assert_eq!(TimeZone::load(Some(":")).offset_at(NOV), 0);
        assert_eq!(TimeZone::load(Some("not a zone")).offset_at(NOV), 0);
        assert_eq!(TimeZone::load(Some("../../etc/passwd")).offset_at(NOV), 0);
    }

    /// A `TZif` file of `version` with transitions `(time, type)`, types
    /// `(offset, is_dst)`, and `footer`.
    fn tzif(version: u8, transitions: &[(i64, u8)], types: &[(i32, u8)], footer: &str) -> Vec<u8> {
        let block = |out: &mut Vec<u8>, wide: bool| {
            out.extend_from_slice(b"TZif");
            out.push(version);
            out.extend_from_slice(&[0; 15]);
            let n = |x: usize| u32::try_from(x).unwrap().to_be_bytes();
            for c in [0, 0, 0, transitions.len(), types.len(), 4] {
                out.extend_from_slice(&n(c));
            }
            for &(t, _) in transitions {
                if wide {
                    out.extend_from_slice(&t.to_be_bytes());
                } else {
                    out.extend_from_slice(&i32::try_from(t).unwrap().to_be_bytes());
                }
            }
            out.extend(transitions.iter().map(|&(_, i)| i));
            for &(off, dst) in types {
                out.extend_from_slice(&off.to_be_bytes());
                out.extend_from_slice(&[dst, 0]);
            }
            out.extend_from_slice(b"ABC\0");
        };
        let mut out = Vec::new();
        block(&mut out, false);
        if version != 0 {
            block(&mut out, true);
            out.extend_from_slice(format!("\n{footer}\n").as_bytes());
        }
        out
    }

    #[test]
    fn tzif_files() {
        let types = [(-5 * 3600, 0), (-4 * 3600, 1)];
        // A version-1 file: the last transition's type stays in effect.
        let v1 = tzif(0, &[(1_000_000_000, 1)], &types, "");
        let z = TimeZone::from_tzif(&v1).unwrap();
        assert_eq!(z.offset_at(999_999_999), -5 * 3600);
        assert_eq!(z.offset_at(NOV), -4 * 3600);
        // A version-2 file with a footer for the times after it.
        let v2 = tzif(b'2', &[(1_000_000_000, 1), (1_010_000_000, 0)], &types, NY);
        let z = TimeZone::from_tzif(&v2).unwrap();
        assert_eq!(z.offset_at(0), -5 * 3600);
        assert_eq!(z.offset_at(1_005_000_000), -4 * 3600);
        assert_eq!(z.offset_at(NOV), -5 * 3600);
        assert_eq!(z.offset_at(JUL), -4 * 3600);
        // No footer rule: the last type.
        let v2 = tzif(b'2', &[(1_000_000_000, 1)], &types, "");
        assert_eq!(TimeZone::from_tzif(&v2).unwrap().offset_at(NOV), -4 * 3600);
        // A UTC file (no transitions, footer "UTC0").
        let utc = tzif(b'2', &[], &[(0, 0)], "UTC0");
        assert_eq!(TimeZone::from_tzif(&utc).unwrap().offset_at(NOV), 0);
        // Truncated or not TZif.
        assert_eq!(TimeZone::from_tzif(&v1[..40]), None);
        assert_eq!(TimeZone::from_tzif(b"not a zone file at all, not at all, no....."), None);
    }
}
