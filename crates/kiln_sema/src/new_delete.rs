//! `new` and `delete` expressions ([expr.new], [expr.delete]) and the
//! implicitly declared global allocation and deallocation functions
//! ([basic.stc.dynamic.general]/2), spec/sema.md §5.9.
//!
//! clang's `Sema::ActOnCXXNew` / `BuildCXXNew` / `ActOnCXXDelete` for
//! non-class and aggregate element types: class-scope `operator new` /
//! `operator delete` lookup is WP-068b, constructor calls and destructor
//! calls (array cookies) are WP-067b / WP-076.

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, EnumData, FnSpec};
use kiln_ast::expr::{ExprDeps, ExprFlags, ExprKind, NewExtra, NewInitStyle, Radix};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{ExcSpec, FuncInfo, Nns, Quals, TagKeyword};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::class::{CtorCall, DeletedAs, DiagSites, InitEntity, Selected, SpecialKind};
use crate::conv::{self, ConvCtx};
use crate::declspec::{Chunk, DeclSpec, Declarator};
use crate::init::{InitList, InitValue, Initializer};
use crate::switch_conv::ContextualConv;
use crate::templ::ctad::CtadInit;
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
        // The first new-expression may sit in a template definition; the
        // implicit declarations (above all an implicit `std::align_val_t`)
        // are not members of it and must not get dependent types, or every
        // later `operator new(std::size_t, std::align_val_t)` accepts any
        // argument (`::operator new(n, nothrow)` was ambiguous after a
        // `requires { ::new T; }` in `<concepts>`).
        let saved = std::mem::take(&mut self.ast.templates.dependent_depth);
        // The implicit declarations are attached to the global module
        // ([basic.stc.dynamic.general]/2), even when the first use sits in
        // a module purview (else `operator delete` mangled as `_ZW1Mdl…`).
        let start = self.ast.decls.next_id();
        let owner = self.ast.modules.owner(start);
        let global = kiln_ast::modules::Owner { purview: false, exported: false, ..owner };
        self.ast.modules.set_owner_from(start, global);
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
        let end = self.ast.decls.next_id();
        self.ast.modules.set_owner_from(end, owner);
        self.ast.templates.dependent_depth = saved;
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
        self.warn_redundant_parens(None, &d);
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
        // Parentheses counted the drained chunks (WP-191 `ParenType`).
        for p in &mut d.parens {
            p.0 = p.0.saturating_sub(n_arrays);
        }
        kids.extend(arrays.iter().filter_map(|a| a.0));
        // [expr.new]/7: only the first bound may be omitted, and only
        // with an initializer.
        if let Some(&(None, at)) = arrays.first()
            && matches!(init, NewInit::None)
        {
            self.error(DiagId::sema_new_array_size_unknown, at);
            return self.recover_err(&kids, loc);
        }
        // [dcl.type.class.deduct]/2: a placeholder for a deduced class
        // type is not an array element (clang reports the innermost
        // array: a later bound's `[`, else the first bound).
        if d.chunks.is_empty()
            && self.is_ctad_placeholder(tid.base)
            && let Some(&(size, bracket)) = arrays.last()
        {
            let at = match size {
                _ if arrays.len() > 1 => bracket,
                Some(e) => self.begin_loc(e),
                None => tid.loc,
            };
            let d = self.diag(DiagId::sema_ctad_compound_type, at).arg("form array of");
            self.emit(d);
            return self.recover_err(&kids, loc);
        }
        // A dependent new-expression is checked at instantiation ([temp.res]).
        let kids_dep = kids.iter().any(|&k| {
            self.ast
                .exprs
                .get(k)
                .deps()
                .intersects(ExprDeps::TYPE_VALUE_INST | ExprDeps::UNEXPANDED_PACK)
        });
        if kids_dep || self.is_dependent_type(tid.base) {
            if d.invalid || self.is_error_type(tid.base) {
                return self.recover_err(&kids, loc);
            }
            // The later bounds are part of the allocated type.
            let rest = std::mem::take(&mut d.chunks);
            d.chunks = arrays
                .iter()
                .skip(1)
                .map(|&(size, loc)| Chunk::Array { size, loc })
                .chain(rest)
                .collect();
            let later = arrays.len().saturating_sub(1);
            for p in d.parens.iter_mut().filter(|p| p.0 > 0) {
                p.0 += later;
            }
            let elem = self.type_for_declarator(tid.base, &d);
            if self.is_error_type(elem) {
                return self.recover_err(&kids, loc);
            }
            return self.dependent_new(
                global,
                loc,
                placement,
                elem,
                arrays.first().copied(),
                init,
                tid.loc,
                end,
            );
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
        // [dcl.fct]/6: `new F` of a qualified function typedef `F`.
        elem = self.check_qualified_fn_type(elem, &DeclSpec::new(tid.loc), &d);
        if self.is_error_type(elem) {
            return self.recover_err(&kids, loc);
        }
        // [dcl.type.class.deduct]/2: `new C(args)` (WP-071b).
        if self.is_ctad_placeholder(elem) {
            let t = match &init {
                NewInit::None => self.deduce_ctad(elem, &[], &[], CtadInit::Direct, tid.loc),
                NewInit::Paren(es, l, r) => {
                    let i = Initializer::Paren(es.clone(), *l, *r);
                    self.ctad_for_initializer(elem, &i, false, tid.loc)
                }
                NewInit::List(l) => {
                    let i = Initializer::List(l.clone());
                    self.ctad_for_initializer(elem, &i, true, tid.loc)
                }
            };
            match t {
                Some(t) => elem = t,
                None => return self.recover_err(&kids, loc),
            }
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
                        let msg = self
                            .diag(DiagId::sema_array_too_large, at)
                            .arg(self.element_count_text(n));
                        self.emit(msg);
                        return self.recover_err(&kids, loc);
                    }
                    self.ast.types.constant_array(elem, n, None)
                }
                None => self.ast.types.incomplete_array(elem),
            };
        }
        if self.is_dependent_type(elem) {
            let first = arrays.first().copied();
            return self.dependent_new(global, loc, placement, elem, first, init, tid.loc, end);
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
            match self.convert_new_size(e, elem, loc) {
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
            self.find_operator_new(&alloc.fns, &place_args, pass_align).or_else(|| {
                self.find_operator_new_template(&alloc.templates, &place_args, pass_align, loc)
            })
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
        // A class's own allocation function must be accessible
        // ([expr.new]/19; clang's `CheckAllocationAccess`). As clang, an
        // inaccessible one does not invalidate the expression, so the
        // matching deallocation function is still checked below.
        if let Some(naming) = alloc.naming {
            self.check_member_access(op_new, naming, None, loc);
        }
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
        self.mark_used_at(op_new, loc);
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
        // clang treats any allocation function other than `(size_t)` as a
        // placement one here, so an aligned `operator new` is matched by
        // the unsized aligned `operator delete` (`isPlacementNew`).
        let op_delete = self.matching_operator_delete(
            elem,
            global,
            array_form,
            placement.is_some() || params.len() != 1 || variadic,
            &params,
            pass_align,
            tid.loc,
        );
        if let Some((od, naming)) = op_delete {
            // The deallocation function is access-checked at the
            // new-expression too ([expr.new]/28, [class.access]/1); clang
            // looks it up only when exceptions are enabled.
            if self.exceptions
                && let Some(naming) = naming
            {
                self.check_member_access_distinct(od, naming, loc);
            }
            self.mark_used(od);
        }
        let op_delete = op_delete.map(|d| d.0);
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
        if array_form {
            // [expr.new]/27: the destructor of an array's elements is
            // potentially invoked (odr-used, [basic.def.odr]/8).
            // clang `BuildCXXNew` diagnoses both at the type's location,
            // an inaccessible one as for a temporary.
            let at = tid.loc;
            let access = DiagId::sema_temp_dtor_access;
            if self.mark_element_dtor_referenced(elem, (at, access), at).is_none() {
                return self.recover_err(&kids, loc);
            }
        }
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

    /// A new-expression whose allocated type or operands are dependent,
    /// kept as written for instantiation (clang's `BuildCXXNew` for a
    /// dependent type): the allocated type `elem` with its later bounds,
    /// the first bound as written, the placement arguments and the
    /// initializer as a `ParenListExpr` or dependent `InitListExpr`; no
    /// allocation function is looked up.
    #[allow(clippy::too_many_arguments, reason = "the parts of the new-expression")]
    fn dependent_new(
        &mut self,
        global: bool,
        loc: Loc,
        placement: Option<NewPlacement>,
        elem: QualType,
        first: Option<(Option<ExprId>, Loc)>,
        init: NewInit,
        type_loc: Loc,
        end: Loc,
    ) -> ExprId {
        let (style, init_expr) = match init {
            NewInit::None => (NewInitStyle::None, None),
            NewInit::Paren(v, open, rparen) => {
                // Braced arguments keep their nested lists.
                let v = self.dependent_args(&v);
                (NewInitStyle::Parens, Some(self.builder().paren_list(&v, open, rparen)))
            }
            NewInit::List(l) => (NewInitStyle::Braces, Some(self.dependent_init_list(&l))),
        };
        let mut flags = ExprFlags::NONE;
        if global {
            flags = flags.with(ExprFlags::GLOBAL);
        }
        if first.is_some() {
            flags = flags.with(ExprFlags::ARRAY_FORM);
        }
        let placement_list = placement.map(|p| self.ast.lists.push(&p.args));
        // `new auto(e)` with a dependent `e`: the type is deduced later.
        let ty = if self.contains_auto(elem) {
            QualType::DEPENDENT
        } else {
            self.ast.types.pointer_to(elem)
        };
        let extra = NewExtra {
            allocated_ty: elem,
            array_size: first.and_then(|f| f.0),
            init: init_expr,
            placement: placement_list,
            operator_new: None,
            operator_delete: None,
            type_loc,
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
    fn convert_new_size(
        &mut self,
        e: ExprId,
        elem: QualType,
        new_loc: Loc,
    ) -> Option<(ExprId, Option<u64>)> {
        let mut e = e;
        let mut ety = self.ast.exprs.get(e).ty();
        if self.is_error_type(ety) {
            return None;
        }
        let at = self.begin_loc(e);
        let size_t = self.size_type();
        if self.std < kiln_lex::Std::Cxx14 {
            // C++11 [expr.new]/6: contextually implicitly converted to an
            // integral or unscoped enumeration type (`switch_conv.rs`),
            // diagnosed at the new-expression.
            let mut ok = self.contextual_target_matches(ContextualConv::ArraySize, ety);
            if !ok && self.ast.types.as_record(ety).is_some() {
                match self.contextual_class_conversion(ContextualConv::ArraySize, e, new_loc) {
                    Some(c) if self.is_err_expr(c) => return None,
                    Some(c) => {
                        let conv_ty = self.ast.exprs.get(c).ty();
                        if self.std < kiln_lex::Std::Cxx11 {
                            // C++98 [expr.new]/6 wants an integral or
                            // enumeration type: clang
                            // `ext_array_size_conversion`.
                            let is_enum = self.ast.types.as_enum(conv_ty).is_some();
                            let msg = self
                                .diag(DiagId::sema_array_size_conversion, new_loc)
                                .arg(self.ty_arg(ety))
                                .arg(if is_enum { "1" } else { "0" })
                                .arg(self.ty_arg(conv_ty));
                            self.emit(msg);
                        }
                        e = c;
                        ety = conv_ty;
                        ok = true;
                    }
                    None => {}
                }
            }
            if !ok {
                let unscoped = if self.std >= kiln_lex::Std::Cxx11 { "0" } else { "1" };
                let msg = self
                    .diag(DiagId::sema_array_size_not_integral, new_loc)
                    .arg(unscoped)
                    .arg(self.ty_arg(ety));
                self.emit(msg);
                return None;
            }
        }
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
        let c = match conv::implicit_convert(self, e, size_t, ConvCtx::Initialization) {
            Ok(c) => c,
            Err(err) if class && err.id == DiagId::sema_no_viable_conversion => {
                // C++14: clang `PerformImplicitConversion` (an
                // `AssignmentAction::Converting`) reports no usable
                // conversion function as an incompatible conversion.
                let msg = self
                    .diag(DiagId::sema_converting_to_incompatible, at)
                    .arg(self.ty_arg(ety))
                    .arg(self.ty_arg(size_t));
                self.emit(msg);
                return None;
            }
            Err(err) if class && err.id == DiagId::sema_conv_deleted_ctor => {
                // ... and a deleted one as a use of a deleted function.
                let mut msg = self.diag(DiagId::sema_use_deleted_function, at).arg("");
                for n in err.notes {
                    msg = msg.with_note(n);
                }
                self.emit(msg);
                return None;
            }
            Err(err) => {
                self.emit_conv_error(err, at);
                return None;
            }
        };
        let v = self.fold_int(c).and_then(|v| u64::try_from(v).ok());
        if let Some(n) = v
            && self.array_too_large(n, elem)
        {
            let msg = self.diag(DiagId::sema_array_too_large, at).arg(self.element_count_text(n));
            self.emit(msg);
            return None;
        }
        Some((c, v))
    }

    /// An element count as `array is too large` prints it: clang 22
    /// groups the digits with `'` separators (`18'446'744'073'709'551'615`),
    /// clang 19 does not.
    pub(crate) fn element_count_text(&self, n: impl std::fmt::Display) -> String {
        let s = n.to_string();
        if self.clang_compat < 22 {
            return s;
        }
        let (sign, digits) = s.strip_prefix('-').map_or(("", s.as_str()), |d| ("-", d));
        let mut out = String::from(sign);
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push('\'');
            }
            out.push(c);
        }
        out
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

    /// Whether the elements of (array) type `elem` are of a
    /// const-default-constructible class ([dcl.init.general]/7: a
    /// user-provided default constructor, or every member has a default
    /// member initializer or is itself const-default-constructible).
    fn const_default_constructible(&mut self, elem: QualType, at: Loc) -> bool {
        let Some(rec) = self.ast.types.as_record(self.base_element(elem)) else { return false };
        self.instantiate_for_init(rec, at);
        let Some(def) = self.ast.definition(rec) else { return false };
        conv::bases::complete_class(self, def)
            .is_some_and(|c| self.defn_bits(c).contains(DefinitionBits::CAN_CONST_DEFAULT_INIT))
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
    /// new-expression (or of a value-initialized array member) by their
    /// default constructor ([expr.new]/23, [dcl.init.general]/7, /9): a
    /// `CXXConstructExpr` of type `ty` (the element, or the array of
    /// them; `elem` may itself be an array). `None` when the element type is
    /// not a class, or a default-initialization needs no constructor call
    /// (a trivial default constructor). `Some(Err)` after an error.
    pub(crate) fn new_class_default(
        &mut self,
        elem: QualType,
        ty: QualType,
        value_init: bool,
        at: Loc,
    ) -> Option<Result<ExprId, ()>> {
        // `new K[n][3]`: the elements of the inner arrays are constructed
        // too (clang's `CXXConstructExpr` of type `K[n][3]`).
        let rec = self.ast.types.as_record(self.base_element(elem))?;
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
    pub(crate) fn find_operator_new(
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

    /// The specialization of a class-scope allocation function template
    /// among `tmpls` called with `place` ([expr.new]/19-20, [temp.over]):
    /// overload resolution with a `size_t` prvalue, then a
    /// `std::align_val_t` one when `pass_align` (first with it, then
    /// without), then the placement arguments. Templates are tried only
    /// when no non-template function matches.
    pub(crate) fn find_operator_new_template(
        &mut self,
        tmpls: &[DeclId],
        place: &[ExprId],
        pass_align: bool,
        loc: Loc,
    ) -> Option<(DeclId, bool)> {
        use crate::overload::{ArgInfo, CandidateKind, IcsOptions, OvlOutcome, SetKind};
        use kiln_ast::expr::ValueCategory;
        if tmpls.is_empty() {
            return None;
        }
        let size_t = self.size_type();
        let al = self.global_ops.align_val_t;
        let tries: &[bool] = if pass_align && al.is_some() { &[true, false] } else { &[false] };
        for &with_align in tries {
            let mut set = self.begin_set(SetKind::Normal, loc);
            set.args.push(ArgInfo::of_type(size_t, ValueCategory::PRValue));
            if with_align && let Some(al) = al {
                set.args.push(ArgInfo::of_type(al, ValueCategory::PRValue));
            }
            for &a in place {
                set.args.push(self.arg_info(a));
            }
            self.add_found_candidates(
                &mut set,
                tmpls,
                CandidateKind::Function,
                IcsOptions::NONE,
                false,
            );
            let out = self.resolve_set(&mut set);
            let f = match out {
                OvlOutcome::Success(b) | OvlOutcome::Deleted(b) => Some(set.cand(b).func()),
                _ => None,
            };
            self.end_set(set);
            if let Some(f) = f {
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
    ) -> Option<(DeclId, Option<DeclId>)> {
        let op = if array { OverloadedOperator::ArrayDelete } else { OverloadedOperator::Delete };
        let found = self.alloc_fns(elem, op, global, loc);
        let naming = found.naming;
        if !placement {
            return self
                .usual_delete(&found.fns, found.in_class, pass_align, true, false)
                .map(|u| (u.0, naming));
        }
        let vp = self.ast.types.pointer_to(QualType::VOID);
        let mut params = vec![vp];
        params.extend(new_params.iter().skip(1));
        found
            .fns
            .iter()
            .copied()
            .find(|&f| {
                self.fn_params(f).is_some_and(|(ps, variadic)| {
                    !variadic
                        && ps.len() == params.len()
                        && ps.iter().zip(&params).all(|(a, b)| self.ast.types.same_type(*a, *b))
                })
            })
            .map(|f| (f, naming))
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
                // An array of a const-default-constructible class is
                // default-initialized element by element ([dcl.init.general]/7).
                if self.ast.types.effective_quals(elem).is_const()
                    && (array_form || self.ast.types.as_record(elem).is_none())
                    && !(array_form && self.const_default_constructible(elem, at))
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
                    let t = array_ty(self);
                    if !v.is_empty() {
                        return self.new_paren_array_init(elem, t, &v, first, l, r);
                    }
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
                        // The initialized entity is "a new value".
                        let entity = self.init_entity.replace(crate::init::entity::NEW);
                        let e = self.perform_init(elem, Initializer::Paren(v, l, r), at, false);
                        self.init_entity = entity;
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
                    let scalar = self.ast.types.as_record(elem).is_none();
                    let entity = if scalar {
                        self.init_entity.replace(crate::init::entity::NEW)
                    } else {
                        None
                    };
                    let e = self.perform_init(elem, Initializer::List(list), at, true);
                    if scalar {
                        self.init_entity = entity;
                    }
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

    /// `new T[n](args...)`: a parenthesized aggregate initialization of the
    /// array (P0960). Like clang's `TryOrBuildParenListInitialization` for
    /// a new-expression, the `CXXParenListInitExpr` always carries an array
    /// filler (a runtime bound may exceed the arguments); with no written
    /// bound its length completes the array type.
    fn new_paren_array_init(
        &mut self,
        elem: QualType,
        t: QualType,
        args: &[ExprId],
        first: Option<(Option<ExprId>, Loc)>,
        l: Loc,
        r: Loc,
    ) -> Result<(Option<ExprId>, Option<u64>), ()> {
        let e = self.paren_aggregate_init(t, args, l, r);
        if self.is_err_expr(e) || self.ast.exprs.get(e).kind() != ExprKind::CXXParenListInitExpr {
            return Err(());
        }
        let ety = self.ast.exprs.get(e).ty();
        let e = if self.ast.exprs.paren_list_filler(e).is_none() {
            let inits: Vec<ExprId> = {
                let l = self.ast.exprs.paren_list_inits(e);
                self.ast.lists.iter(l).collect()
            };
            let filler = self.paren_value_init(elem, l);
            self.builder().paren_list_init(ety, &inits, Some(filler), l, r)
        } else {
            e
        };
        let completed =
            if matches!(first, Some((None, _))) { self.ast.types.array_size(ety) } else { None };
        Ok((Some(e), completed))
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
        self.delete_cxx26 = cxx26;
        if self.ast.exprs.get(operand).is_type_dependent() {
            // [temp.dep.expr]: checked when instantiated (clang
            // `ActOnCXXDelete` skips a type-dependent operand).
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
        let mut operand = operand;
        let mut ety = self.ast.exprs.get(operand).ty();
        if self.ast.types.as_record(ety).is_some() {
            // [expr.delete]/2: contextually implicitly converted to a
            // pointer to object type (`switch_conv.rs`).
            match self.contextual_class_conversion(ContextualConv::Delete, operand, loc) {
                Some(c) if self.is_err_expr(c) => return c,
                Some(c) => {
                    operand = c;
                    ety = self.ast.exprs.get(c).ty();
                }
                None => {}
            }
        }
        let t = &self.ast.types;
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
        // [expr.delete]/6: the destructor is invoked.
        let destructed = if complete {
            let at = self.expr_loc(operand);
            match self.mark_element_dtor_referenced(elem, (at, DiagId::sema_access_dtor), loc) {
                Some(d) => d,
                None => return self.recover_err(&[operand], loc),
            }
        } else {
            false
        };
        let align = if complete {
            crate::target::size_align(&self.ast, elem, &self.abi).map_or(1, |(_, a)| a)
        } else {
            1
        };
        let aligned = align > self.abi.default_new_align;
        // [expr.delete]/10: a usual deallocation function; the sized one
        // when the type is complete and the size is known: a `delete[]`
        // knows it only from an array cookie, i.e. for elements with a
        // non-trivial destructor (clang `CanProvideSize`).
        let want_sized = complete && (!array || destructed);
        let op = if array { OverloadedOperator::ArrayDelete } else { OverloadedOperator::Delete };
        // Unless `::delete`, the class's own `operator delete` is looked
        // up first ([expr.delete]/9).
        let found = self.alloc_fns(elem, op, global || !complete, loc);
        let usual = self.usual_delete(&found.fns, found.in_class, aligned, want_sized, !array);
        if usual.is_none() && found.in_class {
            self.no_suitable_delete(op, elem, &found.fns, loc);
            return self.recover_err(&[operand], loc);
        }
        let op_delete = usual.map(|u| u.0);
        let sized = usual.is_some_and(|u| u.1);
        // A class's own deallocation function must be accessible
        // ([expr.delete]/12; clang's `CheckAllocationAccess`): a
        // substitution failure in SFINAE context (libc++'s
        // `__can_be_deleted`, constraining `shared_ptr(Y*)`).
        if let (Some(f), Some(naming)) = (op_delete, found.naming)
            && !self.check_member_access(f, naming, None, loc)
        {
            return self.recover_err(&[operand], loc);
        }
        if let Some(f) = op_delete {
            self.mark_used(f);
        }
        let mut c = conv::lvalue_to_rvalue(self, operand);
        if let Some(f) = op_delete
            && self.ast.is_destroying_operator_delete(f)
            && !self.has_virtual_dtor(elem)
        {
            // [expr.delete]/7.1, /11: a destroying operator delete called
            // non-virtually is passed the operand converted to its first
            // parameter's type `C*` (clang `ActOnCXXDelete`); the
            // destructor is not invoked.
            if let Some((ps, _)) = self.fn_params(f)
                && let Some(&p0) = ps.first()
                && !self.ast.types.same_type(self.ety(c), p0)
            {
                match conv::implicit_convert(self, c, p0, ConvCtx::Argument) {
                    Ok(x) => c = x,
                    Err(err) => {
                        let msg = err.diag(self, c);
                        self.emit(msg);
                        return self.recover_err(&[c], loc);
                    }
                }
            }
        }
        let op_delete = op_delete.map(|d| self.ast.latest(d));
        self.builder().cxx_delete(c, op_delete, flags, sized, aligned, QualType::VOID, loc)
    }
}

/// The expressions of the top-level clauses of `l`.
impl Sema<'_, '_> {
    /// Whether class type `t` has a virtual destructor ([class.dtor]/10).
    fn has_virtual_dtor(&self, t: QualType) -> bool {
        let Some(def) = self.ast.types.as_record(t).and_then(|r| self.ast.definition(r)) else {
            return false;
        };
        self.ast.destructor(def).is_some_and(|d| crate::class::is_virtual(&self.ast, d))
    }

    /// Marks the destructor of `elem`'s innermost element class as
    /// referenced (odr-used, [basic.def.odr]/8) when it is non-trivial,
    /// returning whether it is; `None` after diagnosing a deleted
    /// destructor at `loc` (clang `DiagnoseUseOfDecl`, the expression
    /// fails). An inaccessible one is diagnosed with `access` = (location,
    /// diagnostic), clang `CheckDestructorAccess`.
    fn mark_element_dtor_referenced(
        &mut self,
        mut elem: QualType,
        access: (Loc, DiagId),
        loc: Loc,
    ) -> Option<bool> {
        while let Some(e) = self.ast.types.element(elem) {
            elem = e;
        }
        let Some(def) = self.ast.types.as_record(elem).and_then(|r| self.ast.definition(r)) else {
            return Some(false);
        };
        let cflags = self.ast.decls.get(def).flags();
        if cflags.contains(DeclFlags::INVALID) || cflags.contains(DeclFlags::DEPENDENT_CTX) {
            return Some(false);
        }
        let trivial = self
            .ast
            .decls
            .record(def)
            .is_some_and(|r| r.defn.contains(DefinitionBits::TRIVIAL_DTOR));
        match self.select_special(def, SpecialKind::Dtor, Quals::NONE, Quals::NONE) {
            Selected::Found(m) => {
                let msg = self.special_member_access(m, def, |s, prot| {
                    s.diag(access.1, access.0).arg(s.ty_arg(elem)).arg(u8::from(prot))
                });
                if let Some(msg) = msg {
                    self.emit(msg);
                }
                if !trivial {
                    self.mark_function_referenced(m, loc);
                }
            }
            Selected::Deleted(m) => {
                self.diagnose_deleted_use(m, loc);
                return None;
            }
            _ => {}
        }
        Some(!trivial)
    }
}

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

impl Sema<'_, '_> {
    /// clang `CheckDestructor`: the deallocation function the deleting
    /// destructor of virtual destructor `dtor` of `class` calls
    /// ([class.dtor]/16, `FindDeallocationFunctionForDestructor`),
    /// recorded in `cold.dtor_delete` for codegen (WP-080).
    pub(crate) fn check_destructor_delete(&mut self, dtor: DeclId, class: DeclId) {
        let first = self.ast.first(dtor);
        let is_virtual = self
            .ast
            .redecls_of(first)
            .any(|r| self.ast.decls.function(r).is_some_and(|f| f.spec.is_virtual()));
        if !is_virtual || self.ast.decls.cold.dtor_delete.contains(first) {
            return;
        }
        let loc = self.ast.decls.get(dtor).loc();
        let Some((op, naming)) = self.dtor_dealloc_fn(class, loc) else { return };
        // The deallocation function is access-checked at the destructor,
        // from the destructor ([class.dtor]/16, clang `CheckDestructor` →
        // `FindDeallocationFunction`).
        if let Some(naming) = naming {
            self.check_member_access(op, naming, None, loc);
        }
        self.mark_function_referenced(op, loc);
        if self.ast.is_destroying_operator_delete(op)
            && let Some((ps, _)) = self.fn_params(op)
            && let Some(&p0) = ps.first()
        {
            // A destroying operator delete is passed `this` converted to
            // its first parameter's type ([expr.delete]/7.1; clang
            // `getOperatorDeleteThisArg`).
            let cls = self.tag_type(class);
            let this_ty = self.ast.types.pointer_to(cls);
            let this = self.builder().this_expr(this_ty, true, loc);
            if let Ok(arg) = conv::implicit_convert(self, this, p0, ConvCtx::Argument) {
                let _ = self.ast.decls.cold.dtor_delete_this.insert(first, arg);
            }
        }
        let op = self.ast.latest(op);
        let _ = self.ast.decls.cold.dtor_delete.insert(first, op);
    }

    /// The deallocation function the deleting destructor of class
    /// `class` calls and the class naming it (`None` for a global one),
    /// looked up from the class scope ([class.dtor]/16). `None` when
    /// class-scope lookup finds no usual deallocation function.
    fn dtor_dealloc_fn(&mut self, class: DeclId, loc: Loc) -> Option<(DeclId, Option<DeclId>)> {
        self.declare_global_new_delete();
        let ty = self.tag_type(class);
        let align = self.complete_class_align(ty);
        let aligned = align > self.abi.default_new_align;
        let found = self.alloc_fns(ty, OverloadedOperator::Delete, false, loc);
        let usual = self.usual_delete(&found.fns, found.in_class, aligned, true, true)?;
        Some((usual.0, found.naming))
    }

    /// Whether a virtual destructor of `class` finds an unambiguous,
    /// accessible deallocation function; one that does not is defined as
    /// deleted ([class.dtor]/7.3).
    pub(crate) fn virtual_dtor_delete_usable(&mut self, class: DeclId) -> bool {
        let loc = self.ast.decls.get(class).loc();
        match self.dtor_dealloc_fn(class, loc) {
            None => false,
            Some((op, Some(naming))) => self.member_accessible_from(op, naming, class),
            Some((_, None)) => true,
        }
    }

    /// `alignof` of class type `ty` (declaration `class`), through the
    /// per-Sema [`kiln_abi::LayoutCache`]: the layout of a complete class,
    /// and that of every class it contains by value (complete too,
    /// [class.mem]/7, [class.derived]/2), is final for the rest of the TU,
    /// so a lattice of `n` classes is laid out once instead of once per
    /// class (clang caches `ASTRecordLayout`s the same way). A class still
    /// being defined (its implicit destructor is declared before `}`) is
    /// laid out over the cached layouts of its bases and members but not
    /// cached itself (`Layouter::into_complete_cache`; WP-450, WP-428).
    fn complete_class_align(&mut self, ty: QualType) -> u64 {
        let cache = std::mem::take(&mut self.layout_cache);
        let mut lay = kiln_abi::Layouter::with_cache(&self.ast, &self.abi, cache);
        let align = lay.type_layout(ty).align;
        self.layout_cache = lay.into_complete_cache();
        align
    }
}
