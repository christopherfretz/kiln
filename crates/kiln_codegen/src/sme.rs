//! `AArch64` SME function attributes (WP-1426; clang
//! `AddAttributesFromFunctionProtoType` and `ConstructAttributeList`):
//! the streaming mode and the ZA / ZT0 state of a function type become
//! string attributes of every definition, declaration and call of it;
//! `__arm_locally_streaming` and `__arm_new` those of the function
//! declaration (its definition, its declaration and a direct call).

use kiln_ast::attr::AttrKind;
use kiln_ast::types::sme;
use kiln_ast::{Ast, DeclId};
use kiln_llvm::Attr;

/// The attributes of the SME bits `bits` of a function type, plus, for
/// the function declaration `def`, those of its `__arm_locally_streaming`
/// and `__arm_new` attributes (on any of its declarations).
pub(crate) fn sme_attrs(ast: &Ast, def: Option<DeclId>, bits: u16, out: &mut Vec<Attr>) {
    let mut push = |s: &str| out.push(Attr::Str(s.into(), "".into()));
    if bits & sme::STREAMING != 0 {
        push("aarch64_pstate_sm_enabled");
    }
    if bits & sme::STREAMING_COMPATIBLE != 0 {
        push("aarch64_pstate_sm_compatible");
    }
    if bits & sme::AGNOSTIC_ZA != 0 {
        push("aarch64_za_state_agnostic");
    }
    for (state, name) in [(sme::za(bits), "za"), (sme::zt0(bits), "zt0")] {
        let what = match state {
            sme::IN => "in",
            sme::OUT => "out",
            sme::INOUT => "inout",
            sme::PRESERVES => "preserves",
            _ => continue,
        };
        push(&format!("aarch64_{what}_{name}"));
    }
    let Some(d) = def else { return };
    let (mut body, mut new) = (false, 0);
    for r in ast.redecls_of(d) {
        for a in ast.lists.iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r)) {
            let n = ast.attrs.get(a);
            if n.kind == AttrKind::ARM_LOCALLY_STREAMING {
                body = true;
            } else if n.kind == AttrKind::ARM_NEW {
                new |= n.a;
            }
        }
    }
    if body {
        push("aarch64_pstate_sm_body");
    }
    if new & 1 != 0 {
        push("aarch64_new_za");
    }
    if new & 2 != 0 {
        push("aarch64_new_zt0");
    }
}
