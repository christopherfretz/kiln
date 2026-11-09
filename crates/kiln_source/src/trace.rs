//! The `-ftime-trace` recorder (spec/driver.md "Time trace"; the Oven's
//! events: spec/oven.md "Trace hook").
//!
//! It records Chrome trace events in the format clang's `-ftime-trace`
//! writes (LLVM's `TimeProfiler`): complete events (`"ph":"X"`) with a
//! name, a detail and args; the async `Source` events of included
//! headers (`"ph":"b"` / `"e"`); per-name `Total <name>` summaries; the
//! process and thread name metadata.
//!
//! **Thread-confined.** Kiln compiles a translation unit on one thread,
//! so the recorder is thread-local: nothing is locked, and in-process
//! tests that compile in parallel never see each other's events. A span
//! begun on one thread must end on the same thread.
//!
//! **Off cost.** With no trace running, [`begin`] and [`Scope::new`] read
//! one thread-local [`Cell`] and return `None`: no clock read, no
//! allocation. Hot callers (the Oven's per-call hook) cache [`enabled`]
//! in a field so the check is one predictable branch on a plain bool.
//!
//! **Granularity.** As clang: an event shorter than the granularity
//! (default 500 µs) is dropped, but still counted in its name's total.
//! A total counts only the outermost of nested events of one name (an
//! instantiation inside an instantiation is charged once).
//!
//! The details are produced lazily ([`end`] takes a closure), so an event
//! that is dropped costs two clock reads and nothing else.
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// clang's default `-ftime-trace-granularity` (µs).
pub const DEFAULT_GRANULARITY_US: u64 = 500;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static VERBOSE: Cell<bool> = const { Cell::new(false) };
    static REC: RefCell<Option<Recorder>> = const { RefCell::new(None) };
}

/// Whether a trace is being recorded on this thread.
#[inline]
pub fn enabled() -> bool {
    ON.with(Cell::get)
}

/// Whether the running trace is verbose (`-ftime-trace-verbose`: extra
/// args, and the Oven's per-call events).
#[inline]
pub fn verbose() -> bool {
    VERBOSE.with(Cell::get)
}

/// An argument value of an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    /// A string.
    Str(String),
    /// An integer.
    Int(i64),
}

/// How an event is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// A complete event (`"ph":"X"`).
    Complete,
    /// An async begin/end pair (`"ph":"b"` then `"ph":"e"`), category =
    /// name (clang's `Source`).
    Async,
}

/// One recorded event.
#[derive(Clone, Debug)]
pub struct Event {
    /// The event name (`InstantiateClass`, `EvaluateAsConstantExpr`).
    pub name: Cow<'static, str>,
    /// The `detail` arg (omitted when empty).
    pub detail: String,
    /// Further args, after `detail`.
    pub args: Vec<(Cow<'static, str>, Arg)>,
    /// Start, µs since the trace began.
    pub ts: u64,
    /// Duration, µs.
    pub dur: u64,
    /// Complete or async.
    pub kind: EventKind,
}

/// The per-name summary (`Total <name>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Total {
    /// The event name.
    pub name: Cow<'static, str>,
    /// Outermost events of this name.
    pub count: u64,
    /// Their summed duration, µs.
    pub dur: u64,
}

/// An open span: what [`begin`] returns and [`end`] takes.
#[derive(Clone, Copy, Debug)]
pub struct Span {
    start: Instant,
    name: &'static str,
}

impl Span {
    /// When the span began.
    pub fn start(&self) -> Instant {
        self.start
    }

    /// The span's event name.
    pub fn name(&self) -> &'static str {
        self.name
    }
}

/// One name's `Total`: the spans of it open now, and the outermost
/// ones closed so far.
struct NameTotal {
    name: Cow<'static, str>,
    open: u32,
    count: u64,
    dur: u64,
}

struct Recorder {
    start: Instant,
    begin_us: u64,
    granularity: u64,
    events: Vec<Event>,
    totals: Vec<NameTotal>,
    sources: Vec<(Instant, String)>,
}

impl Recorder {
    fn slot(&mut self, name: &str) -> &mut NameTotal {
        let i = self.totals.iter().position(|t| t.name == name).unwrap_or_else(|| {
            self.totals.push(NameTotal {
                name: Cow::Owned(name.to_owned()),
                open: 0,
                count: 0,
                dur: 0,
            });
            self.totals.len() - 1
        });
        &mut self.totals[i]
    }

    fn open(&mut self, name: &'static str) {
        let i = self.totals.iter().position(|t| t.name == name).unwrap_or_else(|| {
            self.totals.push(NameTotal { name: Cow::Borrowed(name), open: 0, count: 0, dur: 0 });
            self.totals.len() - 1
        });
        self.totals[i].open += 1;
    }

    /// Closes one `name` span of `dur` µs: charged to the total when no
    /// other `name` span is open. Returns whether the event is kept.
    fn close(&mut self, name: &str, dur: u64) -> bool {
        let g = self.granularity;
        let t = self.slot(name);
        t.open = t.open.saturating_sub(1);
        if t.open == 0 {
            t.count += 1;
            t.dur = t.dur.saturating_add(dur);
        }
        dur >= g
    }

    fn us_since_start(&self, t: Instant) -> u64 {
        micros(t.saturating_duration_since(self.start))
    }
}

fn micros(d: std::time::Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

/// Starts recording on this thread (replacing a trace already running):
/// events shorter than `granularity_us` are dropped; `verbose` asks for
/// the extra events and args of `-ftime-trace-verbose`.
pub fn start(granularity_us: u64, verbose: bool) {
    let begin_us = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX));
    REC.with(|r| {
        *r.borrow_mut() = Some(Recorder {
            start: Instant::now(),
            begin_us,
            granularity: granularity_us,
            events: Vec::new(),
            totals: Vec::new(),
            sources: Vec::new(),
        });
    });
    ON.with(|c| c.set(true));
    VERBOSE.with(|c| c.set(verbose));
}

/// Stops recording on this thread and returns the trace (`None` if none
/// was running). Spans still open are not recorded.
pub fn finish() -> Option<Trace> {
    ON.with(|c| c.set(false));
    VERBOSE.with(|c| c.set(false));
    let r = REC.with(|r| r.borrow_mut().take())?;
    let mut totals: Vec<Total> = r
        .totals
        .into_iter()
        .filter(|t| t.count > 0)
        .map(|t| Total { name: t.name, count: t.count, dur: t.dur })
        .collect();
    totals.sort_by(|a, b| b.dur.cmp(&a.dur).then_with(|| a.name.cmp(&b.name)));
    Some(Trace { begin_us: r.begin_us, granularity: r.granularity, events: r.events, totals })
}

/// Begins a `name` span, or returns `None` when no trace is running (one
/// thread-local read: no clock read, no allocation).
#[inline]
pub fn begin(name: &'static str) -> Option<Span> {
    if enabled() { begin_on(name) } else { None }
}

/// [`begin`] without the enabled check (the caller knows a trace runs;
/// still `None` if it does not).
#[inline(never)]
pub fn begin_on(name: &'static str) -> Option<Span> {
    REC.with(|r| {
        let mut r = r.borrow_mut();
        let r = r.as_mut()?;
        r.open(name);
        Some(Span { start: Instant::now(), name })
    })
}

/// Ends `span`: recorded, with the detail `detail()`, when it lasted at
/// least the granularity.
pub fn end(span: Span, detail: impl FnOnce() -> String) {
    end_args(span, detail, Vec::new);
}

/// [`end`] with further args (`args()` runs only for a recorded event).
pub fn end_args(
    span: Span,
    detail: impl FnOnce() -> String,
    args: impl FnOnce() -> Vec<(Cow<'static, str>, Arg)>,
) {
    end_at(span, Instant::now(), detail, args);
}

/// [`end_args`] at `now` (a clock reading the caller already has).
pub fn end_at(
    span: Span,
    now: Instant,
    detail: impl FnOnce() -> String,
    args: impl FnOnce() -> Vec<(Cow<'static, str>, Arg)>,
) {
    let dur = micros(now.saturating_duration_since(span.start));
    // Decide under the borrow, format outside it: a detail may read the
    // recorder's state (`enabled`) but must never find it borrowed.
    let keep = REC.with(|r| {
        let mut r = r.borrow_mut();
        let r = r.as_mut()?;
        r.close(span.name, dur).then(|| r.us_since_start(span.start))
    });
    let Some(ts) = keep else { return };
    let ev = Event {
        name: Cow::Borrowed(span.name),
        detail: detail(),
        args: args(),
        ts,
        dur,
        kind: EventKind::Complete,
    };
    push(ev);
}

fn push(ev: Event) {
    REC.with(|r| {
        if let Some(r) = r.borrow_mut().as_mut() {
            r.events.push(ev);
        }
    });
}

/// Records a complete `name` event that ran from `start` to `end`,
/// counted in the totals as an outermost one (for phases timed outside
/// the recorder, such as a child process's run).
pub fn complete(name: &'static str, start: Instant, end: Instant, detail: impl FnOnce() -> String) {
    if !enabled() {
        return;
    }
    if let Some(mut s) = begin_on(name) {
        s.start = start;
        end_at(s, end, detail, Vec::new);
    }
}

/// Begins the async `Source` span of an included header (clang's
/// `Source` event; `detail`: the header's path as found).
pub fn source_begin(detail: impl FnOnce() -> String) {
    if !enabled() {
        return;
    }
    let d = detail();
    REC.with(|r| {
        if let Some(r) = r.borrow_mut().as_mut() {
            r.open("Source");
            r.sources.push((Instant::now(), d));
        }
    });
}

/// Ends the innermost `Source` span.
pub fn source_end() {
    if !enabled() {
        return;
    }
    let now = Instant::now();
    REC.with(|r| {
        let mut r = r.borrow_mut();
        let Some(r) = r.as_mut() else { return };
        let Some((start, detail)) = r.sources.pop() else { return };
        let dur = micros(now.saturating_duration_since(start));
        if r.close("Source", dur) {
            let ts = r.us_since_start(start);
            r.events.push(Event {
                name: Cow::Borrowed("Source"),
                detail,
                args: Vec::new(),
                ts,
                dur,
                kind: EventKind::Async,
            });
        }
    });
}

/// `loc` as clang's `SourceLocation::printToString` writes it in a
/// trace detail: `file:line:col` at the presumed position (`<invalid
/// loc>` when it has none).
pub fn loc_detail(sm: &crate::SourceManager, loc: crate::Loc) -> String {
    match sm.presumed_line_col(loc) {
        Some(p) => format!("{}:{}:{}", p.file_name, p.line, p.col),
        None => "<invalid loc>".to_owned(),
    }
}

/// The range `begin`..`end` (`end` the last token's location) as clang's
/// `SourceRange::printToString` writes it in a trace detail:
/// `<file:line:col>` for one location, `<file:line:col, col:c>` on one
/// line, `<file:line:col, line:l:c>` across lines.
pub fn range_detail(sm: &crate::SourceManager, begin: crate::Loc, end: crate::Loc) -> String {
    let Some(b) = sm.presumed_line_col(begin) else { return "<<invalid sloc>>".to_owned() };
    let mut s = format!("<{}:{}:{}", b.file_name, b.line, b.col);
    if end != begin {
        match sm.presumed_line_col(end) {
            Some(e) if e.file_name != b.file_name => {
                let _ = write!(s, ", {}:{}:{}", e.file_name, e.line, e.col);
            }
            Some(e) if e.line != b.line => {
                let _ = write!(s, ", line:{}:{}", e.line, e.col);
            }
            Some(e) if e.col != b.col => {
                let _ = write!(s, ", col:{}", e.col);
            }
            Some(_) => {}
            None => s.push_str(", <invalid sloc>"),
        }
    }
    s.push('>');
    s
}

/// A scoped span: begun by [`Scope::new`], ended when dropped.
#[derive(Debug)]
#[must_use = "the span ends when the scope is dropped"]
pub struct Scope {
    span: Option<Span>,
    detail: String,
}

impl Scope {
    /// Begins a `name` span with no detail (`None` inside when no trace
    /// runs).
    #[inline]
    pub fn new(name: &'static str) -> Scope {
        Scope { span: begin(name), detail: String::new() }
    }

    /// Begins a `name` span; `detail()` is computed now, and only when a
    /// trace runs.
    #[inline]
    pub fn with_detail(name: &'static str, detail: impl FnOnce() -> String) -> Scope {
        if !enabled() {
            return Scope { span: None, detail: String::new() };
        }
        let detail = detail();
        Scope { span: begin_on(name), detail }
    }

    /// Whether the scope is recording.
    pub fn active(&self) -> bool {
        self.span.is_some()
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(s) = self.span.take() {
            let d = std::mem::take(&mut self.detail);
            end(s, || d);
        }
    }
}

/// A finished trace.
#[derive(Clone, Debug)]
pub struct Trace {
    /// Wall-clock start, µs since the Unix epoch (`beginningOfTime`).
    pub begin_us: u64,
    /// The granularity it was recorded with, µs.
    pub granularity: u64,
    /// The events, in completion order.
    pub events: Vec<Event>,
    /// The totals, longest first.
    pub totals: Vec<Total>,
}

impl Trace {
    /// Merges the trace an LLVM tool wrote (`opt`/`llc --time-trace`):
    /// its events, moved to this trace's time base (by the two
    /// `beginningOfTime`s) and filtered by this trace's granularity; its
    /// totals added to these. Its metadata is dropped.
    ///
    /// # Errors
    ///
    /// The text is not a JSON object with a `traceEvents` array.
    pub fn merge_child(&mut self, text: &str) -> Result<(), String> {
        let v = Json::parse(text)?;
        let evs = v.get("traceEvents").and_then(Json::as_array).ok_or("no traceEvents")?;
        let child_begin = v.get("beginningOfTime").and_then(Json::as_u64).unwrap_or(self.begin_us);
        let shift = i128::from(child_begin) - i128::from(self.begin_us);
        let mut open: Vec<(String, u64, String)> = Vec::new();
        for e in evs {
            let name = e.get("name").and_then(Json::as_str).unwrap_or_default();
            let ph = e.get("ph").and_then(Json::as_str).unwrap_or_default();
            let ts = e.get("ts").and_then(Json::as_u64).unwrap_or(0);
            let dur = e.get("dur").and_then(Json::as_u64).unwrap_or(0);
            let ts = u64::try_from((i128::from(ts) + shift).max(0)).unwrap_or(0);
            let detail = e
                .get("args")
                .and_then(|a| a.get("detail"))
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_owned();
            let total = name.strip_prefix("Total ");
            match ph {
                "X" if total.is_some() => {
                    let n = total.unwrap_or_default();
                    let count = e
                        .get("args")
                        .and_then(|a| a.get("count"))
                        .and_then(Json::as_u64)
                        .unwrap_or(1);
                    self.add_total(n, count, dur);
                }
                "X" => {
                    if dur < self.granularity {
                        continue;
                    }
                    let args = e.get("args").map(child_args).unwrap_or_default();
                    self.events.push(Event {
                        name: Cow::Owned(name.to_owned()),
                        detail,
                        args,
                        ts,
                        dur,
                        kind: EventKind::Complete,
                    });
                }
                "b" => open.push((name.to_owned(), ts, detail)),
                "e" => {
                    if let Some(i) = open.iter().rposition(|o| o.0 == name) {
                        let (n, start, detail) = open.remove(i);
                        let dur = ts.saturating_sub(start);
                        if dur >= self.granularity {
                            self.events.push(Event {
                                name: Cow::Owned(n),
                                detail,
                                args: Vec::new(),
                                ts: start,
                                dur,
                                kind: EventKind::Async,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        self.totals.sort_by(|a, b| b.dur.cmp(&a.dur).then_with(|| a.name.cmp(&b.name)));
        Ok(())
    }

    fn add_total(&mut self, name: &str, count: u64, dur: u64) {
        if let Some(t) = self.totals.iter_mut().find(|t| t.name == name) {
            t.count += count;
            t.dur = t.dur.saturating_add(dur);
        } else {
            self.totals.push(Total { name: Cow::Owned(name.to_owned()), count, dur });
        }
    }

    /// The trace as clang writes it: `{"traceEvents":[...],
    /// "beginningOfTime":N}`, the events, then the totals (on threads
    /// `tid + 1`, `tid + 2`, ...), then the `process_name` and
    /// `thread_name` metadata.
    pub fn to_json(&self, pid: u32, process: &str, thread: &str) -> String {
        let mut o = String::with_capacity(128 + self.events.len() * 96);
        o.push_str("{\"traceEvents\":[");
        let mut first = true;
        let mut sep = |o: &mut String| {
            if !std::mem::take(&mut first) {
                o.push(',');
            }
        };
        let tid = pid;
        for e in &self.events {
            match e.kind {
                EventKind::Complete => {
                    sep(&mut o);
                    let _ = write!(
                        o,
                        "{{\"pid\":{pid},\"tid\":{tid},\"ts\":{},\"ph\":\"X\",\"dur\":{},\"name\":",
                        e.ts, e.dur
                    );
                    json_str(&mut o, &e.name);
                    write_args(&mut o, &e.detail, &e.args);
                    o.push('}');
                }
                EventKind::Async => {
                    for (ph, ts) in [("b", e.ts), ("e", e.ts.saturating_add(e.dur))] {
                        sep(&mut o);
                        let _ = write!(o, "{{\"pid\":{pid},\"tid\":{tid},\"ts\":{ts},\"cat\":");
                        json_str(&mut o, &e.name);
                        let _ = write!(o, ",\"ph\":\"{ph}\",\"id\":0,\"name\":");
                        json_str(&mut o, &e.name);
                        if ph == "b" {
                            write_args(&mut o, &e.detail, &e.args);
                        }
                        o.push('}');
                    }
                }
            }
        }
        for (i, t) in self.totals.iter().enumerate() {
            sep(&mut o);
            let total_tid = u64::from(tid) + 1 + i as u64;
            let avg_ms = t.dur / t.count.max(1) / 1000;
            let _ = write!(
                o,
                "{{\"pid\":{pid},\"tid\":{total_tid},\"ph\":\"X\",\"ts\":0,\"dur\":{},\"name\":",
                t.dur
            );
            json_str(&mut o, &format!("Total {}", t.name));
            let _ = write!(o, ",\"args\":{{\"count\":{},\"avg ms\":{avg_ms}}}}}", t.count);
        }
        for (what, name) in [("process_name", process), ("thread_name", thread)] {
            sep(&mut o);
            let _ = write!(
                o,
                "{{\"cat\":\"\",\"pid\":{pid},\"tid\":{tid},\"ts\":0,\"ph\":\"M\",\"name\":\"{what}\",\"args\":{{\"name\":"
            );
            json_str(&mut o, name);
            o.push_str("}}");
        }
        let _ = write!(o, "],\"beginningOfTime\":{}}}", self.begin_us);
        o
    }
}

fn child_args(a: &Json) -> Vec<(Cow<'static, str>, Arg)> {
    let Json::Obj(fields) = a else { return Vec::new() };
    fields
        .iter()
        .filter(|(k, _)| k != "detail")
        .filter_map(|(k, v)| {
            let v = match v {
                Json::Str(s) => Arg::Str(s.clone()),
                Json::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => Arg::Int(*n as i64),
                _ => return None,
            };
            Some((Cow::Owned(k.clone()), v))
        })
        .collect()
}

fn write_args(o: &mut String, detail: &str, args: &[(Cow<'static, str>, Arg)]) {
    if detail.is_empty() && args.is_empty() {
        return;
    }
    o.push_str(",\"args\":{");
    let mut first = true;
    if !detail.is_empty() {
        o.push_str("\"detail\":");
        json_str(o, detail);
        first = false;
    }
    for (k, v) in args {
        if !std::mem::take(&mut first) {
            o.push(',');
        }
        json_str(o, k);
        o.push(':');
        match v {
            Arg::Str(s) => json_str(o, s),
            Arg::Int(n) => {
                let _ = write!(o, "{n}");
            }
        }
    }
    o.push('}');
}

/// Appends `s` as a JSON string literal.
pub fn json_str(o: &mut String, s: &str) {
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
}

/// A parsed JSON value: enough to read a trace back (merging an LLVM
/// tool's trace; the tests' structural checks).
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// A number.
    Num(f64),
    /// A string.
    Str(String),
    /// An array.
    Arr(Vec<Json>),
    /// An object, its members in order.
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// Parses `text` (one value, surrounded by optional whitespace).
    ///
    /// # Errors
    ///
    /// The text is not JSON (RFC 8259), or nests deeper than 256.
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut p = Parser { b: text.as_bytes(), i: 0, depth: 0 };
        let v = p.value()?;
        p.ws();
        if p.i != p.b.len() {
            return Err(format!("trailing characters at byte {}", p.i));
        }
        Ok(v)
    }

    /// The member `key` of an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The elements of an array.
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    /// A string's text.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    /// A non-negative integral number.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Num(n) if *n >= 0.0 && n.fract() == 0.0 && *n < 1.8e19 => Some(*n as u64),
            _ => None,
        }
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
    depth: u32,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.b.get(self.i).is_some_and(|c| matches!(c, b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{what} at byte {}", self.i))
    }

    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        let Some(&c) = self.b.get(self.i) else { return self.err("unexpected end") };
        match c {
            b'{' | b'[' => {
                self.depth += 1;
                if self.depth > 256 {
                    return self.err("nesting too deep");
                }
                self.i += 1;
                let v = if c == b'{' { self.object()? } else { self.array()? };
                self.depth -= 1;
                Ok(v)
            }
            b'"' => Ok(Json::Str(self.string()?)),
            b't' => self.word("true", Json::Bool(true)),
            b'f' => self.word("false", Json::Bool(false)),
            b'n' => self.word("null", Json::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => self.err("unexpected character"),
        }
    }

    fn word(&mut self, w: &str, v: Json) -> Result<Json, String> {
        if self.b[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            Ok(v)
        } else {
            self.err("bad literal")
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        let mut m = Vec::new();
        if self.eat(b'}') {
            return Ok(Json::Obj(m));
        }
        loop {
            self.ws();
            if self.b.get(self.i) != Some(&b'"') {
                return self.err("expected a member name");
            }
            let k = self.string()?;
            if !self.eat(b':') {
                return self.err("expected ':'");
            }
            let v = self.value()?;
            m.push((k, v));
            if self.eat(b',') {
                continue;
            }
            if self.eat(b'}') {
                return Ok(Json::Obj(m));
            }
            return self.err("expected ',' or '}'");
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        let mut a = Vec::new();
        if self.eat(b']') {
            return Ok(Json::Arr(a));
        }
        loop {
            a.push(self.value()?);
            if self.eat(b',') {
                continue;
            }
            if self.eat(b']') {
                return Ok(Json::Arr(a));
            }
            return self.err("expected ',' or ']'");
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let h = self.b.get(self.i..self.i + 4).ok_or("short \\u escape")?;
        let s = std::str::from_utf8(h).map_err(|_| "bad \\u escape")?;
        let v = u32::from_str_radix(s, 16).map_err(|_| "bad \\u escape")?;
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1; // the opening quote
        let mut s = String::new();
        loop {
            let start = self.i;
            while self.b.get(self.i).is_some_and(|&c| c != b'"' && c != b'\\' && c >= 0x20) {
                self.i += 1;
            }
            s.push_str(std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "invalid UTF-8")?);
            match self.b.get(self.i) {
                Some(b'"') => {
                    self.i += 1;
                    return Ok(s);
                }
                Some(b'\\') => {
                    self.i += 1;
                    let Some(&e) = self.b.get(self.i) else {
                        return self.err("unterminated string");
                    };
                    self.i += 1;
                    match e {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'b' => s.push('\u{8}'),
                        b'f' => s.push('\u{c}'),
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'u' => {
                            let mut c = self.hex4()?;
                            if (0xd800..0xdc00).contains(&c) && self.b[self.i..].starts_with(b"\\u")
                            {
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&lo) {
                                    return self.err("bad surrogate pair");
                                }
                                c = 0x10000 + ((c - 0xd800) << 10) + (lo - 0xdc00);
                            }
                            s.push(char::from_u32(c).ok_or("bad \\u escape")?);
                        }
                        _ => return self.err("bad escape"),
                    }
                }
                _ => return self.err("unterminated string"),
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        self.i += usize::from(self.b[self.i] == b'-');
        let digits = |p: &mut Self| {
            let s = p.i;
            while p.b.get(p.i).is_some_and(u8::is_ascii_digit) {
                p.i += 1;
            }
            p.i > s
        };
        if !digits(self) {
            return self.err("bad number");
        }
        if self.b.get(self.i) == Some(&b'.') {
            self.i += 1;
            if !digits(self) {
                return self.err("bad number");
            }
        }
        if self.b.get(self.i).is_some_and(|c| matches!(c, b'e' | b'E')) {
            self.i += 1;
            if self.b.get(self.i).is_some_and(|c| matches!(c, b'+' | b'-')) {
                self.i += 1;
            }
            if !digits(self) {
                return self.err("bad number");
            }
        }
        let t = std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "bad number")?;
        t.parse().map(Json::Num).map_err(|_| format!("bad number at byte {start}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_records_nothing() {
        assert!(!enabled());
        assert!(begin("X").is_none());
        let s = Scope::new("X");
        assert!(!s.active());
        drop(s);
        assert!(finish().is_none());
    }

    #[test]
    fn granularity_drops_short_events_but_totals_count_them() {
        start(1_000_000, false);
        let outer = begin("Outer").expect("on");
        for _ in 0..3 {
            let s = begin("Inner").expect("on");
            end(s, || unreachable!("a dropped event formats no detail"));
        }
        end(outer, String::new);
        let t = finish().expect("trace");
        assert!(t.events.is_empty());
        let inner = t.totals.iter().find(|t| t.name == "Inner").expect("total");
        assert_eq!(inner.count, 3);
        assert!(!enabled());
    }

    #[test]
    fn nested_same_name_counts_once() {
        start(0, false);
        let a = begin("I").expect("on");
        let b = begin("I").expect("on");
        end(b, || "inner".into());
        end(a, || "outer".into());
        let t = finish().expect("trace");
        assert_eq!(t.events.len(), 2);
        assert_eq!(t.events[0].detail, "inner");
        assert_eq!(t.totals[0].count, 1);
    }

    #[test]
    fn json_round_trips_and_has_clang_shape() {
        start(0, false);
        {
            let _s = Scope::with_detail("ParseClass", || "A \"q\"\n".into());
            source_begin(|| "./h.h".into());
            source_end();
        }
        let mut t = finish().expect("trace");
        let child = format!(
            r#"{{"traceEvents":[{{"pid":1,"tid":1,"ts":5,"ph":"X","dur":7,"name":"RunPass","args":{{"detail":"SROAPass"}}}},{{"pid":1,"tid":2,"ph":"X","ts":0,"dur":7,"name":"Total RunPass","args":{{"count":1,"avg ms":0}}}},{{"cat":"","pid":1,"tid":1,"ts":0,"ph":"M","name":"process_name","args":{{"name":"opt"}}}}],"beginningOfTime":{}}}"#,
            t.begin_us + 10
        );
        t.merge_child(&child).expect("child");
        let j = Json::parse(&t.to_json(42, "kiln", "kiln++")).expect("valid JSON");
        let evs = j.get("traceEvents").and_then(Json::as_array).expect("events");
        let names: Vec<&str> =
            evs.iter().filter_map(|e| e.get("name").and_then(Json::as_str)).collect();
        for n in ["ParseClass", "Source", "RunPass", "Total RunPass", "Total ParseClass"] {
            assert!(names.contains(&n), "{n} missing from {names:?}");
        }
        let pass = evs
            .iter()
            .find(|e| e.get("name").and_then(Json::as_str) == Some("RunPass"))
            .expect("pass");
        assert_eq!(pass.get("ts").and_then(Json::as_u64), Some(15));
        let class = evs
            .iter()
            .find(|e| e.get("name").and_then(Json::as_str) == Some("ParseClass"))
            .expect("class");
        assert_eq!(
            class.get("args").and_then(|a| a.get("detail")).and_then(Json::as_str),
            Some("A \"q\"\n")
        );
        let meta = evs.last().expect("metadata");
        assert_eq!(meta.get("ph").and_then(Json::as_str), Some("M"));
        assert!(j.get("beginningOfTime").and_then(Json::as_u64).is_some());
    }

    #[test]
    fn parser_rejects_malformed() {
        for bad in ["", "{", "[1,]", "{\"a\" 1}", "\"\\x\"", "01x", "[1] 2"] {
            assert!(Json::parse(bad).is_err(), "{bad:?}");
        }
        assert_eq!(Json::parse(" \"\\ud83d\\ude00\" ").unwrap(), Json::Str("😀".into()));
    }
}
