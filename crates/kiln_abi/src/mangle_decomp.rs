//! Itanium mangling of a structured-binding declaration's variable
//! (clang `mangleUnqualifiedName` for a `DecompositionDecl`):
//! `<unqualified-name> ::= DC <source-name>+ E`, never `L`-prefixed and
//! never emitted unmangled (not even in `extern "C"`).

use kiln_ast::{Ast, DeclId};

use crate::mangle::decl_name;

/// `DC <source-name>+ E` if `d` is a decomposition's variable.
pub fn decomposition_name(ast: &Ast, d: DeclId) -> Option<String> {
    let l = ast.decls.decomp_bindings(d)?;
    let mut out = String::from("DC");
    for b in ast.lists.iter::<DeclId>(l) {
        let n = decl_name(ast, b);
        out.push_str(&n.len().to_string());
        out.push_str(&n);
    }
    out.push('E');
    Some(out)
}
