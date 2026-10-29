//! Lowering of new- and delete-expressions to `Op::New` / `Op::Delete`
//! (spec/oven.md "Transient allocation"; [expr.new], [expr.delete],
//! [expr.const]/5.20).
//!
//! The checks clang 19's `VisitCXXNewExpr` / `VisitCXXDeleteExpr` make
//! before touching memory are made here, at compile time, when they depend
//! only on the expression (which allocation function, which placement
//! arguments); the rest (the array bound, the pointer deleted) are made by
//! the VM.

use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::expr::{CastKind, ExprFlags, ExprKind};
use kiln_ast::names::OverloadedOperator;
use kiln_ast::{Ast, DeclId, ExprId, QualType};

use super::{Access, Compiler, skip_parens};
use crate::kbc::{Op, cmp, int_aux, new_aux};
use crate::mem::Origin;
use crate::ty::{OTy, TyKind};

/// The note of a class-type new/delete the Oven cannot run yet.
const CLASS_UNSUPPORTED: &str =
    "kiln: constant evaluation of constructors and destructors in new/delete lands with WP-076";

impl Compiler<'_> {
    /// `new T`, `new T(init)`, `new T{init}`, `new T[n]...` into `dst`.
    pub(super) fn new_expr(&mut self, e: ExprId, dst: u32) {
        let ast = self.ast();
        let x = *ast.exprs.new_extra(e);
        let Some(op_new) = x.operator_new else {
            self.unsupported(e);
            return;
        };
        // [expr.const]/5.20: only a replaceable global allocation function
        // may be called (clang: `note_constexpr_new_non_replaceable`).
        if !is_replaceable_global_alloc(ast, op_new) {
            let msg = non_replaceable_note(ast, op_new);
            self.trap(e, &msg);
            return;
        }
        let mut aux = 0;
        if let Some(list) = x.placement {
            let args: Vec<ExprId> = ast.lists.iter::<ExprId>(list).collect();
            // clang supports exactly `new (std::nothrow) T`.
            let nothrow = args.len() == 1 && {
                let t = ast.exprs.get(args[0]).ty();
                is_std_record(ast, t, "nothrow_t")
            };
            if !nothrow {
                self.trap(
                    e,
                    "this placement new expression is not yet supported in constant expressions",
                );
                return;
            }
            let _ = self.lv(args[0], Access::Addr);
            aux |= new_aux::NOTHROW;
        }
        let elem = self.ty(x.allocated_ty);
        let init = x.init;
        let loc_new = self.ast().exprs.get(e).loc();
        let oi = self.o.push_origin(Origin::Heap(e, 0));
        let Some(size_e) = x.array_size else {
            // A single object.
            if init.is_none() && !self.trivially_default_init(x.allocated_ty) {
                self.trap(e, CLASS_UNSUPPORTED);
                return;
            }
            if init.is_some_and(|i| self.node(i).kind() == ExprKind::CXXConstructExpr) {
                self.trap(e, CLASS_UNSUPPORTED);
                return;
            }
            self.emit_site(Op::New, aux, &[dst, elem.0, dst, oi, 0], e, loc_new);
            if let Some(i) = init {
                self.init(i, dst, elem);
            }
            return;
        };
        // The bound, without its conversion to `size_t` (clang strips
        // implicit no-op and integral casts, so a negative bound is seen as
        // such).
        let ast = self.ast();
        let mut stripped = size_e;
        while ast.exprs.get(stripped).kind() == ExprKind::ImplicitCastExpr
            && matches!(ast.exprs.cast_kind(stripped), CastKind::NoOp | CastKind::IntegralCast)
        {
            stripped = ast.exprs.operand(stripped);
        }
        let size_loc = ast.exprs.get(size_e).loc();
        let bty = self.ety(stripped);
        if matches!(self.kind(bty), TyKind::Int { signed: true, .. }) {
            aux |= new_aux::SIGNED;
        }
        aux |= new_aux::ARRAY;
        // How the elements are initialized.
        enum ArrInit {
            /// Default-initialization (nothing to do for trivial types).
            None,
            /// Value-initialization: all zero.
            Zero,
            /// A braced list (of `k` elements) and its filler.
            List { list: ExprId, k: u64, filler: Option<ExprId>, zero_rest: bool },
        }
        let ai = match init.map(|i| (i, self.node(i).kind())) {
            None => {
                if !self.trivially_default_init(x.allocated_ty) {
                    self.trap(e, CLASS_UNSUPPORTED);
                    return;
                }
                ArrInit::None
            }
            Some((_, ExprKind::ImplicitValueInitExpr)) => ArrInit::Zero,
            Some((i, ExprKind::InitListExpr)) => {
                let ast = self.ast();
                let lty = ast.exprs.get(i).ty();
                let Some(k) = ast.types.array_size(lty) else {
                    self.unsupported(i);
                    return;
                };
                let filler = ast.exprs.init_list(i).array_filler;
                let zero_rest = filler.is_some_and(|f| self.is_zero_init(f));
                ArrInit::List { list: i, k, filler, zero_rest }
            }
            Some((i, _)) => {
                if self.node(i).kind() == ExprKind::CXXConstructExpr {
                    self.trap(e, CLASS_UNSUPPORTED);
                } else {
                    self.unsupported(i);
                }
                return;
            }
        };
        let k = match ai {
            ArrInit::List { k, .. } => k,
            _ => 0,
        };
        if matches!(ai, ArrInit::Zero | ArrInit::List { zero_rest: true, .. }) {
            aux |= new_aux::ZERO;
        }
        let cnt = self.rv_copy(stripped);
        self.emit_site(Op::New, aux, &[dst, elem.0, cnt, oi, k as u32], e, size_loc);
        let ArrInit::List { list, k, filler, zero_rest } = ai else {
            return;
        };
        // `new (std::nothrow) T[n]{...}` with a bad bound yields null:
        // initialize nothing.
        let skip = (aux & new_aux::NOTHROW != 0).then(|| {
            let c = self.reg();
            self.emit(Op::PtrToBool, 0, &[c, dst]);
            self.jmp_if(c, false)
        });
        let ast = self.ast();
        let inits: Vec<ExprId> =
            ast.lists.iter::<ExprId>(ast.exprs.init_list_inits(list)).collect();
        let lty = self.ety(list);
        let from = if inits.len() == 1
            && self.node(skip_parens(self.ast(), inits[0])).kind() == ExprKind::StringLiteral
        {
            // `new char[n]{"..."}`: the leading `k` elements as a whole.
            self.init(list, dst, lty);
            k
        } else {
            let es = self.o.types.size(elem);
            for (i, &v) in inits.iter().enumerate() {
                let q = self.elem_ptr(dst, i as u64 * es, e);
                self.init(v, q, elem);
            }
            inits.len() as u64
        };
        // The filler over the remaining elements (those of the list's own
        // type it does not initialize, and those beyond it).
        if !zero_rest && let Some(f) = filler {
            self.fill_loop(dst, from, cnt, f, elem, e);
        }
        if let Some(j) = skip {
            self.patch_here(j);
        }
    }

    /// Initializes elements `[from, lim)` of the array at `p` from `f`
    /// (`lim`: a register).
    fn fill_loop(&mut self, p: u32, from: u64, lim: u32, f: ExprId, elem: OTy, e: ExprId) {
        let m = self.next;
        let k = self.reg();
        let one = self.reg();
        let c = self.reg();
        self.konst(k, u128::from(from));
        self.konst(one, 1);
        let top = self.here();
        self.emit(Op::ICmp, int_aux(64, false) | cmp::LT << 9, &[c, k, lim]);
        let exit = self.jmp_if(c, false);
        let q = self.reg();
        self.emit_at(Op::PtrAdd, int_aux(64, false), &[q, p, k, elem.0], e);
        self.init(f, q, elem);
        self.emit(Op::IAdd, int_aux(64, false), &[k, k, one]);
        let j = self.jmp();
        self.patch(j, top);
        self.patch_here(exit);
        self.next = m;
    }

    /// Whether default-initializing a `t` (or each element of an array of
    /// them) does nothing the Oven must run: no constructor.
    fn trivially_default_init(&self, t: QualType) -> bool {
        let ast = self.ast();
        let mut t = t;
        while ast.types.is_array(t)
            && let Some(e) = ast.types.element(t)
        {
            t = e;
        }
        match ast.types.as_record(t) {
            None => true,
            Some(r) => record_bits(ast, r).contains(DefinitionBits::TRIVIAL_DEFAULT_CTOR),
        }
    }

    /// Whether initializer `e` yields all-zero bits with no side effects
    /// (an implicit value-initialization, or a list of them).
    fn is_zero_init(&self, e: ExprId) -> bool {
        let ast = self.ast();
        let n = ast.exprs.get(e);
        match n.kind() {
            ExprKind::ImplicitValueInitExpr => true,
            ExprKind::InitListExpr => {
                // A union's list selects a member: not plain zero.
                if ast.types.as_record(n.ty()).is_some_and(|r| {
                    let def = ast.definition(r).unwrap_or(r);
                    ast.decls.record(def).is_some_and(|x| x.tag == kiln_ast::decl::TagKind::Union)
                }) {
                    return false;
                }
                let extra = ast.exprs.init_list(e);
                ast.lists.iter::<ExprId>(ast.exprs.init_list_inits(e)).all(|x| self.is_zero_init(x))
                    && extra.array_filler.is_none_or(|f| self.is_zero_init(f))
            }
            _ => false,
        }
    }

    /// `delete p` / `delete[] p`.
    pub(super) fn delete_expr(&mut self, e: ExprId) {
        let ast = self.ast();
        let Some(op_del) = ast.exprs.operator_delete(e) else {
            self.unsupported(e);
            return;
        };
        if !is_replaceable_global_alloc(ast, op_del) {
            let msg = non_replaceable_note(ast, op_del);
            self.trap(e, &msg);
            return;
        }
        let array = self.node(e).has(ExprFlags::ARRAY_FORM);
        let arg = ast.exprs.delete_operand(e);
        let Some(pq) = ast.types.pointee(ast.exprs.get(arg).ty()) else {
            self.unsupported(e);
            return;
        };
        if ast.types.is_void(pq) {
            self.unsupported(e);
            return;
        }
        let mut t = pq;
        while ast.types.is_array(t)
            && let Some(x) = ast.types.element(t)
        {
            t = x;
        }
        if let Some(r) = ast.types.as_record(t)
            && !record_bits(ast, r).contains(DefinitionBits::TRIVIAL_DTOR)
        {
            self.trap(e, CLASS_UNSUPPORTED);
            return;
        }
        let pointee = self.ty(pq);
        let p = self.rv(arg);
        self.emit_at(Op::Delete, u32::from(array), &[p, pointee.0], e);
    }
}

/// The definition bits of record `r` (empty when it is incomplete).
fn record_bits(ast: &Ast, r: DeclId) -> DefinitionBits {
    let def = ast.definition(r).unwrap_or(r);
    ast.decls.record(def).map(|x| x.defn).unwrap_or_default()
}

/// clang's `note_constexpr_new_non_replaceable`.
fn non_replaceable_note(ast: &Ast, d: DeclId) -> String {
    let what =
        if ast.decls.get(d).kind() == DeclKind::CXXMethod { "class-specific" } else { "placement" };
    let name = match ast.decls.get(d).name().as_operator() {
        Some(OverloadedOperator::New) => "operator new",
        Some(OverloadedOperator::ArrayNew) => "operator new[]",
        Some(OverloadedOperator::Delete) => "operator delete",
        Some(OverloadedOperator::ArrayDelete) => "operator delete[]",
        _ => "operator new",
    };
    format!("call to {what} '{name}'")
}

/// Whether `d` is in namespace `std` at translation-unit scope (through
/// inline namespaces).
fn in_std(ast: &Ast, d: DeclId) -> bool {
    let decls = &ast.decls;
    let mut ns = decls.get(d).ctx();
    while let Some(n) = ns
        && decls.namespace(n).is_some_and(kiln_ast::decl::NamespaceData::is_inline)
    {
        ns = decls.get(n).ctx();
    }
    ns.is_some_and(|n| {
        decls.get(n).kind() == DeclKind::Namespace
            && is_named(ast, n, "std")
            && decls
                .get(n)
                .ctx()
                .is_some_and(|tu| decls.get(tu).kind() == DeclKind::TranslationUnit)
    })
}

fn is_named(ast: &Ast, d: DeclId, name: &str) -> bool {
    ast.decls.get(d).name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == name)
}

/// Whether `q` (or what it refers to) is the class `std::<name>`.
fn is_std_record(ast: &Ast, q: QualType, name: &str) -> bool {
    let q = ast.types.pointee(q).filter(|_| ast.types.is_reference(q)).unwrap_or(q);
    ast.types.as_record(q).is_some_and(|r| is_named(ast, r, name) && in_std(ast, r))
}

/// clang's `FunctionDecl::isReplaceableGlobalAllocationFunction`: a global
/// `operator new`/`new[]`/`delete`/`delete[]` whose parameters after the
/// first are, in order and each optional, `std::size_t` (deallocation
/// only), `std::align_val_t`, and `const std::nothrow_t&` (not after a
/// size). A program's own definition of one replaces the library's and is
/// still replaceable ([replacement.functions]).
pub(crate) fn is_replaceable_global_alloc(ast: &Ast, d: DeclId) -> bool {
    let decls = &ast.decls;
    let node = decls.get(d);
    if node.kind() != DeclKind::Function {
        return false;
    }
    let alloc = match node.name().as_operator() {
        Some(OverloadedOperator::New | OverloadedOperator::ArrayNew) => true,
        Some(OverloadedOperator::Delete | OverloadedOperator::ArrayDelete) => false,
        _ => return false,
    };
    let mut ctx = node.ctx();
    while let Some(c) = ctx
        && matches!(decls.get(c).kind(), DeclKind::LinkageSpec | DeclKind::Export)
    {
        ctx = decls.get(c).ctx();
    }
    if !ctx.is_some_and(|c| decls.get(c).kind() == DeclKind::TranslationUnit) {
        return false;
    }
    let Some(proto) = node.ty().and_then(|t| ast.types.proto(t)) else {
        return false;
    };
    if proto.variadic() {
        return false;
    }
    let params = ast.types.params(proto);
    if params.is_empty() || params.len() > 4 {
        return false;
    }
    let mut rest = &params[1..];
    let size_t = QualType::unqual(ast.types.target().size_t);
    let mut sized = false;
    if !alloc && rest.first().is_some_and(|&t| ast.types.same_type(t, size_t)) {
        sized = true;
        rest = &rest[1..];
    }
    if rest.first().is_some_and(|&t| {
        ast.types.as_enum(t).is_some_and(|en| is_named(ast, en, "align_val_t") && in_std(ast, en))
    }) {
        rest = &rest[1..];
    }
    if !sized
        && rest
            .first()
            .is_some_and(|&t| ast.types.is_reference(t) && is_std_record(ast, t, "nothrow_t"))
    {
        rest = &rest[1..];
    }
    rest.is_empty()
}

/// Whether `d` is a member of `std::allocator` whose call the Oven treats
/// as a transient allocation ([allocator.members]/6, [expr.const]/5.20:
/// `std::allocator<T>::allocate`/`deallocate`). Always `false` until the
/// templates wave: the library's allocator is a class template, and its
/// calls run as ordinary calls (reaching `::operator new`, which is not
/// constant). spec/oven.md "Transient allocation" records the deferral,
/// with `__builtin_operator_new`/`__builtin_operator_delete`.
pub(crate) fn is_std_allocator_call(_ast: &Ast, _d: DeclId) -> bool {
    false
}
