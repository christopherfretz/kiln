//! The one rendering path of diagnostic records (spec/diag.md §10.3
//! "Rendering records"): the sink the driver gives an engine for its
//! output format, and the same sink over a string for records a caller
//! holds (an evaluation-session run's, spec/eval-session.md §7).

use std::io::Write;

use kiln_source::SourceManager;

use crate::diag::Diagnostic;
use crate::engine::DiagSink;
use crate::json::JsonSink;
use crate::options::{DiagOptions, Format};
use crate::text::TextSink;

/// The sink diagnostics are rendered with in `opts.format`, as the
/// driver's engines render them to standard error: a [`TextSink`] with
/// `opts`' display settings, after `prefix: ` when one is given (the
/// driver's own reports name the program), or a [`JsonSink`], which has
/// no prefix.
pub fn format_sink<'w, W: Write + Send + 'w>(
    out: W,
    opts: &DiagOptions,
    prefix: Option<&str>,
) -> Box<dyn DiagSink + Send + 'w> {
    match (opts.format, prefix) {
        (Format::Json, _) => Box::new(JsonSink::new(out, opts)),
        (Format::Text, None) => Box::new(TextSink::new(out, opts)),
        (Format::Text, Some(p)) => Box::new(TextSink::new(out, opts).with_prefix(p)),
    }
}

/// `records` rendered in order against `sm` exactly as an engine with
/// `opts` writes them when they reach its sink ([`format_sink`] without a
/// prefix): text or JSON by `opts.format`, every note (a call stack's
/// `in call to` frames included) under its diagnostic. One sink renders
/// the whole slice, so what the text form prints once per run of
/// diagnostics (an include stack) is printed as the driver would. The
/// summary line (`1 error generated.`) is the driver's, not a record's,
/// and is not part of it.
pub fn render_diagnostics(
    sm: &SourceManager,
    opts: &DiagOptions,
    records: &[Diagnostic],
) -> String {
    let mut out = Vec::new();
    {
        let mut sink = format_sink(&mut out, opts, None);
        for d in records {
            sink.emit(sm, d);
        }
        sink.flush();
    }
    String::from_utf8_lossy(&out).into_owned()
}
