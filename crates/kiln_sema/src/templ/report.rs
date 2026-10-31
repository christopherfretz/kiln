//! The template instantiation report (spec/sema-templates.md §11.3):
//! per-template rows of instantiation counts, retained AST nodes and
//! bytes, and self/inclusive time, printed by
//! `-fkiln-instantiation-report[=all|json]`.

use std::fmt::Write as _;

use kiln_ast::DeclId;
use kiln_source::fx::FxHashMap;

use super::stack::FrameKind;
use crate::Sema;

/// What a report row aggregates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RowKind {
    /// Class template specializations.
    Class,
    /// Function template specializations.
    Function,
    /// Variable template specializations.
    Var,
    /// Member functions, member classes, enumerations and default member
    /// initializers of specializations.
    Member,
    /// Static data members of specializations.
    Static,
    /// Alias template specializations.
    Alias,
    /// Default arguments.
    Default,
}

impl RowKind {
    /// The report's `kind` column.
    pub const fn name(self) -> &'static str {
        match self {
            RowKind::Class => "class",
            RowKind::Function => "function",
            RowKind::Var => "var",
            RowKind::Member => "member",
            RowKind::Static => "static",
            RowKind::Alias => "alias",
            RowKind::Default => "default",
        }
    }

    /// The row kind a frame charges.
    pub const fn of(k: FrameKind) -> RowKind {
        match k {
            FrameKind::ClassInstantiation => RowKind::Class,
            FrameKind::FunctionSpec => RowKind::Function,
            FrameKind::StaticDataMember => RowKind::Static,
            FrameKind::AliasTemplate => RowKind::Alias,
            FrameKind::VarTemplateInstantiation | FrameKind::VarTemplateInit => RowKind::Var,
            FrameKind::MemberClass
            | FrameKind::MemberFunction
            | FrameKind::ExceptionSpec
            | FrameKind::Enum
            | FrameKind::Nsdmi => RowKind::Member,
            FrameKind::DefaultTemplateArg
            | FrameKind::DefaultFunctionArg
            | FrameKind::DefaultArgChecking
            | FrameKind::PartialSpecDeduction
            | FrameKind::DeducedArgSubstitution
            | FrameKind::ExplicitArgSubstitution
            | FrameKind::ConstraintsCheck
            | FrameKind::ConceptCheck
            | FrameKind::ConstraintSubstitution => RowKind::Default,
        }
    }
}

/// One row: every instantiation of `template` of one kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstRow {
    /// The primary template, or the member's pattern.
    pub template: DeclId,
    /// What was instantiated.
    pub kind: RowKind,
    /// Number of instantiations.
    pub count: u32,
    /// AST nodes created (self: nested instantiations excluded).
    pub nodes: u64,
    /// AST bytes retained (self).
    pub bytes: u64,
    /// Time spent, nested instantiations excluded.
    pub self_ns: u64,
    /// Time spent, nested instantiations included.
    pub incl_ns: u64,
}

/// The report's output format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ReportFormat {
    /// No report.
    #[default]
    Off,
    /// The top 50 rows as text.
    Text,
    /// Every row as text.
    All,
    /// NDJSON.
    Json,
}

/// The per-template counters (always on; cheap).
#[derive(Debug, Default)]
pub struct InstReport {
    /// What `-fkiln-instantiation-report` asked for.
    pub format: ReportFormat,
    rows: Vec<InstRow>,
    index: FxHashMap<(DeclId, RowKind), u32>,
    /// Bytes still held by frames' children: the self deltas subtract
    /// them (one entry per open frame, see [`InstReport::charge`]).
    child_bytes: Vec<(u64, u64)>,
    /// Total instantiations.
    pub count: u64,
}

impl InstReport {
    /// Whether frames are timed (only when a report is printed).
    pub fn timed(&self) -> bool {
        self.format != ReportFormat::Off
    }

    /// Opens a frame's self accounting (its children's bytes and nodes).
    pub(crate) fn open(&mut self) {
        self.child_bytes.push((0, 0));
    }

    /// Charges a popped frame: `nodes`, `bytes` and `incl_ns` are
    /// inclusive; `child_ns` is its children's inclusive time.
    pub(crate) fn charge(
        &mut self,
        kind: FrameKind,
        template: DeclId,
        nodes: u64,
        bytes: u64,
        incl_ns: u64,
        child_ns: u64,
    ) {
        let (cb, cn) = self.child_bytes.pop().unwrap_or((0, 0));
        if let Some(p) = self.child_bytes.last_mut() {
            p.0 = p.0.saturating_add(bytes);
            p.1 = p.1.saturating_add(nodes);
        }
        let counted = kind != FrameKind::VarTemplateInit;
        let kind = RowKind::of(kind);
        let i = if let Some(&i) = self.index.get(&(template, kind)) {
            i as usize
        } else {
            let i = self.rows.len();
            self.rows.push(InstRow {
                template,
                kind,
                count: 0,
                nodes: 0,
                bytes: 0,
                self_ns: 0,
                incl_ns: 0,
            });
            let _ = self.index.insert((template, kind), i as u32);
            i
        };
        let r = &mut self.rows[i];
        r.count = r.count.saturating_add(u32::from(counted));
        r.nodes = r.nodes.saturating_add(nodes.saturating_sub(cn));
        r.bytes = r.bytes.saturating_add(bytes.saturating_sub(cb));
        r.incl_ns = r.incl_ns.saturating_add(incl_ns);
        r.self_ns = r.self_ns.saturating_add(incl_ns.saturating_sub(child_ns));
        self.count += u64::from(counted);
    }

    /// The rows, by bytes descending (ties: count, then first seen).
    pub fn rows(&self) -> Vec<InstRow> {
        let mut v = self.rows.clone();
        v.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(b.count.cmp(&a.count)));
        v
    }

    /// Bytes of the report's own tables.
    pub fn heap_bytes(&self) -> u64 {
        (self.rows.capacity() * size_of::<InstRow>()
            + self.index.capacity() * (size_of::<(DeclId, RowKind)>() + 4)) as u64
    }
}

impl Sema<'_, '_> {
    /// How a report row names its template: `A<T, N>` for a class or
    /// alias template, `A<T, N>::f` for a member.
    pub fn report_template_name(&self, d: DeclId) -> String {
        let mut s = self.qualified_decl_name(d);
        if let Some(info) = self.ast.template_info(d).copied() {
            let list = *self.ast.templates.param_list(info.params);
            let names: Vec<String> = self
                .ast
                .lists
                .iter::<DeclId>(list.params)
                .map(|p| {
                    let n = self.decl_name(p);
                    let pack = self.ast.template_parm(p).is_some_and(|i| i.pack);
                    if pack { format!("{n}...") } else { n }
                })
                .collect();
            let _ = write!(s, "<{}>", names.join(", "));
        }
        s
    }

    /// The instantiation report for main file `file` in the requested
    /// format (empty when off).
    pub fn instantiation_report(&self, file: &str) -> String {
        let r = &self.inst.report;
        let rows = r.rows();
        let mut out = String::new();
        let loc = |d: DeclId| -> String {
            let l = self.ast.decls.get(d).loc();
            self.diags.sm().presumed_line_col(l).map_or_else(String::new, |p| {
                let name = p.file_name.rsplit('/').next().unwrap_or(p.file_name);
                format!("{name}:{}:{}", p.line, p.col)
            })
        };
        let total_bytes: u64 = rows.iter().map(|x| x.bytes).sum();
        let total_nodes: u64 = rows.iter().map(|x| x.nodes).sum();
        let total_ns: u64 =
            self.inst.report.rows.iter().map(|x| x.self_ns).fold(0u64, u64::saturating_add);
        match r.format {
            ReportFormat::Off => {}
            ReportFormat::Json => {
                for x in &rows {
                    let _ = writeln!(
                        out,
                        "{{\"template\":{},\"kind\":\"{}\",\"loc\":{},\"count\":{},\"nodes\":{},\"bytes\":{},\"self_us\":{},\"incl_us\":{}}}",
                        json_str(&self.report_template_name(x.template)),
                        x.kind.name(),
                        json_str(&loc(x.template)),
                        x.count,
                        x.nodes,
                        x.bytes,
                        x.self_ns / 1000,
                        x.incl_ns / 1000
                    );
                }
                let _ = writeln!(
                    out,
                    "{{\"total\":true,\"count\":{},\"nodes\":{},\"bytes\":{},\"self_us\":{}}}",
                    r.count,
                    total_nodes,
                    total_bytes,
                    total_ns / 1000
                );
            }
            ReportFormat::Text | ReportFormat::All => {
                let _ = writeln!(out, "===-- kiln template instantiation report: {file} --===");
                let _ = writeln!(
                    out,
                    "{:>8} {:>9} {:>10} {:>9} {:>9}  {:<9} template",
                    "count", "nodes", "bytes", "self ms", "incl ms", "kind"
                );
                let n = if r.format == ReportFormat::All { rows.len() } else { rows.len().min(50) };
                for x in &rows[..n] {
                    let _ = writeln!(
                        out,
                        "{:>8} {:>9} {:>10} {:>9.2} {:>9.2}  {:<9} {}   ({})",
                        x.count,
                        x.nodes,
                        x.bytes,
                        ms(x.self_ns),
                        ms(x.incl_ns),
                        x.kind.name(),
                        self.report_template_name(x.template),
                        loc(x.template)
                    );
                }
                if n < rows.len() {
                    let _ = writeln!(out, "  ... {} more rows (use =all)", rows.len() - n);
                }
                let _ = writeln!(
                    out,
                    "  total: {} instantiations, {} nodes, {:.2} MiB retained, {:.1} ms",
                    r.count,
                    total_nodes,
                    mib(total_bytes),
                    ms(total_ns)
                );
            }
        }
        out
    }
}

fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Nanoseconds as milliseconds, for display.
#[allow(clippy::cast_precision_loss, reason = "display only; exact below 2^52 ns")]
fn ms(ns: u64) -> f64 {
    ns as f64 / 1e6
}

/// Bytes as MiB, for display.
#[allow(clippy::cast_precision_loss, reason = "display only; exact below 2^52 bytes")]
fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_excludes_children() {
        let mut r = InstReport::default();
        let a = DeclId::new(1);
        let b = DeclId::new(2);
        r.open(); // a
        r.open(); // b inside a
        r.charge(FrameKind::ClassInstantiation, b, 10, 100, 50, 0);
        r.charge(FrameKind::ClassInstantiation, a, 25, 300, 80, 50);
        let rows = r.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].template, a);
        assert_eq!(rows[0].bytes, 200);
        assert_eq!(rows[0].nodes, 15);
        assert_eq!(rows[0].self_ns, 30);
        assert_eq!(rows[1].bytes, 100);
        assert_eq!(r.count, 2);
    }

    #[test]
    fn json_escapes() {
        assert_eq!(json_str("a\"b\\"), "\"a\\\"b\\\\\"");
    }
}
