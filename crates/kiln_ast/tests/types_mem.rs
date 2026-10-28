//! Per-type memory cost (spec/types.md §9): live bytes per created type,
//! averaged over many, including the intern table's share. Run with
//! `--nocapture` to see the measured numbers; the bounds guard against
//! regressions.

#![allow(clippy::cast_precision_loss, reason = "byte counts far below 2^52")]

use kiln_ast::ids::{DeclId, ExprId, QualType};
use kiln_ast::mem::HeapSize;
use kiln_ast::types::{ExcSpec, FuncInfo, TemplateArg, TypeContext, TypeFlags};

const N: u64 = 4096;

/// Live bytes per call of `make`, averaged over `N` calls.
fn per_call(mut make: impl FnMut(&mut TypeContext, u64)) -> f64 {
    let mut ctx = TypeContext::default();
    // Warm up so that fixed costs (sentinels) are excluded.
    make(&mut ctx, 0);
    let before = ctx.heap_bytes().live_bytes();
    for i in 1..=N {
        make(&mut ctx, i);
    }
    (ctx.heap_bytes().live_bytes() - before) as f64 / N as f64
}

fn check(name: &str, per: f64, bound: f64) {
    println!("{name:<32} {per:6.1} B/type");
    assert!(per <= bound, "{name}: {per} B/type exceeds {bound}");
}

fn rec(c: &mut TypeContext, i: u64) -> QualType {
    c.record_type(DeclId::new(i as u32 + 1)).into()
}

#[test]
fn per_type_costs() {
    let record = per_call(|c, i| {
        rec(c, i);
    });
    check("record", record, 24.0);
    check(
        "pointer",
        per_call(|c, i| {
            let r = rec(c, i);
            c.pointer_to(r);
        }) - record,
        24.0,
    );
    check(
        "constant array",
        per_call(|c, i| {
            c.constant_array(QualType::INT, i, None);
        }),
        40.0,
    );
    check(
        "function (2 params)",
        per_call(|c, i| {
            let r = rec(c, i);
            c.function(QualType::VOID, &[r, QualType::INT], FuncInfo::new(), ExcSpec::None);
        }) - record,
        64.0,
    );
    check(
        "typedef",
        per_call(|c, i| {
            c.typedef_type(DeclId::new(i as u32 + 1), QualType::INT);
        }),
        32.0,
    );
    check(
        "specialization <int, rec>",
        per_call(|c, i| {
            let r = rec(c, i);
            let w = c.template_name_of(DeclId::new(1));
            let l = c.arg_list(&[TemplateArg::ty(QualType::INT), TemplateArg::ty(r)]);
            c.template_specialization(w, l, None, false);
        }) - record,
        96.0,
    );
    check(
        "dependent sized array",
        per_call(|c, i| {
            let t: QualType = c.template_type_parm(DeclId::new(1), 0, 0, false).into();
            let e = ExprId::new(i as u32 + 1);
            let p = c.profile(&[1, i as u32], e, TypeFlags::DEPENDENT_INSTANTIATION);
            c.dependent_sized_array(t, e, p);
        }),
        128.0,
    );
}
