//! The expressions written in a type (spec/types.md §8.4, WP-3015): what a
//! tooling client walks to find every expression, and so every call,
//! written in a declaration's type, a `sizeof` or cast operand's type, or
//! a template argument (clang's `TypeLoc` children, which kiln keeps as
//! type sugar).

use super::args::ArgView;
use super::context::{ExprOperand, TypeContext, TypeOfOperand, TypeView};
use super::func::{ExcSpec, FuncProto};
use super::nns::Nns;
use crate::ids::{ArgListId, DeclId, ExprId, NnsId, QualType};

/// How deep [`TypeContext::written_exprs`] descends (a written type is
/// never this deep; the bound keeps a malformed one finite).
const MAX_DEPTH: u32 = 512;

impl TypeContext {
    /// Appends to `out` the root of every expression written in type `q`
    /// as written (spec/types.md §8.4), in pre-order over the type's
    /// structure: a constant array's written bound (§7.2 "Written
    /// bounds"), a dependent or variable array's bound, the operand of
    /// `decltype(e)`, `typeof(e)`, `noexcept(e)` in a function type, a
    /// pack index, a splice, a dependent vector size or address space, and
    /// every expression argument of a template-id written in it. It
    /// descends into the types `q` is written of (elements, pointees,
    /// referees, a member pointer's class, return and parameter types,
    /// qualifiers' types, template-id type arguments, a pack expansion's
    /// pattern, an attributed type's modified type, an adjusted
    /// parameter's declared array type), never into what a name denotes
    /// (a typedef's, alias's or using-declaration's type, a class, a
    /// deduced or substituted type) nor into a canonical profile, whose
    /// expression belongs to another declaration. An expression's own
    /// subexpressions are `ExprArena::children`'s.
    pub fn written_exprs(&self, q: QualType, out: &mut Vec<ExprId>) {
        self.written_exprs_in(q, out, 0);
    }

    /// True if [`Self::written_exprs`] appends at least one expression for
    /// `q`. Allocates nothing when `q` contains no written expression.
    pub fn has_written_exprs(&self, q: QualType) -> bool {
        let mut out = Vec::new();
        self.written_exprs_in(q, &mut out, 0);
        !out.is_empty()
    }

    fn written_exprs_in(&self, q: QualType, out: &mut Vec<ExprId>, depth: u32) {
        if depth > MAX_DEPTH {
            return;
        }
        let d = depth + 1;
        let operand = |o: ExprOperand, out: &mut Vec<ExprId>| {
            if let ExprOperand::Expr(e) = o {
                out.push(e);
            }
        };
        match self.view(q.ty()) {
            TypeView::Pointer(x)
            | TypeView::LValueRef(x)
            | TypeView::RValueRef(x)
            | TypeView::IncompleteArray(x)
            | TypeView::Paren(x)
            | TypeView::Complex(x)
            | TypeView::Atomic(x)
            | TypeView::Vector { elem: x, .. }
            | TypeView::ExtQual { base: x, .. }
            | TypeView::Attributed { modified: x, .. }
            | TypeView::Decayed { original: x, .. }
            | TypeView::PackExpansion { pattern: x, .. }
            | TypeView::UnaryTransform { base: x, .. } => self.written_exprs_in(x, out, d),
            TypeView::QualifiedArray { base, .. } => {
                self.written_exprs_in(QualType::unqual(base), out, d);
            }
            TypeView::MemberPointer { pointee, class } => {
                self.written_exprs_in(pointee, out, d);
                self.written_exprs_in(QualType::unqual(class), out, d);
            }
            TypeView::ConstantArray(a) => {
                out.extend(a.size_expr);
                self.written_exprs_in(a.elem, out, d);
            }
            TypeView::VariableArray { elem, size } => {
                // `[*]`'s shared bound is no expression written there.
                if self.vla_star_bound != Some(size) {
                    out.push(size);
                }
                self.written_exprs_in(elem, out, d);
            }
            TypeView::DependentSizedArray { elem, size }
            | TypeView::DependentVector { elem, size, .. }
            | TypeView::DependentAddrSpace { pointee: elem, space: size } => {
                operand(size, out);
                self.written_exprs_in(elem, out, d);
            }
            TypeView::Function(p) => {
                self.written_exprs_in(p.ret(), out, d);
                self.written_fn_params_exc(&p, out, d);
            }
            TypeView::Elaborated { nns, named, .. } => {
                if let Some(n) = nns {
                    self.written_nns_exprs(n, out, d);
                }
                self.written_exprs_in(named, out, d);
            }
            TypeView::DependentName { nns, .. } => self.written_nns_exprs(nns, out, d),
            TypeView::DependentTemplateSpecialization { nns, args, .. } => {
                self.written_nns_exprs(nns, out, d);
                self.written_args_in(args, out, d);
            }
            TypeView::TemplateSpecialization { args, .. }
            | TypeView::Auto { args: Some(args), .. } => {
                self.written_args_in(args, out, d);
            }
            TypeView::PackIndexing { pattern, index, .. } => {
                self.written_exprs_in(pattern, out, d);
                operand(index, out);
            }
            TypeView::Decltype { expr, .. } => out.push(expr),
            TypeView::TypeOf { operand: TypeOfOperand::Expr(e), .. } => out.push(e),
            TypeView::TypeOf { operand: TypeOfOperand::Type(t), .. } => {
                self.written_exprs_in(t, out, d);
            }
            TypeView::Splice { operand: o, .. } => operand(o, out),
            _ => {}
        }
    }

    /// Appends to `out` the expressions written in template argument list
    /// `args` ([`Self::written_exprs`]): an expression argument's root,
    /// and those written in type arguments and packs. For the arguments
    /// as written of a specialization
    /// ([`crate::templates::TemplateTables::args_as_written`]).
    pub fn written_arg_exprs(&self, args: ArgListId, out: &mut Vec<ExprId>) {
        self.written_args_in(args, out, 0);
    }

    fn written_args_in(&self, args: ArgListId, out: &mut Vec<ExprId>, depth: u32) {
        if depth > MAX_DEPTH {
            return;
        }
        for &a in self.args(args) {
            match a.view() {
                ArgView::Type(t) => self.written_exprs_in(t, out, depth + 1),
                ArgView::Expr { expr, .. } => out.push(expr),
                ArgView::Pack(l) => self.written_args_in(l, out, depth + 1),
                _ => {}
            }
        }
    }

    fn written_nns_exprs(&self, n: NnsId, out: &mut Vec<ExprId>, depth: u32) {
        if depth > MAX_DEPTH {
            return;
        }
        match self.nns_view(n) {
            Nns::Type { prefix, ty, .. } => {
                if let Some(p) = prefix {
                    self.written_nns_exprs(p, out, depth + 1);
                }
                self.written_exprs_in(ty, out, depth + 1);
            }
            Nns::Namespace { prefix: Some(p), .. }
            | Nns::NamespaceAlias { prefix: Some(p), .. }
            | Nns::Identifier { prefix: Some(p), .. } => {
                self.written_nns_exprs(p, out, depth + 1);
            }
            Nns::Splice(e) => out.push(e),
            _ => {}
        }
    }

    fn written_fn_params_exc(&self, p: &FuncProto, out: &mut Vec<ExprId>, depth: u32) {
        for &t in self.params(p) {
            self.written_exprs_in(t, out, depth);
        }
        if let ExcSpec::NoexceptExpr { expr, .. } = self.exc_spec(p) {
            out.push(expr);
        }
    }

    /// Like [`Self::written_exprs`] for a function whose return type was
    /// deduced (spec/types.md §8.4, WP-3641): skips transparent top-level
    /// sugar (such as a calling-convention attribute) to the function type
    /// and appends only its parameter and `noexcept(e)` expressions, never
    /// its return type.
    fn written_exprs_without_return(&self, mut q: QualType, out: &mut Vec<ExprId>) {
        for depth in 0..=MAX_DEPTH {
            match self.view(q.ty()) {
                TypeView::Attributed { modified: x, .. }
                | TypeView::Paren(x)
                | TypeView::ExtQual { base: x, .. } => q = x,
                TypeView::Function(p) => {
                    self.written_fn_params_exc(&p, out, depth + 1);
                    return;
                }
                _ => return,
            }
        }
    }

    /// Appends only the expressions written in the return type of function
    /// type `q` (skipping transparent top-level sugar), for a deduced
    /// return type whose written form carries expressions around its
    /// placeholder (spec/types.md §8.4, WP-3640).
    fn written_return_exprs(&self, mut q: QualType, out: &mut Vec<ExprId>) {
        for depth in 0..=MAX_DEPTH {
            match self.view(q.ty()) {
                TypeView::Attributed { modified: x, .. }
                | TypeView::Paren(x)
                | TypeView::ExtQual { base: x, .. } => q = x,
                TypeView::Function(p) => {
                    self.written_exprs_in(p.ret(), out, depth + 1);
                    return;
                }
                _ => return,
            }
        }
    }
}

impl crate::Ast {
    /// Appends to `out` the expressions written in declaration `d`'s type
    /// (spec/types.md §8.4, WP-3583, WP-3640, WP-3641):
    /// [`TypeContext::written_exprs`] of its type, except that a deduced
    /// type is no written one. For a structured binding (`Binding`),
    /// nothing is appended. For a variable deduced from a placeholder
    /// ([`crate::decl::VarSpec::placeholder`] is not `None`), only the
    /// expressions written around the placeholder
    /// ([`crate::decl::DeclArena::written_decl_type`], if any) are
    /// appended. For a function whose return type was deduced
    /// ([`crate::decl::FnSpec::return_deduced`]), any expressions written
    /// around the return placeholder are appended first, then its
    /// parameter types' and `noexcept(e)` expressions, never its deduced
    /// return type.
    pub fn decl_written_exprs(&self, d: DeclId, out: &mut Vec<ExprId>) {
        use crate::decl::{DeclKind, VarPlaceholder};
        let n = self.decls.get(d);
        if n.kind() == DeclKind::Binding {
            return;
        }
        if self.decls.var(d).is_some_and(|v| v.spec.placeholder() != VarPlaceholder::None) {
            if let Some(w) = self.decls.written_decl_type(d) {
                self.types.written_exprs(w, out);
            }
            return;
        }
        let Some(t) = n.ty() else { return };
        if self.decls.function(d).is_some()
            && self
                .redecls_of(d)
                .any(|r| self.decls.function(r).is_some_and(|f| f.spec.return_deduced()))
        {
            if let Some(w) = self
                .decls
                .written_decl_type(d)
                .or_else(|| self.redecls_of(d).find_map(|r| self.decls.written_decl_type(r)))
            {
                self.types.written_return_exprs(w, out);
            }
            self.types.written_exprs_without_return(t, out);
        } else {
            self.types.written_exprs(t, out);
        }
    }
}
