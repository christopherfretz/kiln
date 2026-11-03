//! The shape of a control-flow graph dump (spec/harness.md "cfg tests"):
//! kiln's `-fkiln-dump-cfg` and clang's `debug.DumpCFG` print the same
//! layout but number blocks and spell elements differently, so a `cfg`
//! test compares shapes. A shape keeps, per block, whether it is the
//! entry, the exit or a `NORETURN` block, whether it starts with a label,
//! the kind of its terminator and its successors (pruned ones marked);
//! blocks are renumbered by a depth-first walk from the entry along the
//! successors in order, then from each block the walk missed, in print
//! order. Element lists and predecessor lists are not compared (clang
//! records a `noreturn` block's dropped edge on the predecessor side
//! only).

use std::collections::HashMap;
use std::fmt::Write as _;

/// One function's graph.
#[derive(Debug)]
pub struct Graph {
    /// The function's name: the identifier before the signature's `(`,
    /// without qualifiers or template arguments.
    pub name: String,
    blocks: Vec<Block>,
}

#[derive(Debug, Default)]
struct Block {
    num: u32,
    tag: &'static str,
    label: bool,
    term: String,
    succs: Vec<Succ>,
}

#[derive(Debug, Clone, Copy)]
enum Succ {
    To(u32, bool),
    Null,
}

/// The graphs of `dump`, in order.
pub fn parse(dump: &str) -> Vec<Graph> {
    let mut graphs: Vec<Graph> = Vec::new();
    let mut sig = String::new();
    let mut fresh = false;
    for line in dump.lines() {
        let line = line.trim_end();
        if let Some(h) = line.strip_prefix(" [B").and_then(|h| h.strip_suffix(']')) {
            let (n, tag) = h.split_once(' ').unwrap_or((h, ""));
            let Ok(num) = n.parse::<u32>() else { continue };
            let tag = match tag {
                "(ENTRY)" => "entry",
                "(EXIT)" => "exit",
                "(NORETURN)" => "noreturn",
                _ => "",
            };
            if tag == "entry" {
                graphs.push(Graph { name: fn_name(&sig), blocks: Vec::new() });
            }
            if let Some(g) = graphs.last_mut() {
                g.blocks.push(Block { num, tag, ..Block::default() });
            }
            fresh = true;
            continue;
        }
        if !line.starts_with(' ') && !line.is_empty() {
            line.clone_into(&mut sig);
            continue;
        }
        let Some(b) = graphs.last_mut().and_then(|g| g.blocks.last_mut()) else { continue };
        let first = std::mem::take(&mut fresh);
        if let Some(t) = line.strip_prefix("   T: ") {
            b.term = term_kind(t);
        } else if let Some(s) = line.strip_prefix("   Succs (") {
            let list = s.split_once("):").map_or("", |x| x.1);
            b.succs = list.split_whitespace().map(succ).collect();
        } else if first && line.starts_with("  ") && !line.starts_with("   ") && line.ends_with(':')
        {
            b.label = true;
        }
    }
    graphs
}

/// `B3`, `B3(Unreachable)`, `NULL`.
fn succ(s: &str) -> Succ {
    let (n, dead) = match s.strip_suffix("(Unreachable)") {
        Some(n) => (n, true),
        None => (s, false),
    };
    n.strip_prefix('B').and_then(|n| n.parse().ok()).map_or(Succ::Null, |n| Succ::To(n, !dead))
}

/// The terminator's kind: its keyword, or the operator of a `?:`, `&&`
/// or `||` terminator.
fn term_kind(t: &str) -> String {
    let t = t.trim();
    // `[B4.3] && ...`, `([B4.3] || [B3.3]) && ...`, `[B6.3] ? ... : ...`.
    if t.ends_with("? ... : ...") || t.ends_with("?: ...") {
        return "?:".to_owned();
    }
    for op in ["&&", "||"] {
        if t.ends_with(&format!("{op} ...")) {
            return op.to_owned();
        }
    }
    if t.starts_with("(Temp Dtor)") {
        return "tempdtor".to_owned();
    }
    if t.starts_with("goto") {
        return if t.contains('*') { "goto*".to_owned() } else { "goto".to_owned() };
    }
    let w = t.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').next().unwrap_or(t);
    if w.is_empty() { t.to_owned() } else { w.to_owned() }
}

const LAMBDA: &str = "(lambda)";

/// The name of the function a signature line declares.
fn fn_name(sig: &str) -> String {
    // clang's analyzer dumps no lambda call operator; kiln's are only in
    // the golden.
    if sig.contains("(lambda)") {
        return LAMBDA.to_owned();
    }
    let head = sig.split('(').next().unwrap_or(sig);
    // `template<> int t<int>`: drop template argument lists.
    let mut out = String::new();
    let mut depth = 0u32;
    for c in head.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let last = out.split_whitespace().last().unwrap_or("");
    last.rsplit("::").next().unwrap_or(last).to_owned()
}

impl Graph {
    /// The canonical text of the shape.
    pub fn shape(&self) -> String {
        let by_num: HashMap<u32, usize> =
            self.blocks.iter().enumerate().map(|(i, b)| (b.num, i)).collect();
        let mut canon: Vec<Option<usize>> = vec![None; self.blocks.len()];
        let mut order: Vec<usize> = Vec::new();
        for root in 0..self.blocks.len() {
            let mut stack = vec![root];
            while let Some(i) = stack.pop() {
                if canon[i].is_some() {
                    continue;
                }
                canon[i] = Some(order.len());
                order.push(i);
                for s in self.blocks[i].succs.iter().rev() {
                    if let Succ::To(n, _) = s
                        && let Some(&j) = by_num.get(n)
                        && canon[j].is_none()
                    {
                        stack.push(j);
                    }
                }
            }
        }
        let mut out = String::new();
        for &i in &order {
            let b = &self.blocks[i];
            let mut line = format!("c{}", canon[i].unwrap_or(0));
            for part in [b.tag, if b.label { "label" } else { "" }, b.term.as_str()] {
                if !part.is_empty() {
                    line.push(' ');
                    line.push_str(part);
                }
            }
            if !b.succs.is_empty() {
                line.push_str(" ->");
                for s in &b.succs {
                    match *s {
                        Succ::Null => line.push_str(" NULL"),
                        Succ::To(n, reached) => {
                            let c = by_num.get(&n).and_then(|&j| canon[j]);
                            let c = c.map_or_else(|| format!("?{n}"), |c| format!("c{c}"));
                            line.push(' ');
                            line.push_str(&c);
                            if !reached {
                                line.push_str("(Unreachable)");
                            }
                        }
                    }
                }
            }
            out.push_str(&line);
            out.push('\n');
        }
        out
    }
}

/// Compares the shapes of the graphs of `got` with those of `want`, by
/// function name (the `n`th graph of a name with the `n`th), skipping
/// the functions `skip` names; `None` when they agree. With `stale`, a
/// skipped function whose shapes agree is reported too.
pub fn diff(want: &str, got: &str, skip: &[&str], stale: bool) -> Option<String> {
    let index = |gs: Vec<Graph>| {
        let mut m: Vec<(String, Vec<String>)> = Vec::new();
        for g in gs.into_iter().filter(|g| g.name != LAMBDA) {
            let shape = g.shape();
            match m.iter_mut().find(|(n, _)| *n == g.name) {
                Some((_, v)) => v.push(shape),
                None => m.push((g.name, vec![shape])),
            }
        }
        m
    };
    let (w, g) = (index(parse(want)), index(parse(got)));
    let mut out = String::new();
    for (name, ws) in &w {
        let gs = g.iter().find(|(n, _)| n == name).map_or(&[][..], |(_, v)| v.as_slice());
        let skipped = skip.contains(&name.as_str());
        if ws.len() != gs.len() {
            if !skipped {
                let _ =
                    writeln!(out, "`{name}`: {} graph(s) expected, {} found", ws.len(), gs.len());
            }
            continue;
        }
        for (a, b) in ws.iter().zip(gs) {
            if skipped {
                if stale && a == b {
                    let _ =
                        writeln!(out, "`{name}` is listed in `cfg-diverges` but the shapes agree");
                }
            } else if a != b {
                let _ = write!(
                    out,
                    "`{name}`: the shapes differ\n{}",
                    super::runner::indent(&super::compare::line_diff(a, b))
                );
            }
        }
    }
    for (name, _) in &g {
        if !w.iter().any(|(n, _)| n == name) && !skip.contains(&name.as_str()) {
            let _ = writeln!(out, "`{name}`: not in the reference dump");
        }
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::{LAMBDA, diff, fn_name, parse, term_kind};

    const CLANG: &str = "\
int f(int x)
 [B4 (ENTRY)]
   Succs (1): B3
 [B1]
   1: 2
   2: return [B1.1];
   Preds (1): B3
   Succs (1): B0
 [B2]
   1: 1
   2: return [B2.1];
   Preds (1): B3
   Succs (1): B0
 [B3]
   1: x
   2: [B3.1] (ImplicitCastExpr, LValueToRValue, int)
   3: [B3.2] (ImplicitCastExpr, IntegralToBoolean, _Bool)
   T: if [B3.3]
   Preds (1): B4
   Succs (2): B2 B1(Unreachable)
 [B0 (EXIT)]
   Preds (2): B1 B2
";

    #[test]
    fn shapes_ignore_numbering_and_elements() {
        let kiln = "\
f()
 [B4 (ENTRY)]
   Succs (1): B1
 [B1]
   1: x (DeclRefExpr)
   T: if [B1.1]
   Preds (1): B4
   Succs (2): B3 B2(Unreachable)
 [B3]
   1: ReturnStmt
   Succs (1): B0
 [B2]
   Succs (1): B0
 [B0 (EXIT)]
";
        let g = parse(CLANG);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].name, "f");
        assert_eq!(
            g[0].shape(),
            "c0 entry -> c1\nc1 if -> c2 c4(Unreachable)\nc2 -> c3\nc3 exit\nc4 -> c3\n"
        );
        assert_eq!(diff(CLANG, kiln, &[], true), None);
        let other = kiln.replace("B2(Unreachable)", "B2");
        assert!(diff(CLANG, &other, &[], true).is_some());
        assert_eq!(diff(CLANG, &other, &["f"], false), None);
        assert!(diff(CLANG, kiln, &["f"], true).is_some(), "a stale divergence is reported");
    }

    #[test]
    fn function_names() {
        assert_eq!(fn_name("template<> int t<int>(int x)"), "t");
        assert_eq!(fn_name("void S::n()"), "n");
        assert_eq!(fn_name("~S() noexcept"), "~S");
        assert_eq!(fn_name("S::S()"), "S");
        assert_eq!(fn_name("f(int)::(lambda)::operator()()"), LAMBDA);
    }

    #[test]
    fn labels_and_terminators() {
        let d = "\
g()
 [B3 (ENTRY)]
   Succs (1): B2
 [B2]
  case 1:
  10: x
   T: [B2.10] && ...
   Succs (2): B1 NULL
 [B1]
   T: do ... while [B1.1]
   Succs (1): B0
 [B0 (EXIT)]
";
        assert_eq!(
            parse(d)[0].shape(),
            "c0 entry -> c1\nc1 label && -> c2 NULL\nc2 do -> c3\nc3 exit\n"
        );
    }

    #[test]
    fn terminator_kinds() {
        assert_eq!(term_kind("[B4.3] && ..."), "&&");
        assert_eq!(term_kind("([B4.3] || [B3.3]) && ..."), "&&");
        assert_eq!(term_kind("([B8.3] && [B7.5]) || ..."), "||");
        assert_eq!(term_kind("[B6.3] ? ... : ..."), "?:");
        assert_eq!(term_kind("(Temp Dtor) [B4.3]"), "tempdtor");
        assert_eq!(term_kind("do ... while [B1.1]"), "do");
        assert_eq!(term_kind("goto *[B2.1];"), "goto*");
        assert_eq!(term_kind("for (...; [B3.2]; ...)"), "for");
    }
}
