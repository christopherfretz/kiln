//! Member templates and templates in templates in the declaration image
//! (spec/modules.md §11.11, "Templates as patterns", "Member templates";
//! WP-1292).
//!
//! A member template (a member function or class template of a class or
//! of a class template) is carried like a namespace-scope one: its
//! parameter records, its pattern record and its template record, all in
//! the class. Its template-head's depth is the number of class templates
//! (and partial specializations) enclosing it, and its parameters carry
//! that depth; a generic lambda's call operator template also counts the
//! function templates whose bodies enclose it (WP-3410). An out-of-line definition of a member (template) of a
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
        // A concept (WP-1284) has no pattern.
        if let Payload::Template { pattern, .. } = d.payload
            && pattern != NONE
        {
            let _ = m.entry(pattern).or_insert(i as u32);
        }
    }
    m
}

/// The parameter count of the template-head that record `c` opens (a
/// class template's pattern, a partial specialization, or a function or
/// variable template's pattern), or `None` for a declaration that is
/// not a template's pattern. A function or variable template's head
/// encloses the lambdas of its body, default arguments or initializer:
/// a generic lambda's call operator template there is one level deeper
/// (WP-3410, [expr.prim.lambda.closure]/3).
fn head_len(img: &Image, pats: &FxHashMap<u32, u32>, c: u32) -> Option<usize> {
    if let Some(n) = classtempl::partial_head_len(img, c) {
        return Some(n);
    }
    let own = img.decls.get(c as usize).map(|r| &r.payload);
    match (own, pats.get(&c).and_then(|&t| img.decls.get(t as usize)).map(|t| &t.payload)) {
        (Some(Payload::Record { .. }), Some(Payload::Template { kind: 1, params, .. }))
        | (Some(Payload::Function { .. }), Some(Payload::Template { kind: 0, params, .. }))
        | (Some(Payload::Var { .. }), Some(Payload::Template { kind: 3, params, .. })) => {
            Some(params.len())
        }
        _ => None,
    }
}

/// The template-heads enclosing declarations whose context is record
/// `c`, outermost first, as parameter counts; `None` when the chain is
/// malformed. Every context is an earlier record (checked by the
/// caller for `c`, here for the rest); a closure with an owner
/// (`owners`, [`super::lambdas::closure_owners`]) continues at its
/// owner, which may be a later record, so the walk is also bounded by
/// the record count.
pub(super) fn enclosing_heads(
    img: &Image,
    pats: &FxHashMap<u32, u32>,
    owners: &FxHashMap<u32, u32>,
    c: u32,
) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    let mut cur = c;
    let mut steps = 0usize;
    while cur != NONE {
        steps += 1;
        if steps > img.decls.len() + 1 {
            return None;
        }
        let r = img.decls.get(cur as usize)?;
        match r.payload {
            // WP-1350: a closure type or local class is in a function,
            // a template's pattern or not; WP-3410: a closure in a
            // variable (template)'s initializer is the variable's.
            Payload::Record { .. } | Payload::Function { .. } | Payload::Var { .. } => {
                if let Some(n) = head_len(img, pats, cur) {
                    out.push(n);
                }
            }
            Payload::Namespace | Payload::Export => break,
            _ => return None,
        }
        // WP-3410: a lambda in a variable's initializer or a default
        // argument is enclosed by its owner's template-heads.
        if let Some(&o) = owners.get(&cur)
            && img.decls.get(o as usize).is_some_and(|x| {
                matches!(x.payload, Payload::Function { .. } | Payload::Var { .. })
            })
        {
            cur = o;
            continue;
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
pub(super) fn level(
    img: &Image,
    pats: &FxHashMap<u32, u32>,
    owners: &FxHashMap<u32, u32>,
    c: u32,
) -> Option<usize> {
    enclosing_heads(img, pats, owners, c).map(|h| h.len())
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
    owners: &FxHashMap<u32, u32>,
    i: usize,
    own: usize,
    claimed: &mut [bool],
) -> Res<()> {
    let d = &img.decls[i];
    let Some(heads) = enclosing_heads(img, pats, owners, d.ctx) else {
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
