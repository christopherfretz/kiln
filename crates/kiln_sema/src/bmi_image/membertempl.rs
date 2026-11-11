//! Member templates and templates in templates in the declaration image
//! (spec/modules.md §11.11, "Templates as patterns", "Member templates";
//! WP-1292).
//!
//! A member template (a member function or class template of a class or
//! of a class template) is carried like a namespace-scope one: its
//! parameter records, its pattern record and its template record, all in
//! the class. Its template-head's depth is the number of class templates
//! (and partial specializations) enclosing it, and its parameters carry
//! that depth. An out-of-line definition of a member (template) of a
//! class template is preceded by one parameter record per parameter of
//! each enclosing template-head, outermost first, then by its own
//! template-head's parameters; all of them in the definition's lexical
//! scope.
//!
//! Claims (the validator's rule that every dependent record belongs to
//! exactly one template): a member of a class template's pattern is
//! claimed by its class, except a member template's pattern and its
//! parameters, which its template record claims, and the parameters of
//! an out-of-line definition's enclosing heads, which the definition
//! claims.

use kiln_source::fx::FxHashMap;

use super::{Image, LEX_TU, NONE, Payload, Res, bad, classtempl};

/// Per pattern record, the template record that claims it.
pub(super) fn template_patterns(img: &Image) -> FxHashMap<u32, u32> {
    let mut m = FxHashMap::default();
    for (i, d) in img.decls.iter().enumerate() {
        if let Payload::Template { pattern, .. } = d.payload {
            let _ = m.entry(pattern).or_insert(i as u32);
        }
    }
    m
}

/// The parameter count of the template-head that class record `c` opens
/// (a class template's pattern, or a partial specialization), or `None`
/// for a class that is not a template.
fn head_len(img: &Image, pats: &FxHashMap<u32, u32>, c: u32) -> Option<usize> {
    if let Some(n) = classtempl::partial_head_len(img, c) {
        return Some(n);
    }
    match pats.get(&c).and_then(|&t| img.decls.get(t as usize)).map(|t| &t.payload) {
        Some(Payload::Template { kind: 1, params, .. }) => Some(params.len()),
        _ => None,
    }
}

/// The template-heads enclosing declarations whose context is record
/// `c`, outermost first, as parameter counts; `None` when the chain is
/// malformed. Every context is an earlier record (checked by the
/// caller for `c`, here for the rest), so the walk ends.
pub(super) fn enclosing_heads(
    img: &Image,
    pats: &FxHashMap<u32, u32>,
    c: u32,
) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    let mut cur = c;
    while cur != NONE {
        let r = img.decls.get(cur as usize)?;
        match r.payload {
            Payload::Record { .. } => {
                if let Some(n) = head_len(img, pats, cur) {
                    out.push(n);
                }
            }
            Payload::Namespace | Payload::Export => break,
            _ => return None,
        }
        if r.ctx != NONE && r.ctx >= cur {
            return None;
        }
        cur = r.ctx;
    }
    out.reverse();
    Some(out)
}

/// The template depth of a template-head declared in context `c`.
pub(super) fn level(img: &Image, pats: &FxHashMap<u32, u32>, c: u32) -> Option<usize> {
    enclosing_heads(img, pats, c).map(|h| h.len())
}

/// The scope an out-of-line definition's parameters are declared in:
/// its lexical context.
pub(super) fn param_scope(lexical: u32, ctx: u32) -> u32 {
    match lexical {
        NONE => ctx,
        LEX_TU => NONE,
        l => l,
    }
}

/// An out-of-line definition, record `i`, of a member of dependent class
/// `ctx`: claims the parameters of the enclosing template-heads, just
/// before its own template-head's `own` parameters (a member template's,
/// claimed by its template record), which are just before it.
pub(super) fn claim_outer_heads(
    img: &Image,
    pats: &FxHashMap<u32, u32>,
    i: usize,
    own: usize,
    claimed: &mut [bool],
) -> Res<()> {
    let d = &img.decls[i];
    let Some(heads) = enclosing_heads(img, pats, d.ctx) else {
        return bad("an out-of-line member of a nested dependent class");
    };
    let total: usize = heads.iter().sum();
    let Some(start) = i.checked_sub(own + total) else {
        return bad("an out-of-line member of a nested dependent class");
    };
    let scope = param_scope(d.lexical, NONE);
    let mut j = start;
    for (depth, &n) in heads.iter().enumerate() {
        for k in 0..n {
            let ok = match img.decls[j].payload {
                Payload::Ttp { depth: pd, index, .. } | Payload::Nttp { depth: pd, index, .. } => {
                    usize::from(pd) == depth && usize::from(index) == k && img.decls[j].ctx == scope
                }
                _ => false,
            };
            if !ok || claimed[j] {
                return bad("bad template parameter list");
            }
            claimed[j] = true;
            j += 1;
        }
    }
    Ok(())
}
