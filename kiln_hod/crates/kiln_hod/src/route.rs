//! The route of each module (spec/driver.md "The `kiln_hod` workspace"):
//! a module takes the bridge (`hod_ir` built in process, fed to a
//! `hod::Session`); a module the bridge does not take, every module under
//! `KILN_HOD_BRIDGE=0` and a module with debug information under
//! `KILN_HOD_BRIDGE_DEBUG=0` take the text path. The choice is made per module and before the session
//! starts: a module is never half bridged.

use std::fmt::Write as _;
use std::io::Write as _;
use std::time::{Duration, Instant};

use hod::coverage::{Outcome, Record};
use hod::{InputKind, Options, OutputKind, Report, Session, Settings};
use hod_ir::TargetSpec;
use kiln_driver::backend_hook::Answer;

use crate::Switches;
use crate::bridge::{self, Bridge, Built, Fallback, eq};

/// Where a module goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Route {
    /// The bridge.
    Bridge,
    /// The text path, for this reason (`disabled`, `debug`).
    Text(&'static str),
}

/// The route of `module`.
pub(crate) fn choose(switches: &Switches, module: &kiln_llvm::Module, _: &Options) -> Route {
    if !switches.bridge {
        Route::Text("disabled")
    } else if !switches.bridge_debug && bridge::has_debug_info(module) {
        Route::Text("debug")
    } else {
        Route::Bridge
    }
}

/// `-mtriple`'s target, as hod's front makes it.
fn forced(options: &Options) -> Result<Option<TargetSpec>, Fallback> {
    match options.triple.as_deref().map(TargetSpec::from_triple) {
        None => Ok(None),
        Some(Ok(target)) => Ok(Some(target)),
        Some(Err(_)) => Err(Fallback("unsupported-target".into())),
    }
}

/// The bridged compile: the answer, and hod's `Report` (its phases, and
/// `bridge`: the producer's time). `Err` before anything was delivered to a session, or
/// where the bridge met what it does not map: the caller takes the text
/// path, whose answer for such a module is hod's own.
pub(crate) fn bridged(
    module: &kiln_llvm::Module,
    options: &Options,
    settings: &Settings,
    object: &mut Vec<u8>,
    record: &mut Record,
) -> Result<(Answer, Report), Fallback> {
    // What hod's front judges before reading, judged as it does, is left
    // to the text path: its answer is the one to give.
    if options.output_kind != OutputKind::Object || options.arch.is_some() {
        return Err(Fallback("options".into()));
    }
    let triple = options
        .triple
        .as_deref()
        .or_else(|| Some(module.target_triple.as_str()).filter(|t| !t.is_empty()));
    if triple.is_some_and(|t| !hod::target_supported(t)) {
        return Err(Fallback("unsupported-target".into()));
    }
    let forced = forced(options)?;
    let mut bridge = Bridge::new(module, forced.as_ref())?;
    let defined = bridge.defined().to_vec();
    let target = bridge.module.target().clone();
    let built = std::mem::replace(&mut bridge.module, hod_ir::Module::new(target));
    record.input_kind = Some(InputKind::Module);
    record.input_bytes = None;
    let mut session = match Session::with_module(built, options, settings.clone()) {
        Ok(session) => session,
        Err(error) => {
            return Ok((crate::finish_answer(Err(error), object, record), Report::default()));
        }
    };
    if session.end_module().is_ok() {
        for (fid, sym) in defined {
            // The body is built in the session's module (its types,
            // constants and inline assembly grow), lent to the bridge.
            std::mem::swap(&mut bridge.module, session.module());
            let built = bridge.body(fid, sym);
            std::mem::swap(&mut bridge.module, session.module());
            let fed = match built? {
                Built::Body(body) => session.body(sym, body),
                Built::Unsupported(constructs) => {
                    session.unsupported_body(sym, constructs.iter().map(|c| &**c))
                }
            };
            if fed.is_err() {
                break;
            }
        }
    }
    let (result, report) = match session.finish(object) {
        Ok(report) => (Ok(()), report),
        Err((error, report)) => (Err(error), report),
    };
    if report.target.is_some() {
        record.target.clone_from(&report.target);
    }
    let mut report = report;
    let mut reasons = std::mem::take(&mut report.reasons);
    if let Err(error) = &result
        && reasons.is_empty()
    {
        reasons.push(crate::reason_of(error).into_owned());
    }
    if settings.census {
        record.reasons = Some(reasons);
    }
    let answer = match result {
        Ok(()) => {
            record.outcome = Outcome::Hod;
            Answer::Done
        }
        Err(error) => {
            object.clear();
            crate::finish_answer(Err(error), object, record)
        }
    };
    Ok((answer, report))
}

/// The IR-equality check of `module`, as the log writes it: `equal`,
/// `refused:<reason>`, `different:<where>`, `reader:<error>`,
/// `fallback:<reason>` or `debug`.
pub(crate) fn equality(
    switches: &Switches,
    module: &kiln_llvm::Module,
    options: &Options,
) -> String {
    if !switches.bridge_debug && bridge::has_debug_info(module) {
        return "debug".to_owned();
    }
    let target = match forced(options) {
        Ok(target) => target,
        Err(fallback) => return format!("fallback:{fallback}"),
    };
    match eq::check(module, target.as_ref()) {
        Ok(eq::Verdict::Equal) => "equal".to_owned(),
        Ok(eq::Verdict::RefusedEqual(reason)) => format!("refused:{reason}"),
        Ok(eq::Verdict::Different(at)) => format!("different:{at}"),
        Ok(eq::Verdict::Reader(error)) => format!("reader:{error}"),
        Err(fallback) => format!("fallback:{fallback}"),
    }
}

/// Dual mode: `None` if the text path answered as the bridge did with
/// the same bytes; else what differs (the first differing section).
pub(crate) fn compare_objects(
    bridged: &Answer,
    object: &[u8],
    text: &Answer,
    other: &[u8],
) -> Option<String> {
    match (bridged, text) {
        (Answer::Done, Answer::Done) if object == other => None,
        (Answer::Done, Answer::Done) => Some(first_section(object, other)),
        (_, text) => Some(format!("the text path answered {text:?}")),
    }
}

/// The name of the first ELF section whose header or contents differ.
pub(crate) fn first_section(a: &[u8], b: &[u8]) -> String {
    let (Some(left), Some(right)) = (elf::sections(a), elf::sections(b)) else {
        return "not ELF".to_owned();
    };
    if left.len() != right.len() {
        return format!("the section count ({} / {})", left.len(), right.len());
    }
    for (l, r) in left.iter().zip(&right) {
        if l.header != r.header || l.contents != r.contents {
            return format!("section '{}'", l.name);
        }
    }
    "the file header".to_owned()
}

mod elf {
    /// A section of an ELF64 little-endian file.
    pub(super) struct Section<'a> {
        pub name: String,
        pub header: &'a [u8],
        pub contents: &'a [u8],
    }

    fn u16_at(b: &[u8], at: usize) -> Option<usize> {
        Some(usize::from(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?)))
    }

    fn u64_at(b: &[u8], at: usize) -> Option<usize> {
        usize::try_from(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?)).ok()
    }

    fn u32_at(b: &[u8], at: usize) -> Option<usize> {
        usize::try_from(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?)).ok()
    }

    pub(super) fn sections(b: &[u8]) -> Option<Vec<Section<'_>>> {
        if b.get(..6)? != b"\x7fELF\x02\x01" {
            return None;
        }
        let (shoff, entsize, count, names) =
            (u64_at(b, 0x28)?, u16_at(b, 0x3a)?, u16_at(b, 0x3c)?, u16_at(b, 0x3e)?);
        let header = |i: usize| b.get(shoff + i * entsize..shoff + (i + 1) * entsize);
        let names_header = header(names)?;
        let names_at = u64_at(names_header, 0x18)?;
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let h = header(i)?;
            let (name, kind) = (u32_at(h, 0)?, u32_at(h, 4)?);
            let (offset, size) = (u64_at(h, 0x18)?, u64_at(h, 0x20)?);
            // SHT_NOBITS has no contents in the file.
            let contents = if kind == 8 { &[][..] } else { b.get(offset..offset + size)? };
            let start = names_at + name;
            let end = b.get(start..)?.iter().position(|&c| c == 0)? + start;
            let name = String::from_utf8_lossy(&b[start..end]).into_owned();
            out.push(Section { name, header: h, contents });
        }
        Some(out)
    }
}

/// One line of `KILN_HOD_BRIDGE_LOG`.
#[derive(Debug)]
pub(crate) struct Line {
    pub output: String,
    pub route: &'static str,
    pub reason: Option<String>,
    /// hod's `Report.bridge`: the producer's time on the bridge.
    pub bridge_time: Option<Duration>,
    /// hod's `Report.session`: the time inside the session's calls.
    pub session_time: Option<Duration>,
    /// hod's phases, each with when it ended from the route's start.
    pub phases: Vec<(&'static str, Duration)>,
    pub eq: Option<String>,
    pub dual: Option<String>,
}

impl Line {
    pub(crate) fn new(output: String, route: &Route) -> Line {
        let (route, reason) = match *route {
            Route::Bridge => ("bridge", None),
            Route::Text(why) => ("text", Some(why.to_owned())),
        };
        Line {
            output,
            route,
            reason,
            bridge_time: None,
            session_time: None,
            phases: Vec::new(),
            eq: None,
            dual: None,
        }
    }

    /// Takes the times of hod's `report` (the session's on the bridge).
    pub(crate) fn report(&mut self, report: &Report, started: Instant) {
        if self.route == "bridge" {
            self.bridge_time = Some(report.bridge);
            self.session_time = Some(report.session);
        }
        self.phases = report
            .phases
            .iter()
            .map(|&(name, at)| (name, at.saturating_duration_since(started)))
            .collect();
    }
}

fn json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Appends `line` to the log, if one is named. A log that cannot be
/// written is ignored: it is a test's record, not the compile's.
pub(crate) fn log(switches: &Switches, line: &Line, started: Instant) {
    let Some(path) = &switches.log else { return };
    let mut out = String::from("{\"output\":");
    json_string(&mut out, &line.output);
    out.push_str(",\"route\":");
    json_string(&mut out, line.route);
    for (key, value) in [("reason", &line.reason), ("eq", &line.eq), ("dual", &line.dual)] {
        if let Some(value) = value {
            let _ = write!(out, ",\"{key}\":");
            json_string(&mut out, value);
        }
    }
    if let Some(bridge) = line.bridge_time {
        let _ = write!(out, ",\"bridge_us\":{}", bridge.as_micros());
    }
    if let Some(session) = line.session_time {
        let _ = write!(out, ",\"session_us\":{}", session.as_micros());
    }
    if !line.phases.is_empty() {
        out.push_str(",\"phases_us\":{");
        for (i, (name, at)) in line.phases.iter().enumerate() {
            let _ = write!(out, "{}\"{name}\":{}", if i == 0 { "" } else { "," }, at.as_micros());
        }
        out.push('}');
    }
    let _ = writeln!(out, ",\"total_us\":{}}}", started.elapsed().as_micros());
    let file = std::fs::OpenOptions::new().create(true).append(true).open(path);
    if let Ok(mut file) = file {
        let _ = file.write_all(out.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_strings_are_escaped() {
        let mut out = String::new();
        json_string(&mut out, "a\"b\\c\nd");
        assert_eq!(out, r#""a\"b\\c\u000ad""#);
    }

    /// A minimal ELF64 file: the null section, `.text` (4 bytes) and
    /// `.shstrtab`.
    fn elf(text: [u8; 4]) -> Vec<u8> {
        let names = b"\0.text\0.shstrtab\0";
        let mut b = vec![0u8; 64];
        b[..6].copy_from_slice(b"\x7fELF\x02\x01");
        let text_at = b.len();
        b.extend_from_slice(&text);
        let names_at = b.len();
        b.extend_from_slice(names);
        let shoff = b.len();
        b[0x28..0x30].copy_from_slice(&(shoff as u64).to_le_bytes());
        b[0x3a..0x3c].copy_from_slice(&64u16.to_le_bytes());
        b[0x3c..0x3e].copy_from_slice(&3u16.to_le_bytes());
        b[0x3e..0x40].copy_from_slice(&2u16.to_le_bytes());
        for (name, kind, at, size) in
            [(0u32, 0u32, 0usize, 0usize), (1, 1, text_at, 4), (7, 3, names_at, names.len())]
        {
            let mut h = [0u8; 64];
            h[0..4].copy_from_slice(&name.to_le_bytes());
            h[4..8].copy_from_slice(&kind.to_le_bytes());
            h[0x18..0x20].copy_from_slice(&(at as u64).to_le_bytes());
            h[0x20..0x28].copy_from_slice(&(size as u64).to_le_bytes());
            b.extend_from_slice(&h);
        }
        b
    }

    #[test]
    fn the_first_differing_section_is_named() {
        let (a, b) = (elf([1, 2, 3, 4]), elf([1, 2, 3, 5]));
        assert_eq!(first_section(&a, &b), "section '.text'");
        assert_eq!(first_section(&a, &a), "the file header");
        assert_eq!(first_section(&a, b"not an object"), "not ELF");
        assert!(compare_objects(&Answer::Done, &a, &Answer::Done, &a).is_none());
    }
}
