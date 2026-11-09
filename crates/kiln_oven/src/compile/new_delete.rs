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
use kiln_ast::types::ArgView;
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_source::Loc;

use super::class::Callee;
use super::{Access, Compiler, skip_parens};
use crate::kbc::{Op, cmp, int_aux, new_aux};
use crate::mem::Origin;
use crate::ty::{OTy, TyKind};

impl Compiler<'_> {
    /// The static type of the object a placement argument designates:
    /// the pointee of the argument before its conversions to `void *`
    /// (`kbc::bi::DYN` when it is `void *` itself, unknown statically).
    fn placement_target(&mut self, arg: ExprId) -> u32 {
        let ast = self.ast();
        let mut a = arg;
        loop {
            let n = ast.exprs.get(a);
            match n.kind() {
                ExprKind::ParenExpr => a = ast.exprs.operand(a),
                ExprKind::ImplicitCastExpr
                | ExprKind::CStyleCastExpr
                | ExprKind::CXXStaticCastExpr
                | ExprKind::CXXFunctionalCastExpr
                    if matches!(ast.exprs.cast_kind(a), CastKind::BitCast | CastKind::NoOp) =>
                {
                    a = ast.exprs.operand(a);
                }
                _ => break,
            }
        }
        let t = ast.exprs.get(a).ty();
        match ast.types.pointee(t) {
            Some(p) if !ast.types.is_void(p) => self.ty(p).0,
            _ => crate::kbc::bi::DYN,
        }
    }

    /// `::new (p) T(init)` / `::new (p) T[n]{...}` (P2747; within
    /// namespace std before C++26, `std::construct_at`): checks the
    /// storage `p` designates and begins the lifetime of the new object
    /// there (spec/oven.md "Transient allocation"). The array forms share
    /// [`Compiler::new_expr`]'s initialization; this emits the placement
    /// itself, leaving `p` in `dst`.
    #[allow(clippy::too_many_arguments)]
    fn placement_new(
        &mut self,
        e: ExprId,
        arg: ExprId,
        dst: u32,
        aux: u32,
        cnt: u32,
        k: u64,
        size_loc: Loc,
    ) {
        let ast = self.ast();
        let x = *ast.exprs.new_extra(e);
        let loc = ast.exprs.get(e).loc();
        let elem = self.ty(x.allocated_ty);
        let stat = match self.placement_target(arg) {
            crate::kbc::bi::DYN if self.std_construct && aux & new_aux::ARRAY == 0 => elem.0,
            s => s,
        };
        let p = self.rv(arg);
        self.emit(Op::Mov, 0, &[dst, p]);
        let ops = [dst, elem.0, stat, cnt, k as u32];
        if aux & new_aux::ARRAY != 0 {
            self.emit_site(Op::Placement, aux | new_aux::BOUND, &ops, e, size_loc);
        }
        self.emit_site(Op::Placement, aux, &ops, e, loc);
    }

    /// A direct call of `::operator new(n)` (`alloc`) or `::operator
    /// delete(p)`, or of `__builtin_operator_new`/`delete`: constant only
    /// within `std::allocator<T>::allocate` / `deallocate`, where it
    /// allocates `T[n / sizeof(T)]` (elements outside their lifetime) or
    /// frees such an allocation ([allocator.members], [expr.const]/5.20).
    /// A type-aware one (P2719) takes the size or pointer after its
    /// `std::type_identity<T>` argument; outside `std::allocator` it is
    /// not constant either, with the same note.
    pub(super) fn global_alloc_call(&mut self, e: ExprId, alloc: bool, args: &[ExprId]) -> u32 {
        let begin = self.begin(e);
        let dst = self.reg();
        let ast = self.ast();
        let typed = args
            .first()
            .is_some_and(|&a| is_std_record(ast, ast.exprs.get(a).ty(), "type_identity"));
        let (tag, args) = if typed { (args.first().copied(), &args[1..]) } else { (None, args) };
        let name = if alloc { "allocate" } else { "deallocate" };
        let elem = self.cur_fn.and_then(|f| std_allocator_elem(ast, f, name));
        let (Some(elem), Some(&first)) = (elem, args.first()) else {
            let msg = if alloc { super::builtins::UNTYPED_NEW } else { super::builtins::INVALID };
            self.trap_loc(begin, msg.into());
            return dst;
        };
        let elem = self.ty(elem);
        if let Some(tag) = tag {
            self.effect(tag);
        }
        let v = self.rv(first);
        for &a in &args[1..] {
            self.effect(a);
        }
        if alloc {
            let oi = self.o.push_origin(Origin::Heap(e, 0));
            let aux = new_aux::ARRAY | new_aux::STD;
            self.emit_site(Op::New, aux, &[dst, elem.0, v, oi, 0], e, begin);
        } else {
            self.emit_site(Op::Delete, 8, &[v, elem.0], e, begin);
        }
        dst
    }

    /// `new T`, `new T(init)`, `new T{init}`, `new T[n]...` into `dst`.
    pub(super) fn new_expr(&mut self, e: ExprId, dst: u32) {
        let ast = self.ast();
        let x = *ast.exprs.new_extra(e);
        let Some(op_new) = x.operator_new else {
            self.unsupported(e);
            return;
        };
        // clang 22's `VisitCXXNewExpr` order: `new (std::nothrow)`, the
        // reserved placement form (P2747: in C++26, or within namespace
        // std), any other placement form, a non-replaceable function.
        let mut aux = 0;
        let mut place = None;
        let args: Vec<ExprId> =
            x.placement.map(|l| ast.lists.iter::<ExprId>(l).collect()).unwrap_or_default();
        let nothrow =
            args.len() == 1 && is_std_record(ast, ast.exprs.get(args[0]).ty(), "nothrow_t");
        if nothrow {
            let _ = self.lv(args[0], Access::Addr);
            aux |= new_aux::NOTHROW;
        } else if is_reserved_placement_new(ast, op_new) {
            if !(self.b.cxx26() || self.cur_fn.is_some_and(|f| is_std_function(ast, f))) {
                self.trap(
                    e,
                    "this placement new expression is not supported in constant expressions before C++2c",
                );
                return;
            }
            place = Some(args[0]);
        } else if !args.is_empty() {
            self.trap(e, "this placement new expression is not supported in constant expressions ");
            return;
        } else if !is_replaceable_global_alloc(ast, op_new)
            && !is_type_aware_global_alloc(ast, op_new)
        {
            // [expr.const]/5.20: only a replaceable global allocation
            // function, or a global type-aware one that would be one
            // without its type-identity parameter (P2719), may be called
            // (`note_constexpr_new_non_replaceable`).
            let msg = non_replaceable_note(ast, op_new);
            self.trap(e, &msg);
            return;
        }
        let elem = self.ty(x.allocated_ty);
        if self.ast().types.effective_quals(x.allocated_ty).is_volatile() {
            // [expr.const]/10.8: its accesses are diagnosed (WP-1019).
            aux |= new_aux::VOLATILE;
            self.o.volatile_exprs.insert(e);
        }
        let init = x.init;
        let loc_new = self.ast().exprs.get(e).loc();
        let oi = self.o.push_origin(Origin::Heap(e, 0));
        let Some(size_e) = x.array_size else {
            // A single object (a class object is initialized by its
            // constructor, a `CXXConstructExpr`).
            if let Some(arg) = place {
                self.placement_new(e, arg, dst, 0, 0, 0, loc_new);
            } else {
                self.emit_site(Op::New, aux, &[dst, elem.0, dst, oi, 0], e, loc_new);
            }
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
            /// Each element by a constructor call.
            Each(ExprId),
        }
        let ai = match init.map(|i| (i, self.node(i).kind())) {
            None => ArrInit::None,
            Some((_, ExprKind::ImplicitValueInitExpr)) => ArrInit::Zero,
            Some((i, kind @ (ExprKind::InitListExpr | ExprKind::CXXParenListInitExpr))) => {
                let ast = self.ast();
                let lty = ast.exprs.get(i).ty();
                let Some(k) = ast.types.array_size(lty) else {
                    self.unsupported(i);
                    return;
                };
                // `new T[n](a, b)` (P0960) carries its filler like a list.
                let filler = if kind == ExprKind::InitListExpr {
                    ast.exprs.init_list(i).array_filler
                } else {
                    ast.exprs.paren_list_filler(i)
                };
                let zero_rest = filler.is_some_and(|f| self.is_zero_init(f));
                ArrInit::List { list: i, k, filler, zero_rest }
            }
            Some((i, ExprKind::CXXConstructExpr)) => ArrInit::Each(i),
            Some((i, _)) => {
                self.unsupported(i);
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
        if let Some(arg) = place {
            self.placement_new(e, arg, dst, aux, cnt, k, size_loc);
        } else {
            self.emit_site(Op::New, aux, &[dst, elem.0, cnt, oi, k as u32], e, size_loc);
        }
        if let ArrInit::Each(i) = ai {
            // A null result (`std::nothrow`) has no elements: the loop
            // bound is checked against the count, which the VM leaves as
            // requested, so test the pointer.
            let skip = (aux & new_aux::NOTHROW != 0).then(|| {
                let c = self.reg();
                self.emit(Op::PtrToBool, 0, &[c, dst]);
                self.jmp_if(c, false)
            });
            self.fill_loop(dst, 0, cnt, i, elem, e);
            if let Some(j) = skip {
                self.patch_here(j);
            }
            return;
        }
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
        let paren = ast.exprs.get(list).kind() == ExprKind::CXXParenListInitExpr;
        let inits: Vec<ExprId> = if paren {
            ast.lists.iter::<ExprId>(ast.exprs.paren_list_inits(list)).collect()
        } else {
            ast.lists.iter::<ExprId>(ast.exprs.init_list_inits(list)).collect()
        };
        let lty = self.ety(list);
        let from = if !paren
            && inits.len() == 1
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
        self.emit_ptr_add(int_aux(64, false), &[q, p, k, elem.0], e);
        self.init(f, q, elem);
        self.emit(Op::IAdd, int_aux(64, false), &[k, k, one]);
        let j = self.jmp();
        self.patch(j, top);
        self.patch_here(exit);
        self.next = m;
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
        if !is_replaceable_global_alloc(ast, op_del) && !is_type_aware_global_alloc(ast, op_del) {
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
        let pointee = self.ty(pq);
        let p = self.rv(arg);
        // [expr.delete]/3: through a base class with a virtual destructor,
        // the most-derived object is destroyed and freed (aux 4).
        let vdtor = if array { None } else { self.virtual_dtor(pointee) };
        if let Some(d) = vdtor {
            self.emit_at(Op::Delete, 2 | 4, &[p, pointee.0], e);
            let m = self.next;
            let c = self.reg();
            self.emit(Op::PtrToBool, 0, &[c, p]);
            let skip = self.jmp_if(c, false);
            let loc = self.ast().exprs.get(e).loc();
            let void = self.ety(e);
            self.call_frame(
                e,
                Callee::Virtual(d, pointee),
                void,
                None,
                Some(p),
                &[],
                &[],
                Some(loc),
            );
            self.patch_here(skip);
            self.next = m;
            self.emit_at(Op::Delete, 4, &[p, pointee.0], e);
            return;
        }
        if self.dtor_of(pointee).is_some() {
            // [expr.delete]/6: the destructor (of each element, in reverse
            // order) runs first, once the operand is known to be an object
            // (or array) new created, then the storage is released.
            self.emit_at(Op::Delete, u32::from(array) | 2, &[p, pointee.0], e);
            let m = self.next;
            let mark = self.reg();
            self.emit(Op::ScopeEnter, 0, &[mark]);
            let loc = self.ast().exprs.get(e).loc();
            self.push_cleanup(p, pointee, u32::from(array), loc);
            self.emit(Op::ScopeExit, 0, &[mark]);
            self.next = m;
        }
        self.emit_at(Op::Delete, u32::from(array), &[p, pointee.0], e);
    }
}

/// The definition bits of record `r` (empty when it is incomplete).
impl Compiler<'_> {
    /// The destructor of class `ty`, if it is virtual.
    pub(super) fn virtual_dtor(&mut self, ty: OTy) -> Option<DeclId> {
        let TyKind::Record { decl, .. } = self.kind(ty) else { return None };
        let ast = self.ast();
        let d = ast.destructor(decl)?;
        ast.decls.function(ast.first(d)).is_some_and(|f| f.spec.is_virtual()).then_some(d)
    }
}

pub(super) fn record_bits(ast: &Ast, r: DeclId) -> DefinitionBits {
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

/// Whether `d` is `std::<name>` (through inline namespaces).
pub(crate) fn is_std_named(ast: &Ast, d: DeclId, name: &str) -> bool {
    is_named(ast, d, name) && in_std(ast, d)
}

pub(crate) fn is_named(ast: &Ast, d: DeclId, name: &str) -> bool {
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

/// Whether `d` is a type-aware allocation or deallocation function
/// (P2719) at global scope that is usable in constant evaluation as a
/// replaceable one ([expr.const]/10.18.1 as P2719 amends it; clang
/// `isUsableAsGlobalAllocationFunctionInConstantEvaluation`): exactly
/// `(type_identity<T>, size_t, align_val_t)` /
/// `(type_identity<T>, void*, size_t, align_val_t)`, optionally followed
/// by a `const std::nothrow_t&`.
pub(crate) fn is_type_aware_global_alloc(ast: &Ast, d: DeclId) -> bool {
    let decls = &ast.decls;
    let node = decls.get(d);
    if node.kind() != DeclKind::Function {
        return false;
    }
    let required = match node.name().as_operator() {
        Some(OverloadedOperator::New | OverloadedOperator::ArrayNew) => 3,
        Some(OverloadedOperator::Delete | OverloadedOperator::ArrayDelete) => 4,
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
    let params = ast.types.params(proto);
    if proto.variadic() || !params.first().is_some_and(|&t| is_std_record(ast, t, "type_identity"))
    {
        return false;
    }
    params.len() == required
        || (params.len() == required + 1 && is_std_record(ast, params[required], "nothrow_t"))
}

/// Whether `d` is a replaceable global allocation (`Some(true)`) or
/// deallocation (`Some(false)`) function, called directly as
/// `::operator new(n)` / `::operator delete(p)`, or a global type-aware
/// one usable as such (P2719, `::operator new(std::type_identity<T>{},
/// n, al)`; clang `isUsableAsGlobalAllocationFunctionInConstantEvaluation`
/// routes both to `HandleOperatorNewCall` / `HandleOperatorDeleteCall`).
pub(crate) fn global_alloc_form(ast: &Ast, d: DeclId) -> Option<bool> {
    if !is_replaceable_global_alloc(ast, d) && !is_type_aware_global_alloc(ast, d) {
        return None;
    }
    Some(matches!(
        ast.decls.get(d).name().as_operator(),
        Some(OverloadedOperator::New | OverloadedOperator::ArrayNew)
    ))
}

/// The element type `T` when `f` is `std::allocator<T>::<name>` (clang's
/// `EvalInfo::getStdAllocatorCaller`: a member named `name` of a class
/// template specialization named `allocator` in namespace std whose first
/// template argument is a type).
pub(crate) fn std_allocator_elem(ast: &Ast, f: DeclId, name: &str) -> Option<QualType> {
    let node = ast.decls.get(f);
    if node.kind() != DeclKind::CXXMethod || !is_named(ast, f, name) {
        return None;
    }
    let rec = node.ctx()?;
    if !is_std_named(ast, rec, "allocator") {
        return None;
    }
    let (_, info) = ast.spec_info(rec)?;
    match ast.types.args(info.args).first()?.view() {
        ArgView::Type(q) => Some(q),
        _ => None,
    }
}

/// clang's `FunctionDecl::isReservedGlobalPlacementOperator` for an
/// allocation function: the global `operator new(std::size_t, void*)` /
/// `operator new[](std::size_t, void*)` ([new.delete.placement]).
fn is_reserved_placement_new(ast: &Ast, d: DeclId) -> bool {
    let decls = &ast.decls;
    let node = decls.get(d);
    if node.kind() != DeclKind::Function
        || !matches!(
            node.name().as_operator(),
            Some(OverloadedOperator::New | OverloadedOperator::ArrayNew)
        )
    {
        return false;
    }
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
    match ast.types.params(proto) {
        [_, p] => ast.types.pointee(*p).is_some_and(|v| ast.types.is_void(v)),
        _ => false,
    }
}

/// clang's `CallStackFrame::isStdFunction`: `f` is declared in namespace
/// std (or a class or function nested in it).
fn is_std_function(ast: &Ast, f: DeclId) -> bool {
    let decls = &ast.decls;
    let mut c = decls.get(f).ctx();
    while let Some(d) = c {
        let up = decls.get(d).ctx();
        if decls.get(d).kind() == DeclKind::Namespace
            && is_named(ast, d, "std")
            && up.is_some_and(|tu| decls.get(tu).kind() == DeclKind::TranslationUnit)
        {
            return true;
        }
        c = up;
    }
    false
}
