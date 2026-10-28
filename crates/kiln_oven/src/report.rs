//! The `-fconstexpr-report=` writer (spec/oven.md "Reporting protocol"):
//! NDJSON, one line per top-level evaluation, then one `total` line.

use std::fmt::Write as _;

use kiln_source::Loc;

use crate::stats::EvalStats;

/// Renders the report of `stats`. `loc` maps a location to its
/// `file:line:col` spelling (`None`: the line's `loc` is `null`).
pub fn render(stats: &EvalStats, loc: &dyn Fn(Loc) -> Option<String>) -> String {
    let mut s = String::new();
    for r in stats.records() {
        s.push_str("{\"loc\":");
        match loc(r.loc) {
            Some(l) => json_string(&l, &mut s),
            None => s.push_str("null"),
        }
        let _ = writeln!(
            s,
            ",\"kind\":\"{}\",\"steps\":{},\"bytes\":{},\"peak_bytes\":{},\"calls\":{},\"allocs\":{},\"wall_us\":{}}}",
            r.kind.as_str(),
            r.steps,
            r.bytes,
            r.peak_bytes,
            r.calls,
            r.allocs,
            r.wall_us
        );
    }
    let t = stats.totals();
    let _ = writeln!(
        s,
        "{{\"total\":{{\"steps\":{},\"bytes\":{},\"peak_bytes\":{},\"calls\":{},\"allocs\":{},\"wall_us\":{},\"evaluations\":{}}}}}",
        t.steps, t.bytes, t.peak_bytes, t.calls, t.allocs, t.wall_us, t.evaluations
    );
    s
}

/// Writes the report of `stats` to `path` (created or truncated).
///
/// # Errors
/// If the file cannot be written.
pub fn write(
    stats: &EvalStats,
    loc: &dyn Fn(Loc) -> Option<String>,
    path: &std::path::Path,
) -> std::io::Result<()> {
    std::fs::write(path, render(stats, loc))
}

fn json_string(v: &str, out: &mut String) {
    out.push('"');
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
