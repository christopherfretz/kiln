//! The `-fdump-vtable-layouts` printer: a port of clang 19's
//! `ItaniumVTableBuilder::dumpLayout`, with the method names of
//! `PredefinedExpr::ComputeName(PrettyFunctionNoVirtual, MD)`.
//!
//! Deviation: clang's leading `Original map` block (a dump of a
//! process-global cache, whose content depends on what was built
//! before) is not printed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use kiln_ast::dump::AstNames;
use kiln_ast::{Ast, DeclId};

use super::builder::Built;
use super::classes::{Classes, is_deleted, is_dtor, is_pure};
use kiln_ast::class::{Thunk, VTableComponent};

/// clang `NamedDecl::printQualifiedName`.
pub(crate) fn qualified_name(ast: &Ast, d: DeclId) -> String {
    let mut s = String::new();
    names(ast).write_qualified_spec_name(d, &mut s);
    s
}

/// clang `PredefinedExpr::ComputeName(PrettyFunctionNoVirtual, m)`:
/// `int A<int>::f(T) const [T = int]`.
pub fn pretty_function(ast: &Ast, m: DeclId) -> String {
    kiln_ast::dump::pretty_function(&names(ast), m, false)
}

/// The names of the dump: class template specializations in a scope
/// print their (canonical, scoped) arguments, `O<n::X>::I`.
fn names(ast: &Ast) -> AstNames<'_> {
    AstNames { scoped_spec_args: true, ..AstNames::new(ast) }
}

fn write_adjustment(out: &mut String, label: &str, adj: (i64, i64), virt: &str) {
    let _ = write!(out, "{label}{} non-virtual", adj.0);
    if adj.1 != 0 {
        let _ = write!(out, ", {} {virt}", adj.1);
    }
}

fn write_thunk_method(ast: &Ast, t: &Thunk, out: &mut String) {
    if let Some(m) = t.method {
        out.push_str(" method: ");
        out.push_str(&pretty_function(ast, m));
    }
}

/// Appends the dump of `b` (clang `dumpLayout`).
#[allow(clippy::too_many_lines, reason = "one arm per component kind, as clang's dumpLayout")]
pub(crate) fn write_built(cx: &mut Classes<'_>, b: &Built, out: &mut String) {
    let ast = cx.ast();
    if b.is_construction() {
        let _ = write!(
            out,
            "Construction vtable for ('{}', {}) in '{}'",
            qualified_name(ast, b.most_derived),
            b.md_offset,
            qualified_name(ast, b.layout_class)
        );
    } else {
        let _ = write!(out, "Vtable for '{}'", qualified_name(ast, b.most_derived));
    }
    let _ = writeln!(out, " ({} entries).", b.components.len());

    let mut by_index: BTreeMap<u32, Vec<(DeclId, i64)>> = BTreeMap::new();
    for &(sub, ap) in &b.address_points {
        by_index.entry(ap).or_default().push(sub);
    }
    for (i, c) in b.components.iter().enumerate() {
        let _ = write!(out, "{i:4} | ");
        let i = u32::try_from(i).unwrap_or(u32::MAX);
        let thunk = b.slot_thunks.get(&i).copied().unwrap_or_default();
        match *c {
            VTableComponent::VCallOffset(o) => {
                let _ = write!(out, "vcall_offset ({o})");
            }
            VTableComponent::VBaseOffset(o) => {
                let _ = write!(out, "vbase_offset ({o})");
            }
            VTableComponent::OffsetToTop(o) => {
                let _ = write!(out, "offset_to_top ({o})");
            }
            VTableComponent::Rtti(d) => {
                out.push_str(&qualified_name(ast, d));
                out.push_str(" RTTI");
            }
            VTableComponent::Func(m) => {
                out.push_str(&pretty_function(ast, m));
                if is_pure(ast, m) {
                    out.push_str(" [pure]");
                }
                if is_deleted(ast, m) {
                    out.push_str(" [deleted]");
                }
                if !thunk.is_empty() {
                    if thunk.ret != (0, 0) {
                        write_adjustment(
                            out,
                            "\n       [return adjustment: ",
                            thunk.ret,
                            "vbase offset offset",
                        );
                        out.push(']');
                        write_thunk_method(ast, &thunk, out);
                    }
                    if thunk.this != (0, 0) {
                        write_adjustment(
                            out,
                            "\n       [this adjustment: ",
                            thunk.this,
                            "vcall offset offset",
                        );
                        out.push(']');
                        write_thunk_method(ast, &thunk, out);
                    }
                }
            }
            VTableComponent::CompleteDtor(m) | VTableComponent::DeletingDtor(m) => {
                out.push_str(&qualified_name(ast, m));
                out.push_str(if matches!(c, VTableComponent::CompleteDtor(_)) {
                    "() [complete]"
                } else {
                    "() [deleting]"
                });
                if is_pure(ast, m) {
                    out.push_str(" [pure]");
                }
                if !thunk.is_empty() {
                    if thunk.this != (0, 0) {
                        write_adjustment(
                            out,
                            "\n       [this adjustment: ",
                            thunk.this,
                            "vcall offset offset",
                        );
                        out.push(']');
                    }
                    write_thunk_method(ast, &thunk, out);
                }
            }
            VTableComponent::UnusedFunc(m) => {
                out.push_str("[unused] ");
                out.push_str(&pretty_function(ast, m));
                if is_pure(ast, m) {
                    out.push_str(" [pure]");
                }
            }
        }
        out.push('\n');
        if let Some(subs) = by_index.get(&(i + 1)) {
            if let [(class, off)] = subs.as_slice() {
                let _ = writeln!(
                    out,
                    "       -- ({}, {off}) vtable address --",
                    qualified_name(ast, *class)
                );
            } else {
                let off = subs.first().map_or(0, |s| s.1);
                let names: BTreeSet<String> =
                    subs.iter().map(|&(c, _)| qualified_name(ast, c)).collect();
                for n in names {
                    let _ = writeln!(out, "       -- ({n}, {off}) vtable address --");
                }
            }
        }
    }
    out.push('\n');
    if b.is_construction() {
        return;
    }
    let md_info = cx.get(b.most_derived);
    if md_info.has_vbases() {
        let mut names: BTreeMap<String, i64> = BTreeMap::new();
        for &(v, oo) in &b.vbase_offset_offsets {
            names.entry(qualified_name(ast, v)).or_insert(oo);
        }
        let _ = writeln!(
            out,
            "Virtual base offset offsets for '{}' ({} {}).",
            qualified_name(ast, b.most_derived),
            names.len(),
            if names.len() == 1 { "entry" } else { "entries" }
        );
        for (n, oo) in &names {
            let _ = writeln!(out, "   {n} | {oo}");
        }
        out.push('\n');
    }
    let mut by_name: BTreeMap<String, &Vec<Thunk>> = BTreeMap::new();
    for (m, list) in &b.thunks {
        by_name.entry(pretty_function(ast, *m)).or_insert(list);
    }
    for (name, list) in by_name {
        let mut list: Vec<Thunk> = list.clone();
        list.sort_by_key(|t| (adj_key(t.this), adj_key(t.ret)));
        let _ = writeln!(
            out,
            "Thunks for '{name}' ({} {}).",
            list.len(),
            if list.len() == 1 { "entry" } else { "entries" }
        );
        for (i, t) in list.iter().enumerate() {
            let _ = write!(out, "{i:4} | ");
            if t.ret != (0, 0) {
                write_adjustment(out, "return adjustment: ", t.ret, "vbase offset offset");
                if t.this != (0, 0) {
                    out.push_str("\n       ");
                }
            }
            if t.this != (0, 0) {
                write_adjustment(out, "this adjustment: ", t.this, "vcall offset offset");
            }
            out.push('\n');
        }
        out.push('\n');
    }
    let mut indices: BTreeMap<u32, String> = BTreeMap::new();
    for &m in &md_info.slots {
        let Some(&(_, idx)) = b.method_indices.iter().find(|(d, _)| *d == m) else { continue };
        let name = pretty_function(ast, m);
        if is_dtor(ast, m) {
            indices.insert(idx, format!("{name} [complete]"));
            indices.insert(idx + 1, format!("{name} [deleting]"));
        } else {
            indices.insert(idx, name);
        }
    }
    if !indices.is_empty() {
        let _ = writeln!(
            out,
            "VTable indices for '{}' ({} entries).",
            qualified_name(ast, b.most_derived),
            indices.len()
        );
        for (i, n) in &indices {
            let _ = writeln!(out, "{i:4} | {n}");
        }
    }
    out.push('\n');
}

/// clang's `operator<` on adjustments: the non-virtual part, then the
/// virtual part as a raw `memcmp` of its little-endian bytes.
fn adj_key(a: (i64, i64)) -> (i64, [u8; 8]) {
    (a.0, a.1.to_le_bytes())
}
