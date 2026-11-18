//! The bridge's specification as a check: for a module, what the bridge
//! builds against what hod's text reader builds from the module's text,
//! both printed by `hod_ir`'s printer, and the unsupported constructs
//! both recorded. No normalisation: the prints are compared byte for
//! byte.

use hod_ir::{Body, Dialect, Module, SymId, TargetSpec};
use hod_ll::{LlError, ParseOptions, ParsedBody};
use kiln_llvm as k;

use super::{Bridge, Built, Fallback};

/// The result of one comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The same module, bodies and record; nothing unsupported.
    Equal,
    /// The same, with something unsupported: the module is refused on
    /// both paths for this reason (the first construct recorded).
    RefusedEqual(String),
    /// The prints or the records differ: where first.
    Different(String),
    /// The reader failed on the text where the bridge did not.
    Reader(String),
}

/// What one path built: the module, the bodies, and the record.
struct Side {
    module: Module,
    bodies: Vec<Body>,
}

impl Side {
    fn print(&self) -> Result<String, String> {
        let mut out = String::new();
        self.module
            .print(&mut out, Dialect::LLVM22, &self.bodies)
            .map_err(|e| format!("print: {e}"))?;
        Ok(out)
    }

    fn record(&self) -> Vec<(String, Option<SymId>)> {
        self.module.unsupported().map(|u| (u.construct.to_owned(), u.symbol)).collect()
    }
}

/// The bridge's side.
fn bridged(m: &k::Module, target: Option<&TargetSpec>) -> Result<Side, Fallback> {
    let mut bridge = Bridge::new(m, target)?;
    let defined = bridge.defined().to_vec();
    let mut bodies = Vec::with_capacity(defined.len());
    for (fid, sym) in defined {
        if let Built::Body(body) = bridge.body(fid, sym)? {
            bodies.push(body);
        }
    }
    Ok(Side { module: bridge.module, bodies })
}

/// The reader's side, from `text`.
fn read(text: &[u8], target: Option<&TargetSpec>) -> Result<Side, LlError> {
    let options = ParseOptions { default_target: target.cloned() };
    let parsed = hod_ll::parse_module(text, &options)?;
    let (mut module, text) = (parsed.module, parsed.text);
    let mut bodies = Vec::new();
    hod_ll::parse_bodies(&mut module, &text, |_, read: ParsedBody<'_>| {
        if let Some(body) = read.body {
            bodies.push(body.clone());
        }
        Ok::<(), LlError>(())
    })?;
    Ok(Side { module, bodies })
}

/// Compares the bridge with the reader on `m`. `target` is the target of
/// a module that names none (`-mtriple`).
///
/// # Errors
/// The bridge does not take `m` (the module would take the text path).
pub fn check(m: &k::Module, target: Option<&TargetSpec>) -> Result<Verdict, Fallback> {
    let bridge = bridged(m, target)?;
    let text = m.to_text();
    let reader = match read(text.as_bytes(), target) {
        Ok(side) => side,
        Err(e) => return Ok(Verdict::Reader(e.to_string())),
    };
    // The printer refuses a module holding what it cannot print (an
    // unsupported construct): both refusing alike is equal, and the
    // records below say what was refused.
    let (left, right) = match (bridge.print(), reader.print()) {
        (Ok(l), Ok(r)) => (l, r),
        (Err(l), Err(r)) if l == r => (l, r),
        (l, r) => {
            return Ok(Verdict::Different(format!(
                "print: bridge {:?} / text {:?}",
                l.err(),
                r.err()
            )));
        }
    };
    if left != right {
        return Ok(Verdict::Different(first_difference(&left, &right)));
    }
    let (record, expected) = (bridge.record(), reader.record());
    if record != expected {
        let at = record.iter().zip(&expected).position(|(a, b)| a != b).unwrap_or(0);
        return Ok(Verdict::Different(format!(
            "record #{at}: bridge {:?} / text {:?}",
            record.get(at),
            expected.get(at)
        )));
    }
    Ok(match expected.first() {
        Some((construct, _)) => Verdict::RefusedEqual(construct.clone()),
        None => Verdict::Equal,
    })
}

/// The first line where `left` (the bridge's print) and `right` (the
/// reader's) differ, both quoted.
fn first_difference(left: &str, right: &str) -> String {
    let mut l = left.lines();
    let mut r = right.lines();
    let mut line = 1;
    loop {
        match (l.next(), r.next()) {
            (Some(a), Some(b)) if a == b => line += 1,
            (a, b) => {
                let clip = |s: Option<&str>| {
                    s.map_or_else(|| "<end>".to_owned(), |s| s.chars().take(160).collect())
                };
                return format!("line {line}: bridge `{}` / text `{}`", clip(a), clip(b));
            }
        }
    }
}
