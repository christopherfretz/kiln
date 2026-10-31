//! `new` and `delete` expressions ([expr.new], [expr.delete]) and the
//! implicitly declared global allocation and deallocation functions
//! ([basic.stc.dynamic.general]/2), spec/sema.md §5.9.
//!
//! clang's `Sema::ActOnCXXNew` / `BuildCXXNew` / `ActOnCXXDelete` for
//! non-class and aggregate element types: class-scope `operator new` /
//! `operator delete` lookup is WP-068b, constructor calls and destructor
//! calls (array cookies) are WP-067b / WP-076.

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, EnumData, FnSpec};
use kiln_ast::expr::{ExprFlags, ExprKind, NewExtra, NewInitStyle, Radix};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{ExcSpec, FuncInfo, Nns, Quals, TagKeyword};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::class::{CtorCall, DeletedAs, DiagSites, InitEntity, Selected, SpecialKind};
use crate::conv::{self, ConvCtx};
use crate::declspec::{Chunk, Declarator};
use crate::init::{InitList, InitValue, Initializer};
use crate::{EvalContext, Sema};

/// The type-id of a new-expression ([expr.new]/1), as parsed: the
/// decl-specifier type and the abstract declarator (a new-declarator,
/// or the declarator of a parenthesized type-id).
#[derive(Clone, Debug)]
pub struct NewTypeId {
    /// The type the decl-specifiers name.
    pub base: QualType,
    /// The declarator; its leading `Array` chunks are the array bounds of
    /// the noptr-new-declarator (the first one may be non-constant).
    pub declarator: Declarator,
    /// The start of the type-id.
    pub loc: Loc,
    /// Written `new (T)`.
    pub parenthesized: bool,
}

/// The new-initializer ([expr.new]/1).
#[derive(Clone, Debug)]
pub enum NewInit {
    /// None: default-initialization.
    None,
    /// `( expression-list )`, possibly empty.
    Paren(Vec<ExprId>, Loc, Loc),
    /// `{ ... }`.
    List(InitList),
}

/// The new-placement arguments `( expression-list )`.
#[derive(Clone, Debug)]
pub struct NewPlacement {
    /// The arguments.
    pub args: Vec<ExprId>,
    /// `(`.
    pub lparen: Loc,
    /// `)`.
    pub rparen: Loc,
}

/// The implicitly declared global allocation functions (clang's
/// `DeclareGlobalNewDelete`).
#[derive(Debug, Default)]
pub(crate) struct GlobalOps {
    declared: bool,
    pub(crate) align_val_t: Option<QualType>,
}

impl Sema<'_, '_> {
    // -- implicit declarations ------------------------------------------------

    /// Declares the replaceable global allocation and deallocation
    /// functions ([new.delete], [basic.stc.dynamic.general]/2) the first
    /// time a new- or delete-expression, or a declaration of a global
    /// `operator new` / `operator delete`, needs them: the four
    /// `operator new` / `operator new[]` forms taking `std::size_t` and
    /// optionally `std::align_val_t`, and the eight `operator delete` /
    /// `operator delete[]` forms, in clang's order. The nothrow forms are
    /// only available when `<new>` declares them.
    pub(crate) fn declare_global_new_delete(&mut self) {
        use OverloadedOperator as O;
        if self.global_ops.declared {
            return;
        }
        self.global_ops.declared = true;
        let al = self.std_align_val_t();
        self.global_ops.align_val_t = Some(al);
        let size = self.size_type();
        let vp = self.ast.types.pointer_to(QualType::VOID);
        for op in [O::New, O::ArrayNew] {
            self.declare_global_op(op, vp, &[size], ExcSpec::None);
            self.declare_global_op(op, vp, &[size, al], ExcSpec::None);
        }
        for op in [O::Delete, O::ArrayDelete] {
            for params in [&[vp][..], &[vp, al], &[vp, size], &[vp, size, al]] {
                self.declare_global_op(op, QualType::VOID, params, ExcSpec::BasicNoexcept);
            }
        }
    }

    fn declare_global_op(
        &mut self,
        op: OverloadedOperator,
        ret: QualType,
        params: &[QualType],
        exc: ExcSpec,
    ) {
        if self.find_global_op(op, params).is_some() {
            return;
        }
        let ty = self.ast.types.function(ret, params, FuncInfo::new(), exc);
        let tu = self.ast.tu;
        let h = DeclHead::new(tu, DeclName::operator(op), Loc::INVALID).flags(DeclFlags::IMPLICIT);
        let f = self.builder().function(h, DeclKind::Function, ty, FnSpec::new(), None);
        let mut ids = Vec::with_capacity(params.len());
        for (i, &p) in params.iter().enumerate() {
            let idx = u16::try_from(i).unwrap_or(u16::MAX);
            let ph = DeclHead::new(f, DeclName::EMPTY, Loc::INVALID).flags(DeclFlags::IMPLICIT);
            let pd = self.builder().param_head(ph, p, idx, 0);
            ids.push(pd);
        }
        let _ = self.builder().set_params(f, &ids);
    }

    /// `std::align_val_t` ([new.syn]): the program's declaration when it
    /// has one, else an implicit `enum class align_val_t : std::size_t` in
    /// (an implicit) namespace `std`, as clang's `getStdAlignValT`.
    fn std_align_val_t(&mut self) -> QualType {
        let tu = self.ast.tu;
        let std = DeclName::ident(self.intern("std"));
        let avt = DeclName::ident(self.intern("align_val_t"));
        let ns = self
            .ast
            .lookup_local(tu, std)
            .to_vec()
            .into_iter()
            .find(|&d| self.kind_of(d) == DeclKind::Namespace);
        if let Some(ns) = ns
            && let Some(e) = self
                .ast
                .lookup_local(ns, avt)
                .to_vec()
                .into_iter()
                .find(|&d| self.kind_of(d) == DeclKind::Enum)
        {
            return self.std_qualified(ns, e);
        }
        let ns = ns.unwrap_or_else(|| {
            let h = DeclHead::new(tu, std, Loc::INVALID).flags(DeclFlags::IMPLICIT);
            self.builder().namespace(h, false, None)
        });
        let size = self.size_type();
        let h = DeclHead::new(ns, avt, Loc::INVALID).flags(DeclFlags::IMPLICIT);
        let e = self.builder().enumeration(h, EnumData::SCOPED, Some((size, size)), None);
        self.std_qualified(ns, e)
    }

    /// The type of tag `d` in namespace `ns`, printed `std::name` as
    /// clang prints the bare `TagType` it uses.
    fn std_qualified(&mut self, ns: DeclId, d: DeclId) -> QualType {
        let t = self.tag_type(d);
        let ns = self.ast.first(ns);
        let n = self.ast.types.nns(Nns::Namespace { prefix: None, ns });
        self.ast.types.elaborated(TagKeyword::None, Some(n), t)
    }

    /// The global `operator op` whose parameter types are `params`.
    fn find_global_op(&self, op: OverloadedOperator, params: &[QualType]) -> Option<DeclId> {
        let tu = self.ast.tu;
        self.ast.lookup_local(tu, DeclName::operator(op)).iter().find(|&f| {
            self.ast.decls.get(f).kind() == DeclKind::Function
                && self.fn_params(f).is_some_and(|(ps, variadic)| {
                    !variadic
                        && ps.len() == params.len()
                        && ps.iter().zip(params).all(|(a, b)| self.ast.types.same_type(*a, *b))
                })
        })
    }

    /// The parameter types of function `f` and whether it is variadic.
    pub(crate) fn fn_params(&self, f: DeclId) -> Option<(Vec<QualType>, bool)> {
        let ty = self.ast.decls.get(f).ty()?;
        // The written (sugared) proto when the type is one, so that
        // diagnostics print `std::align_val_t` as clang does.
        let p = self.ast.types.proto_of(ty.ty()).or_else(|| self.ast.types.proto(ty))?;
        Some((self.ast.types.params(p).to_vec(), p.info().variadic()))
    }

    /// A namespace-scope declaration of `operator op` of type `ty`
    /// (clang's `CheckOperatorNewDeleteDeclaration` callers): declares
    /// the implicit functions first, so that a declaration of a
    /// replaceable one redeclares it, and returns the type the declaration
    /// gets (a deallocation function without an exception specification
    /// is `noexcept`, [except.spec]/9) and the previous declaration.
    pub(crate) fn act_on_global_new_delete_decl(
        &mut self,
        op: OverloadedOperator,
        ty: QualType,
    ) -> (QualType, Option<DeclId>) {
        self.declare_global_new_delete();
        let mut ty = ty;
        if matches!(op, OverloadedOperator::Delete | OverloadedOperator::ArrayDelete)
            && self.written_exc_spec(ty) == ExcSpec::None
        {
            ty = self.with_basic_noexcept(ty);
        }
        let tu = self.ast.tu;
        let cands = self.ast.lookup_local(tu, DeclName::operator(op)).to_vec();
        let prev = cands.into_iter().find(|&p| {
            let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
            self.same_ignoring_exc_spec(pty, ty)
        });
        (ty, prev)
    }

    /// Function type `ty` with a `noexcept` exception specification.
    pub(crate) fn with_basic_noexcept(&mut self, ty: QualType) -> QualType {
        let Some(p) =
            self.ast.types.proto_of(ty.ty()).or_else(|| self.ast.types.proto(ty)).copied()
        else {
            return ty;
        };
        let params = self.ast.types.params(&p).to_vec();
        self.ast.types.function(p.ret(), &params, p.info(), ExcSpec::BasicNoexcept)
    }

    // -- new ------------------------------------------------------------------

    /// A new-expression `::`(opt) `new` new-placement(opt) type
    /// new-initializer(opt) ([expr.new]); `loc` is its first token (`::`
    /// or `new`) and `end` its last.
    #[allow(clippy::too_many_lines, reason = "clang's BuildCXXNew, step by step")]
    pub fn act_on_cxx_new(
        &mut self,
        global: bool,
        loc: Loc,
        placement: Option<NewPlacement>,
        tid: NewTypeId,
        init: NewInit,
        end: Loc,
    ) -> ExprId {
        self.declare_global_new_delete();
        let mut kids: Vec<ExprId> = Vec::new();
        if let Some(p) = &placement {
            kids.extend(&p.args);
        }
        match &init {
            NewInit::Paren(v, ..) => kids.extend(v),
            NewInit::List(l) => kids.extend(list_exprs(l)),
            NewInit::None => {}
        }
        let mut d = tid.declarator;
        // The noptr-new-declarator: the leading array chunks (innermost
        // first, so in written order).
        let n_arrays = d.chunks.iter().take_while(|c| matches!(c, Chunk::Array { .. })).count();
        let arrays: Vec<(Option<ExprId>, Loc)> = d
            .chunks
            .drain(..n_arrays)
            .filter_map(|c| match c {
                Chunk::Array { size, loc } => Some((size, loc)),
                _ => None,
            })
            .collect();
        kids.extend(arrays.iter().filter_map(|a| a.0));
        // [expr.new]/7: only the first bound may be omitted, and only
        // with an initializer.
        if let Some(&(None, at)) = arrays.first()
            && matches!(init, NewInit::None)
        {
            self.error(DiagId::sema_new_array_size_unknown, at);
            return self.recover_err(&kids, loc);
        }
        // Every later bound is a converted constant expression of type
        // `std::size_t` ([expr.new]/6).
        let mut later: Vec<(Option<u64>, Loc)> = Vec::new();
        for &(size, at) in arrays.iter().skip(1) {
            match size {
                None => later.push((None, at)),
                Some(e) => match self.new_later_bound(e) {
                    Some(n) => later.push((Some(n), self.begin_loc(e))),
                    None => return self.recover_err(&kids, loc),
                },
            }
        }
        if d.invalid || self.is_error_type(tid.base) {
            return self.recover_err(&kids, loc);
        }
        let mut elem = self.type_for_declarator(tid.base, &d);
        if self.is_error_type(elem) {
            return self.recover_err(&kids, loc);
        }
        // [dcl.type.auto.deduct]: `new auto(e)`.
        if self.is_undeduced_auto(elem) {
            if let Some(&(_, at)) = arrays.first() {
                self.error(DiagId::sema_new_array_of_auto, at);
                return self.recover_err(&kids, loc);
            }
            match self.deduce_new_auto(elem, &init, loc) {
                Some(t) => elem = t,
                None => return self.recover_err(&kids, loc),
            }
        } else if self.contains_auto(elem) {
            self.unsupported("this form of 'auto' in a new-expression", tid.loc);
            return self.recover_err(&kids, loc);
        }
        // The later bounds, outermost last.
        for &(n, at) in later.iter().rev() {
            elem = match n {
                Some(n) => {
                    if self.array_too_large(n, elem) {
                        let msg = self.diag(DiagId::sema_array_too_large, at).arg(n.to_string());
                        self.emit(msg);
                        return self.recover_err(&kids, loc);
                    }
                    self.ast.types.constant_array(elem, n, None)
                }
                None => self.ast.types.incomplete_array(elem),
            };
        }
        // [expr.new]/5: a typedef of an array type allocates an array.
        let mut first: Option<(Option<ExprId>, Loc)> = arrays.first().copied();
        let mut synthesized_size = None;
        if first.is_none()
            && self.ast.types.is_array(elem)
            && let Some(n) = self.ast.types.array_size(elem)
        {
            let size_t = self.size_type();
            let lit = self.builder().int_literal(size_t, n, Radix::Decimal, tid.loc);
            synthesized_size = Some((lit, n));
            elem = self.ast.types.element(elem).unwrap_or(QualType::ERROR);
            first = Some((None, tid.loc));
        }
        let array_form = first.is_some();
        if !self.check_allocated_type(elem, tid.loc) {
            return self.recover_err(&kids, loc);
        }
        // The first bound: implicitly converted to `std::size_t`
        // ([expr.new]/8, C++14 rules), checked against the maximum array
        // size when constant.
        let mut array_size = synthesized_size.map(|(e, _)| e);
        let mut const_bound: Option<u64> = synthesized_size.map(|(_, n)| n);
        if let Some((Some(e), _)) = first {
            match self.convert_new_size(e, elem) {
                Some((c, v)) => {
                    array_size = Some(c);
                    const_bound = v;
                    if tid.parenthesized && v.is_none() {
                        let at = self.begin_loc(e);
                        self.error(DiagId::sema_new_paren_array_nonconst, at);
                    }
                }
                None => return self.recover_err(&kids, loc),
            }
        }
        let (size, align) = crate::target::size_align(&self.ast, elem, &self.abi).unwrap_or((1, 1));
        let _ = size;
        let mut pass_align = align > self.abi.default_new_align;
        // Class element types: constructors (WP-067b) and array cookies
        // (WP-076) are not supported yet.
        let class_unsupported = self.new_class_unsupported(elem, array_form, &init, tid.loc);
        // The allocation function ([expr.new]/12-19).
        let op = if array_form { OverloadedOperator::ArrayNew } else { OverloadedOperator::New };
        // A class element type's own `operator new` (unless `::new`) is
        // looked up in the class first ([expr.new]/12).
        let alloc = self.alloc_fns(elem, op, global, tid.loc);
        let place_args: Vec<ExprId> =
            placement.as_ref().map(|p| p.args.clone()).unwrap_or_default();
        let Some((op_new, with_align)) =
            self.find_operator_new(&alloc.fns, &place_args, pass_align)
        else {
            let ptr_like = place_args.len() == 1 && {
                let t = self.ast.exprs.get(place_args[0]).ty();
                self.ast.types.is_array(t)
                    || self.ast.types.pointee(t).is_some_and(|p| !self.ast.types.is_function(p))
            };
            let name = format!("'{}'", op_spelling(op));
            if ptr_like {
                let msg = self.diag(DiagId::sema_placement_new_no_operator, loc).arg(name);
                self.emit(msg);
            } else {
                let msg = self
                    .diag(DiagId::sema_ovl_no_viable_function_in_call, loc)
                    .arg(op_spelling(op));
                let msg = self.operator_new_candidate_notes(msg, &alloc.fns, &place_args);
                self.emit(msg);
            }
            return self.recover_err(&kids, loc);
        };
        pass_align = with_align;
        if self.is_deleted_fn(op_new) {
            let own = self.ast.decls.function(op_new).is_some_and(|x| x.spec.is_deleted());
            let note = self
                .diag(DiagId::note_candidate_deleted, self.ast.decls.get(op_new).loc())
                .arg(u8::from(!own));
            let msg = self
                .diag(DiagId::sema_call_deleted_function, loc)
                .arg(self.decl_arg(op_new))
                .arg(self.delete_message_suffix(op_new))
                .with_note(note);
            self.emit(msg);
            return self.recover_err(&kids, loc);
        }
        self.mark_used(op_new);
        let (params, variadic) = self.fn_params(op_new).unwrap_or_default();
        let skip = 1 + usize::from(pass_align);
        let mut conv_place = Vec::with_capacity(place_args.len());
        for (i, &a) in place_args.iter().enumerate() {
            let c = match params.get(skip + i) {
                Some(&pt) => {
                    let to =
                        if self.ast.types.is_reference(pt) { pt } else { conv::strip_cv(self, pt) };
                    self.convert_for_init(a, to, ConvCtx::Argument)
                }
                None if variadic => {
                    let c = conv::decay(self, a);
                    conv::lvalue_to_rvalue(self, c)
                }
                None => a,
            };
            conv_place.push(c);
        }
        // The matching deallocation function, called if the
        // initialization throws ([expr.new]/28).
        let op_delete = self.matching_operator_delete(
            elem,
            global,
            array_form,
            placement.is_some(),
            &params,
            pass_align,
            tid.loc,
        );
        if let Some(od) = op_delete {
            self.mark_used(od);
        }
        // The initializer ([expr.new]/23).
        let style = match init {
            NewInit::None => NewInitStyle::None,
            NewInit::Paren(..) => NewInitStyle::Parens,
            NewInit::List(_) => NewInitStyle::Braces,
        };
        let init_expr = if class_unsupported {
            None
        } else {
            match self.new_initializer(elem, array_form, first, const_bound, init, tid.loc) {
                Ok((e, completed)) => {
                    if let Some(n) = completed {
                        let size_t = self.size_type();
                        array_size =
                            Some(self.builder().int_literal(size_t, n, Radix::Decimal, tid.loc));
                    }
                    e
                }
                Err(()) => return self.recover_err(&kids, loc),
            }
        };
        let mut flags = ExprFlags::NONE;
        if global {
            flags = flags.with(ExprFlags::GLOBAL);
        }
        if array_form {
            flags = flags.with(ExprFlags::ARRAY_FORM);
        }
        if pass_align {
            flags = flags.with(ExprFlags::PASS_ALIGN);
        }
        let placement_list = placement.map(|_| self.ast.lists.push(&conv_place));
        let ty = self.ast.types.pointer_to(elem);
        let extra = NewExtra {
            allocated_ty: elem,
            array_size,
            init: init_expr,
            placement: placement_list,
            operator_new: Some(self.ast.latest(op_new)),
            operator_delete: op_delete.map(|d| self.ast.latest(d)),
            type_loc: tid.loc,
        };
        self.builder().cxx_new(ty, flags, style, extra, loc, end)
    }

    /// `new auto(e)` / `new auto{e}`: the type deduced from the single
    /// initializer ([expr.new]/2, [dcl.type.auto.deduct]).
    fn deduce_new_auto(
        &mut self,
        declared: QualType,
        init: &NewInit,
        loc: Loc,
    ) -> Option<QualType> {
        let exprs: Vec<ExprId> = match init {
            NewInit::None => Vec::new(),
            NewInit::Paren(v, ..) => v.clone(),
            NewInit::List(l) => list_exprs(l),
        };
        let items = match init {
            NewInit::List(l) => l.items.len(),
            NewInit::Paren(v, ..) => v.len(),
            NewInit::None => 0,
        };
        if items == 0 {
            let msg =
                self.diag(DiagId::sema_new_auto_requires_ctor_arg, loc).arg(self.ty_arg(declared));
            self.emit(msg);
            return None;
        }
        if items > 1 || exprs.len() != 1 {
            let at = exprs.get(1).map_or(loc, |&e| self.begin_loc(e));
            let msg = self.diag(DiagId::sema_new_auto_multiple_args, at).arg(self.ty_arg(declared));
            self.emit(msg);
            return None;
        }
        let e = exprs[0];
        let ety = self.ast.exprs.get(e).ty();
        if self.is_error_type(ety) {
            return None;
        }
        let t = &self.ast.types;
        let base = if t.is_array(ety) {
            let el = t.element(ety).unwrap_or(QualType::ERROR);
            self.ast.types.pointer_to(el)
        } else if t.is_function(ety) {
            self.ast.types.pointer_to(ety)
        } else if t.effective_quals(ety).is_empty() {
            ety
        } else {
            self.ast.types.canonical(ety).without_quals()
        };
        let q = declared.quals();
        Some(if q.is_empty() { base } else { self.ast.types.qualify(base, q) })
    }

    /// True if `t` is, or is a compound type built on, an undeduced
    /// placeholder (`new auto*(p)`, unsupported).
    fn contains_auto(&self, t: QualType) -> bool {
        let mut t = t;
        loop {
            if self.is_undeduced_auto(t) {
                return true;
            }
            let ty = &self.ast.types;
            match ty.pointee(t).or_else(|| ty.element(t)) {
                Some(n) => t = n,
                None => return false,
            }
        }
    }

    /// clang's `CheckAllocatedType` ([expr.new]/1): the allocated type is
    /// a complete object type.
    fn check_allocated_type(&mut self, t: QualType, at: Loc) -> bool {
        let id = if self.ast.types.is_function(t) {
            DiagId::sema_new_function_type
        } else if self.ast.types.is_reference(t) {
            DiagId::sema_new_reference_type
        } else if !self.require_complete(t, at) {
            let msg = self.diag(DiagId::sema_new_incomplete_type, at).arg(self.ty_arg(t));
            self.emit(self.incomplete_note(msg, t));
            return false;
        } else {
            return !self.require_non_abstract_alloc(at, t);
        };
        let msg = self.diag(id, at).arg(self.ty_arg(t));
        self.emit(msg);
        false
    }

    /// A later array bound of a new-declarator: a converted constant
    /// expression of type `std::size_t` ([expr.new]/6); `None` after a
    /// diagnostic.
    fn new_later_bound(&mut self, e: ExprId) -> Option<u64> {
        let ety = self.ast.exprs.get(e).ty();
        if self.is_error_type(ety) {
            return None;
        }
        let at = self.begin_loc(e);
        let size_t = self.size_type();
        let t = &self.ast.types;
        let integral =
            t.is_integral(ety) || t.as_enum(ety).is_some_and(|en| !self.is_scoped_enum(en));
        if !integral {
            let id = if t.is_floating(ety) {
                DiagId::sema_cce_conversion_not_allowed
            } else {
                DiagId::sema_cce_not_implicitly_convertible
            };
            let msg = self.diag(id, at).arg(self.ty_arg(ety)).arg(self.ty_arg(size_t));
            self.emit(msg);
            return None;
        }
        let c = conv::lvalue_to_rvalue(self, e);
        let c = match conv::implicit_convert(self, c, size_t, ConvCtx::Initialization) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, at);
                return None;
            }
        };
        match self.eval_int(c, EvalContext::converted(at)) {
            Ok(v) => {
                let n = u64::try_from(v).ok()?;
                Some(n)
            }
            Err(f) => {
                let d = self.diag(DiagId::sema_expr_not_cce, at).arg("array size");
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                None
            }
        }
    }

    /// The first array bound converted to `std::size_t` ([expr.new]/8)
    /// and its value when it is a constant; `None` after a diagnostic.
    fn convert_new_size(&mut self, e: ExprId, elem: QualType) -> Option<(ExprId, Option<u64>)> {
        let ety = self.ast.exprs.get(e).ty();
        if self.is_error_type(ety) {
            return None;
        }
        let at = self.begin_loc(e);
        let size_t = self.size_type();
        let t = &self.ast.types;
        if t.is_pointer(ety) {
            let msg = self
                .diag(DiagId::sema_converting_pointer_to_int, at)
                .arg(self.ty_arg(ety))
                .arg(self.ty_arg(size_t));
            self.emit(msg);
            return None;
        }
        let scoped = t.as_enum(ety).is_some_and(|en| self.is_scoped_enum(en));
        let class = t.as_record(ety).is_some();
        if scoped
            || self.is_nullptr_type(ety)
            || class && !conv::has_conversion_functions(self, ety)
        {
            let msg = self
                .diag(DiagId::sema_converting_to_incompatible, at)
                .arg(self.ty_arg(ety))
                .arg(self.ty_arg(size_t));
            self.emit(msg);
            return None;
        }
        if class {
            self.unsupported("a class-type array bound in a new-expression", at);
            return None;
        }
        let c = match conv::implicit_convert(self, e, size_t, ConvCtx::Initialization) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, at);
                return None;
            }
        };
        let v = self.fold_int(c).and_then(|v| u64::try_from(v).ok());
        if let Some(n) = v
            && self.array_too_large(n, elem)
        {
            let msg = self.diag(DiagId::sema_array_too_large, at).arg(n.to_string());
            self.emit(msg);
            return None;
        }
        Some((c, v))
    }

    /// clang's `ConstantArrayType::getNumAddressingBits` check: `n`
    /// elements of `elem` need more than 61 address bits (the 64-bit
    /// targets' maximum object size).
    fn array_too_large(&self, n: u64, elem: QualType) -> bool {
        const MAX_BITS: u32 = 61;
        let Some((size, _)) = crate::target::size_align(&self.ast, elem, &self.abi) else {
            return false;
        };
        if n == 0 || size == 0 {
            return false;
        }
        let bits = if size.is_power_of_two() {
            (64 - n.leading_zeros()) + size.trailing_zeros()
        } else {
            let total = u128::from(n) * u128::from(size);
            128 - total.leading_zeros()
        };
        bits > MAX_BITS
    }

    /// Diagnoses the default-initialization of a const class object
    /// without a user-provided default constructor ([dcl.init.general]/8);
    /// true if one was reported (the node is built without an
    /// initializer).
    fn new_class_unsupported(
        &mut self,
        elem: QualType,
        array_form: bool,
        init: &NewInit,
        at: Loc,
    ) -> bool {
        let Some(rec) = self.ast.types.as_record(elem) else { return false };
        let Some(def) = self.ast.definition(rec) else { return false };
        let defn = self.ast.decls.record(def).map(|r| r.defn).unwrap_or_default();
        // `new_initializer` diagnoses the const array.
        if !matches!(init, NewInit::None)
            || array_form
            || !self.ast.types.effective_quals(elem).is_const()
            || defn.contains(DefinitionBits::CAN_CONST_DEFAULT_INIT)
        {
            return false;
        }
        let msg = self.diag(DiagId::sema_default_init_const_no_ctor, at).arg(self.ty_arg(elem));
        self.emit(msg);
        true
    }

    /// The default- or value-initialization of the class elements of a
    /// new-expression by their default constructor ([expr.new]/23,
    /// [dcl.init.general]/7, /9): a `CXXConstructExpr` of type `ty` (the
    /// element, or the array of them). `None` when the element type is
    /// not a class, or a default-initialization needs no constructor call
    /// (a trivial default constructor). `Some(Err)` after an error.
    fn new_class_default(
        &mut self,
        elem: QualType,
        ty: QualType,
        value_init: bool,
        at: Loc,
    ) -> Option<Result<ExprId, ()>> {
        let rec = self.ast.types.as_record(elem)?;
        self.instantiate_for_init(rec, at);
        let class = self.ast.definition(rec)?;
        let class = conv::bases::complete_class(self, class)?;
        let defn = self.ast.decls.record(class).map(|r| r.defn).unwrap_or_default();
        if defn.contains(DefinitionBits::TRIVIAL_DEFAULT_CTOR) {
            // Default-initialization does nothing; value-initialization
            // zero-initializes ([dcl.init.general]/9.1.2).
            return value_init.then(|| Ok(self.builder().implicit_value_init(ty)));
        }
        let sel = self.select_special(class, SpecialKind::DefaultCtor, Quals::NONE, Quals::NONE);
        let call = CtorCall { value_init, ..CtorCall::new(&[]) };
        let sites = DiagSites { access: at, deleted: at, deleted_as: DeletedAs::Call };
        let e = self.use_ctor(
            sel,
            ty,
            class,
            SpecialKind::DefaultCtor,
            call,
            at,
            InitEntity::Var,
            sites,
        );
        Some(e.ok_or(()))
    }

    /// The allocation function for `op` with `place` arguments, and
    /// whether it takes the alignment ([expr.new]/19: first with the
    /// alignment argument when the type is over-aligned, then without).
    fn find_operator_new(
        &mut self,
        cands: &[DeclId],
        place: &[ExprId],
        pass_align: bool,
    ) -> Option<(DeclId, bool)> {
        let size_t = self.size_type();
        let al = self.global_ops.align_val_t.unwrap_or(size_t);
        let tries: &[bool] = if pass_align { &[true, false] } else { &[false] };
        for &with_align in tries {
            let mut lead = vec![size_t];
            if with_align {
                lead.push(al);
            }
            let mut best: Option<(DeclId, usize)> = None;
            for &f in cands {
                let Some((ps, variadic)) = self.fn_params(f) else { continue };
                let n = lead.len() + place.len();
                if ps.len() > n || (ps.len() < n && !variadic) {
                    continue;
                }
                if !ps.iter().zip(&lead).all(|(a, b)| self.ast.types.same_type(*a, *b)) {
                    continue;
                }
                let mut exact = 0;
                let mut ok = true;
                for (i, &a) in place.iter().enumerate() {
                    if let Some(&p) = ps.get(lead.len() + i) {
                        match self.placement_arg_match(a, p) {
                            Some(true) => exact += 1,
                            Some(false) => {}
                            None => ok = false,
                        }
                    }
                }
                if ok && best.is_none_or(|(_, e)| exact > e) {
                    best = Some((f, exact));
                }
            }
            if let Some((f, _)) = best {
                return Some((f, with_align));
            }
        }
        None
    }

    /// clang's candidate notes for a failed `operator new` lookup: the
    /// candidates with a bad argument conversion first, then those with
    /// the wrong number of parameters, closest arity first (an
    /// approximation of `CompareOverloadCandidatesForDisplay`).
    fn operator_new_candidate_notes(
        &mut self,
        mut msg: kiln_diag::Diag,
        cands: &[DeclId],
        place: &[ExprId],
    ) -> kiln_diag::Diag {
        let have = 1 + place.len();
        let size_t = self.size_type();
        let mut bad: Vec<(bool, Loc, String)> = Vec::new();
        let mut arity: Vec<(usize, bool, Loc, String)> = Vec::new();
        for &f in cands {
            let Some((ps, variadic)) = self.fn_params(f) else { continue };
            let implicit = self.ast.decls.get(f).flags().contains(DeclFlags::IMPLICIT);
            let fl = if implicit { Loc::INVALID } else { self.ast.decls.get(f).loc() };
            let n = ps.len();
            if have < n || (have > n && !variadic) {
                // Default arguments of allocation functions are not considered
                // (spec/sema.md §5.9): every parameter is required.
                let detail = self.arity_detail(f, n, n, have, variadic);
                arity.push((n.abs_diff(have), implicit, fl, detail));
                continue;
            }
            let args: Vec<(QualType, Option<ExprId>)> = std::iter::once((size_t, None))
                .chain(place.iter().map(|&a| (self.ast.exprs.get(a).ty(), Some(a))))
                .collect();
            for (i, (&(aty, a), &p)) in args.iter().zip(&ps).enumerate() {
                let ok = match a {
                    Some(a) => self.placement_arg_match(a, p).is_some(),
                    None => self.ast.types.same_type(aty, p),
                };
                if ok {
                    continue;
                }
                let mut detail = format!(
                    "no known conversion from {} to {} for {} argument",
                    self.ty_arg(aty),
                    self.ty_arg(p),
                    crate::expr::ordinal(i + 1)
                );
                let t = &self.ast.types;
                if t.is_pointer(aty)
                    && t.pointee(aty).is_some_and(|x| {
                        t.same_unqualified(x, p) || t.is_arithmetic(x) && t.is_arithmetic(p)
                    })
                {
                    detail.push_str("; dereference the argument with *");
                } else if t.is_pointer(p)
                    && t.pointee(p).is_some_and(|x| t.same_unqualified(x, aty))
                {
                    detail.push_str("; take the address of the argument with &");
                }
                bad.push((implicit, fl, detail));
                break;
            }
        }
        bad.sort_by_key(|b| b.0);
        arity.sort_by_key(|a| (a.0, !a.1));
        let notes: Vec<(Loc, String)> = bad
            .into_iter()
            .map(|(_, l, d)| (l, d))
            .chain(arity.into_iter().map(|(_, _, l, d)| (l, d)))
            .collect();
        for (l, d) in notes {
            let note = self.diag(DiagId::note_ovl_candidate_not_viable, l).arg(d);
            msg = msg.with_note(note);
        }
        msg
    }

    /// An approximation of overload resolution's viability check for a
    /// placement argument `a` against parameter type `p`: `Some(true)` for
    /// an exact match, `Some(false)` for a standard conversion, `None`
    /// when not viable.
    pub(crate) fn placement_arg_match(&self, a: ExprId, p: QualType) -> Option<bool> {
        let t = &self.ast.types;
        let at = conv::strip_cv(self, self.ast.exprs.get(a).ty());
        let p = if t.is_reference(p) { t.pointee(p).unwrap_or(p) } else { p };
        let p = conv::strip_cv(self, p);
        if t.same_type(at, p) {
            return Some(true);
        }
        let arith = |q: QualType| {
            t.is_arithmetic(q) || t.as_enum(q).is_some_and(|en| !self.is_scoped_enum(en))
        };
        if arith(at) && t.is_arithmetic(p) {
            return Some(false);
        }
        if let Some(pp) = t.pointee(p).filter(|_| t.is_pointer(p)) {
            if self.is_nullptr_type(at) {
                return Some(false);
            }
            let src = if t.is_array(at) {
                t.element(at)
            } else if t.is_pointer(at) {
                t.pointee(at)
            } else {
                None
            }?;
            if t.is_void(pp) && !t.is_function(src) {
                let sq = t.effective_quals(src);
                let pq = t.effective_quals(pp);
                return (sq.is_const() <= pq.is_const() && sq.is_volatile() <= pq.is_volatile())
                    .then_some(false);
            }
            return t.same_unqualified(src, pp).then_some(t.is_array(at));
        }
        None
    }

    /// The deallocation function matching an allocation function with
    /// parameter types `new_params` ([expr.new]/28), looked up like the
    /// allocation function (in the class of `elem` first unless
    /// `global`): for a placement allocation function, the one with the
    /// same additional parameters; otherwise the usual deallocation
    /// function.
    #[allow(
        clippy::too_many_arguments,
        clippy::fn_params_excessive_bools,
        reason = "the new-expression's forms, as clang's FindAllocationFunctions"
    )]
    fn matching_operator_delete(
        &mut self,
        elem: QualType,
        global: bool,
        array: bool,
        placement: bool,
        new_params: &[QualType],
        pass_align: bool,
        loc: Loc,
    ) -> Option<DeclId> {
        let op = if array { OverloadedOperator::ArrayDelete } else { OverloadedOperator::Delete };
        let found = self.alloc_fns(elem, op, global, loc);
        if !placement {
            return self.usual_delete(&found.fns, found.in_class, pass_align, true).map(|u| u.0);
        }
        let vp = self.ast.types.pointer_to(QualType::VOID);
        let mut params = vec![vp];
        params.extend(new_params.iter().skip(1));
        found.fns.iter().copied().find(|&f| {
            self.fn_params(f).is_some_and(|(ps, variadic)| {
                !variadic
                    && ps.len() == params.len()
                    && ps.iter().zip(&params).all(|(a, b)| self.ast.types.same_type(*a, *b))
            })
        })
    }

    /// The semantic initializer of a new-expression ([expr.new]/23) and,
    /// for `new T[]{...}`, the bound the initializer gives.
    fn new_initializer(
        &mut self,
        elem: QualType,
        array_form: bool,
        first: Option<(Option<ExprId>, Loc)>,
        const_bound: Option<u64>,
        init: NewInit,
        at: Loc,
    ) -> Result<(Option<ExprId>, Option<u64>), ()> {
        let array_ty = |s: &mut Self| match const_bound {
            Some(n) => s.ast.types.constant_array(elem, n, None),
            None => s.ast.types.incomplete_array(elem),
        };
        match init {
            NewInit::None => {
                if self.ast.types.effective_quals(elem).is_const()
                    && (array_form || self.ast.types.as_record(elem).is_none())
                {
                    // clang names the whole array type (`const int[]`).
                    let ty = if array_form { array_ty(self) } else { elem };
                    let msg = self.diag(DiagId::sema_default_init_const, at).arg(self.ty_arg(ty));
                    self.emit(msg);
                    return Err(());
                }
                let ty = if array_form { array_ty(self) } else { elem };
                match self.new_class_default(elem, ty, false, at) {
                    Some(Ok(e)) => Ok((Some(e), None)),
                    Some(Err(())) => Err(()),
                    None => Ok((None, None)),
                }
            }
            NewInit::Paren(v, l, r) => {
                if array_form {
                    if !v.is_empty() {
                        self.unsupported("parenthesized initialization of an array", l);
                        return Err(());
                    }
                    let t = array_ty(self);
                    return match self.new_class_default(elem, t, true, at) {
                        Some(Ok(e)) => Ok((Some(e), None)),
                        Some(Err(())) => Err(()),
                        None => Ok((Some(self.builder().implicit_value_init(t)), None)),
                    };
                }
                if self.ast.types.as_record(elem).is_some() {
                    // A constructor call, or an aggregate's parenthesized
                    // initialization ([dcl.init.general]/16.6).
                    let e = if v.is_empty() {
                        match self.value_init_class(elem, at) {
                            Some(e) => e,
                            None => self.builder().implicit_value_init(elem),
                        }
                    } else {
                        self.perform_init(elem, Initializer::Paren(v, l, r), at, false)
                    };
                    if self.is_err_expr(e) {
                        return Err(());
                    }
                    return Ok((Some(e), None));
                }
                match v.len() {
                    0 => Ok((Some(self.builder().implicit_value_init(elem)), None)),
                    1 => {
                        let e = self.perform_init(elem, Initializer::Paren(v, l, r), at, false);
                        Ok((Some(e), None))
                    }
                    _ => {
                        let msg = self.diag(DiagId::sema_excess_initializers, at).arg("2");
                        self.emit(msg);
                        Err(())
                    }
                }
            }
            NewInit::List(list) => {
                if !array_form {
                    let e = self.perform_init(elem, Initializer::List(list), at, true);
                    return Ok((Some(e), None));
                }
                let lbrace = list.lbrace;
                let rbrace = list.rbrace;
                let t = array_ty(self);
                let e = self.perform_init(t, Initializer::List(list), lbrace, true);
                if self.is_err_expr(e) {
                    return Err(());
                }
                let e = if const_bound.is_none() { self.with_array_filler(e, rbrace) } else { e };
                // `new T[]{...}`: the bound is the initializer's length.
                let completed = if matches!(first, Some((None, _))) {
                    let ety = self.ast.exprs.get(e).ty();
                    self.ast.types.array_size(ety)
                } else {
                    None
                };
                Ok((Some(e), completed))
            }
        }
    }

    /// An array `InitListExpr` for a runtime bound: clang's
    /// `InitListChecker` always gives it an array filler (the elements
    /// past the initializers are value-initialized at run time), except
    /// when it is initialized from a string literal.
    fn with_array_filler(&mut self, e: ExprId, rbrace: Loc) -> ExprId {
        let n = *self.ast.exprs.get(e);
        if n.kind() != ExprKind::InitListExpr || !self.ast.types.is_array(n.ty()) {
            return e;
        }
        let extra = *self.ast.exprs.init_list(e);
        if extra.array_filler.is_some() {
            return e;
        }
        let inits: Vec<ExprId> = {
            let l = self.ast.exprs.init_list_inits(e);
            self.ast.lists.iter(l).collect()
        };
        if inits.len() == 1
            && self
                .string_lit_of(inits[0])
                .is_some_and(|_| self.ast.types.is_array(self.ast.exprs.get(inits[0]).ty()))
        {
            return e;
        }
        let elem = self.ast.types.element(n.ty()).unwrap_or(QualType::ERROR);
        let filler = self.filler(elem, rbrace);
        self.builder().init_list(
            n.ty(),
            &inits,
            Some(filler),
            extra.union_field,
            n.loc(),
            extra.rbrace,
        )
    }

    // -- delete ---------------------------------------------------------------

    /// A delete-expression `::`(opt) `delete` (`[]`)(opt) cast-expression
    /// ([expr.delete]); `loc` is its first token. `cxx26` selects the
    /// C++26 rule that deleting a pointer to an incomplete type is
    /// ill-formed (P3144).
    #[allow(clippy::too_many_lines, reason = "clang's ActOnCXXDelete, step by step")]
    pub fn act_on_cxx_delete(
        &mut self,
        global: bool,
        array_written: bool,
        loc: Loc,
        operand: ExprId,
        cxx26: bool,
    ) -> ExprId {
        self.declare_global_new_delete();
        let mut flags = ExprFlags::NONE;
        if global {
            flags = flags.with(ExprFlags::GLOBAL);
        }
        if array_written {
            flags = flags.with(ExprFlags::ARRAY_FORM).with(ExprFlags::ARRAY_AS_WRITTEN);
        }
        if self.is_err_expr(operand) {
            return self.builder().cxx_delete(
                operand,
                None,
                flags,
                false,
                false,
                QualType::VOID,
                loc,
            );
        }
        let ety = self.ast.exprs.get(operand).ty();
        let t = &self.ast.types;
        if t.as_record(ety).is_some() && conv::has_conversion_functions(self, ety) {
            // [expr.delete]/2: contextually implicitly converted to a
            // pointer by a conversion function.
            self.unsupported("deleting an object of class type through a conversion function", loc);
            return self.recover_err(&[operand], loc);
        }
        let pointee = if t.is_pointer(ety) { t.pointee(ety) } else { None };
        let Some(pointee) = pointee.filter(|p| !t.is_function(*p)) else {
            let msg = self.diag(DiagId::sema_delete_operand, loc).arg(self.ty_arg(ety));
            self.emit(msg);
            return self.recover_err(&[operand], loc);
        };
        let mut complete = true;
        if self.ast.types.is_void(pointee) {
            complete = false;
            if cxx26 {
                let msg = self.diag(DiagId::sema_delete_incomplete, loc).arg(self.ty_arg(pointee));
                self.emit(msg);
                return self.recover_err(&[operand], loc);
            }
            let msg = self.diag(DiagId::sema_delete_void_ptr_warn, loc).arg(self.ty_arg(ety));
            self.emit(msg);
        } else if !self.require_complete(pointee, loc) {
            complete = false;
            let id = if cxx26 {
                DiagId::sema_delete_incomplete
            } else {
                DiagId::sema_delete_incomplete_warn
            };
            let msg = self.diag(id, loc).arg(self.ty_arg(pointee));
            self.emit(self.incomplete_note(msg, pointee));
            if cxx26 {
                return self.recover_err(&[operand], loc);
            }
        }
        let mut array = array_written;
        if !array && self.ast.types.is_array(pointee) {
            // [expr.delete]/2: clang deletes the array and warns.
            let msg = self.diag(DiagId::sema_delete_array_type, loc).arg(self.ty_arg(ety));
            self.emit(msg);
            array = true;
            flags = flags.with(ExprFlags::ARRAY_FORM);
        }
        let mut elem = pointee;
        while self.ast.types.is_array(elem) {
            elem = self.ast.types.element(elem).unwrap_or(QualType::ERROR);
        }
        if complete
            && let Some(rec) = self.ast.types.as_record(elem)
            && let Some(def) = self.ast.definition(rec)
        {
            let defn = self.ast.decls.record(def).map(|r| r.defn).unwrap_or_default();
            if !defn.contains(DefinitionBits::TRIVIAL_DTOR)
                && let Selected::Found(m) =
                    self.select_special(def, SpecialKind::Dtor, Quals::NONE, Quals::NONE)
            {
                // [expr.delete]/6: the destructor is invoked (odr-used,
                // [basic.def.odr]/8).
                self.mark_function_referenced(m, loc);
            }
        }
        let align = if complete {
            crate::target::size_align(&self.ast, elem, &self.abi).map_or(1, |(_, a)| a)
        } else {
            1
        };
        let aligned = align > self.abi.default_new_align;
        // [expr.delete]/10: a usual deallocation function; the sized one
        // when the type is complete (and no array cookie is needed: a
        // `delete[]` of a type without one is unsized, clang).
        let want_sized = complete && !array;
        let op = if array { OverloadedOperator::ArrayDelete } else { OverloadedOperator::Delete };
        // Unless `::delete`, the class's own `operator delete` is looked
        // up first ([expr.delete]/9).
        let found = self.alloc_fns(elem, op, global || !complete, loc);
        let usual = self.usual_delete(&found.fns, found.in_class, aligned, want_sized);
        if usual.is_none() && found.in_class {
            self.no_suitable_delete(op, elem, &found.fns, loc);
            return self.recover_err(&[operand], loc);
        }
        let op_delete = usual.map(|u| u.0);
        let sized = usual.is_some_and(|u| u.1);
        if let Some(f) = op_delete {
            self.mark_used(f);
        }
        let c = conv::lvalue_to_rvalue(self, operand);
        let op_delete = op_delete.map(|d| self.ast.latest(d));
        self.builder().cxx_delete(c, op_delete, flags, sized, aligned, QualType::VOID, loc)
    }
}

/// The expressions of the top-level clauses of `l`.
fn list_exprs(l: &InitList) -> Vec<ExprId> {
    l.items
        .iter()
        .filter_map(|i| if let InitValue::Expr(e) = i.value { Some(e) } else { None })
        .collect()
}

/// `operator new` / `operator new[]`.
fn op_spelling(op: OverloadedOperator) -> &'static str {
    match op {
        OverloadedOperator::ArrayNew => "operator new[]",
        OverloadedOperator::Delete => "operator delete",
        OverloadedOperator::ArrayDelete => "operator delete[]",
        _ => "operator new",
    }
}
