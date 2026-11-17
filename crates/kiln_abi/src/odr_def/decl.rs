//! The declarator and specifier rule of O-10 for a function definition,
//! and the local declarations of its body.
//!
//! Hashed: the declaration kind; the specifier bits below; the return
//! and parameter types by their encodings (canonical: a typedef and the
//! type it names are equal; the return type as deduced); variadic,
//! ref-qualifier, `this` qualifiers, calling convention and GNU
//! `noreturn` of the function type; `noexcept` as the EFFECTIVE
//! specification; the trailing requires-clause through the expression
//! path. Not hashed: parameter names (parameters are locals by index),
//! the function's own name (the row's key), contracts (X16), and the
//! lazily set bits (trivial, immediate-escalating, escalated, NRVO,
//! `not_selected`, `return_deduced`).

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind, Defaulted, InitStyle, TlsKind};
use kiln_ast::types::ExcSpec;
use kiln_ast::types::NoexceptValue;
use kiln_ast::{DeclId, ExprId};

use super::{
    Area, DefHasher, Item, Site, T_DEFAULT_ARGS, T_FUNCTION, T_LOCAL_DECL, T_NONE, T_SYNTHESIZED,
    Work,
};
use crate::odr::is_templated;

/// Whether `d` is a template's own declaration or lies inside one: a
/// pattern, a class template partial specialization (as its kind or as
/// its record's link), or anything they enclose. Only instantiations and
/// non-templates are hashed.
pub fn is_templated_def(ast: &kiln_ast::Ast, d: DeclId) -> bool {
    // One predicate with kind 3's (WP-1597).
    is_templated(ast, d)
}

/// Writes the declarator of `def`. `false` when the definition is a
/// template's own (only its instantiations are hashed).
pub(super) fn declarator(h: &mut DefHasher<'_>, def: DeclId) -> bool {
    declarator_as(h, def, false)
}

/// [`declarator`]; with `pattern`, also of a template's own declaration
/// (a generic lambda's call operator, hashed inside its closure).
#[allow(clippy::many_single_char_names, reason = "short local names, one per AST item")]
pub(super) fn declarator_as(h: &mut DefHasher<'_>, def: DeclId, pattern: bool) -> bool {
    let ast = h.ast;
    let n = ast.decls.get(def);
    h.byte(T_FUNCTION);
    h.byte(n.kind() as u8);
    let Some(f) = ast.decls.function(def) else {
        h.partial(Area::Misc, Site::Decl(n.kind()));
        return false;
    };
    if !pattern && is_templated_def(ast, def) {
        // A template's own body: dependent nodes wait for modules (K-6).
        h.partial(Area::Dependent, Site::Item(Item::Templated));
        return false;
    }
    let spec = f.spec;
    let closure = n.ctx().is_some_and(|c| ast.closure_lambda(c).is_some());
    let synthesized = spec.defaulted() != Defaulted::No || n.has(DeclFlags::IMPLICIT);
    // `constexpr` only where kiln can tell it was written (O-10, N25).
    let constexpr = spec.constexpr_kind() == ConstexprKind::Constexpr && !synthesized && !closure;
    let bits = spec.storage() as u32
        | u32::from(spec.is_inline() || spec.implicit_inline()) << 2
        | u32::from(constexpr) << 3
        | u32::from(spec.constexpr_kind() == ConstexprKind::Consteval) << 4
        | u32::from(spec.is_virtual()) << 5
        | u32::from(spec.is_pure()) << 6
        | u32::from(spec.is_override()) << 7
        | u32::from(spec.is_final()) << 8
        | u32::from(spec.is_explicit()) << 9
        | u32::from(spec.is_deleted()) << 10
        | (spec.defaulted() as u32) << 11
        | u32::from(spec.has_explicit_object()) << 13;
    h.u32(bits);
    // The type: return, parameters, function bits.
    let proto = n.ty().and_then(|t| ast.types.proto(t)).copied();
    match proto {
        Some(p) => {
            h.ty(p.ret());
            let params = ast.types.params(&p);
            h.count(params.len());
            for &q in params {
                h.ty(q);
            }
            // Variadic, ref-qualifier, `this` qualifiers, calling
            // convention, `noreturn`, regparm, SME bits; not whether the
            // return type was written trailing.
            h.u32(p.info().with_trailing_return(false).bits());
            match ast.types.exc_spec(&p) {
                // A synthesized member's effective exception
                // specification follows from its class and the members
                // it calls, which the hash covers; sema computes it only
                // when a unit needs it, so it is never hashed.
                _ if synthesized => h.byte(2),
                ExcSpec::Deferred(_) => h.partial(Area::Init, Site::Item(Item::UnresolvedNoexcept)),
                ExcSpec::DependentNoexcept(_)
                | ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(_), .. } => {
                    h.partial(Area::Dependent, Site::Item(Item::DependentNoexcept));
                }
                e => h.byte(u8::from(e.is_nothrow())),
            }
        }
        None => h.byte(T_NONE),
    }
    // Parameters are the first locals, in order.
    for p in ast.lists.iter::<DeclId>(f.params) {
        h.local(p);
    }
    // The trailing requires-clause.
    let req = ast.templates.trailing_requires.get(def).copied();
    match req {
        Some(e) if ast.exprs.get(e).deps().is_none() => {
            h.stack.push(Work::Expr(e));
            h.run();
        }
        Some(_) => h.partial(Area::Dependent, Site::Item(Item::DependentRequires)),
        None => h.byte(T_NONE),
    }
    // Items of later packages.
    h.listed_attrs(def, true);
    default_args(h, def);
    // An implicit or defaulted member is synthesized: its token, then
    // what sema built for it like any definition (the ctor-initializers
    // and body it completed from the class and the special members
    // overload resolution selected for each subobject).
    if synthesized {
        h.byte(T_SYNTHESIZED);
    }
    true
}

/// The declaration's default arguments (O-10, K8; K-3e). The rule: each
/// parameter hashes the default argument that a namespace- or class-scope
/// declaration of the function's redeclaration chain gives it, as
/// accumulated at the end of the unit (block-scope redeclarations have
/// their own set and are skipped), and for an instantiation the pattern's
/// default argument when it is not dependent (a dependent one is a fixed
/// token: it is instantiated only where a call uses it, and that call
/// hashes it). Default template arguments are not hashed (a use names the
/// arguments it chose; the signature area leaves them out too).
fn default_args(h: &mut DefHasher<'_>, def: DeclId) {
    let ast = h.ast;
    let from = ast.instantiated_from(ast.first(def));
    let src = from.unwrap_or(def);
    let block =
        |r: DeclId| ast.decls.lexical_ctx(r).is_some_and(|c| ast.decls.function(c).is_some());
    let chain: Vec<DeclId> = ast.redecls_of(src).filter(|&r| !block(r)).collect();
    let default_of = |i: usize| {
        chain.iter().find_map(|&r| {
            let f = ast.decls.function(r)?;
            let p = ast.lists.iter::<DeclId>(f.params).nth(i)?;
            ast.decls.param(p)?.default_arg
        })
    };
    let count = ast.decls.function(src).map_or(0, |f| ast.lists.len(f.params));
    let defaults: Vec<Option<ExprId>> = (0..count).map(default_of).collect();
    if defaults.iter().all(Option::is_none) {
        return h.byte(T_NONE);
    }
    h.byte(T_DEFAULT_ARGS);
    h.count(count);
    for e in defaults {
        match e {
            None => h.byte(0),
            Some(e) if from.is_some() && !ast.exprs.get(e).deps().is_none() => h.byte(2),
            Some(e) => {
                h.byte(1);
                let base = h.stack.len();
                h.stack.push(Work::Expr(e));
                h.run_nested(base);
            }
        }
    }
}

/// Whether a declaration in a statement is hashed: not a typedef or
/// alias, a using-declaration or -directive, a namespace alias, a
/// `static_assert`, an empty declaration or a block-scope function
/// declaration (none changes what the definition does; what they name is
/// hashed where it is used).
pub(super) fn decl_hashed(ast: &kiln_ast::Ast, d: DeclId) -> bool {
    !matches!(
        ast.decls.get(d).kind(),
        DeclKind::Typedef
            | DeclKind::TypeAlias
            | DeclKind::Using
            | DeclKind::UsingDirective
            | DeclKind::UsingEnum
            | DeclKind::UsingShadow
            | DeclKind::NamespaceAlias
            | DeclKind::StaticAssert
            | DeclKind::Empty
            | DeclKind::Function
    )
}

impl DefHasher<'_> {
    /// A declaration met in a statement: a local variable (index, type,
    /// specifiers, initializer), a structured binding, a declaration that
    /// does not change the definition's meaning (a typedef, a
    /// using-declaration, `static_assert`, a block-scope function
    /// declaration: not hashed), or a local type (K-3d).
    pub(super) fn local_decl(&mut self, d: DeclId) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        match n.kind() {
            DeclKind::Var => {
                let Some(v) = ast.decls.var(d) else {
                    return self.partial(Area::Misc, Site::Decl(n.kind()));
                };
                self.byte(T_LOCAL_DECL);
                self.declare(d);
                match n.ty() {
                    Some(t) => self.ty(t),
                    None => self.byte(T_NONE),
                }
                let s = v.spec;
                let bits = s.storage() as u32
                    | (s.tls() as u32) << 3
                    | (s.init_style() as u32) << 5
                    | u32::from(s.is_constexpr()) << 7
                    | u32::from(s.is_constinit()) << 8;
                debug_assert!(TlsKind::Dynamic as u32 <= 3 && InitStyle::ParenList as u32 <= 3);
                self.u32(bits);
                self.listed_attrs(d, false);
                if let Some(bs) = ast.decls.decomp_bindings(d) {
                    // A structured binding: each binding is a local; the
                    // binding expressions sema builds are implicit.
                    self.count(ast.lists.len(bs));
                    for b in ast.lists.iter::<DeclId>(bs) {
                        self.declare(b);
                    }
                } else {
                    self.byte(T_NONE);
                }
                let init: Option<ExprId> = v.init;
                self.stack.push(Work::OptExpr(init));
            }
            _ if !decl_hashed(ast, d) => {}
            // A local class or enumeration (lambda.rs).
            DeclKind::CXXRecord | DeclKind::Enum => self.local_class(d),
            DeclKind::ClassTemplate
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::FunctionTemplate
            | DeclKind::TypeAliasTemplate => self.partial(Area::Lambda, Site::Decl(n.kind())),
            k => self.partial(Area::Misc, Site::Decl(k)),
        }
    }
}
