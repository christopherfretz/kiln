//! Substitution into types (spec/sema-templates.md §5.3): a type without
//! instantiation-dependent or unexpanded-pack flags is its own result;
//! otherwise it is rebuilt through the type context's constructors (or
//! the declarator checks, so `T*` with `T = int&` is diagnosed as when
//! written).

use kiln_ast::decl::DeclKind;
use kiln_ast::types::{
    ArgView, ExcSpec, ExprOperand, TemplateArg, TemplateName, TypeFlags, TypeOfOperand, TypeView,
};
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::{SubstFail, SubstResult, Substituter};
use crate::Sema;
use crate::declspec::{Chunk, Declarator, DeclaratorContext};
use crate::lookup::LookupKind;
use crate::templ::actions::TemplateIdRef;
use crate::templ::args::{ArgLookup, LevelMode};
use crate::templ::written::WrittenKey;

impl Substituter<'_, '_, '_> {
    /// Substitutes into type `q` (its qualifiers kept, collapsing as the
    /// constructors do).
    pub fn ty(&mut self, q: QualType, loc: Loc) -> SubstResult<QualType> {
        let flags = self.sema.ast.types.qflags(q);
        // A variably modified type is always rebuilt: its bound names
        // the instantiation's entities (clang `SubstType`).
        if !Sema::dependent_flags(flags) && !flags.contains(TypeFlags::VARIABLY_MODIFIED) {
            return Ok(q);
        }
        self.check_stack(|_| loc)?;
        let inner = self.ty_node(q.without_quals(), loc)?;
        if self.sema.is_error_type(inner) {
            return Err(SubstFail);
        }
        Ok(self.sema.ast.types.qualify(inner, q.quals()))
    }

    /// [depr.volatile.type]/2: a volatile-qualified return type that
    /// substitution formed from the dependent `pattern` return type
    /// (clang's `BuildFunctionType` when the type is rebuilt; a
    /// non-dependent one was warned at its definition).
    pub(crate) fn check_volatile_return(&mut self, pattern: QualType, ret: QualType, loc: Loc) {
        if self.sema.std >= kiln_lex::Std::Cxx20
            && self.sema.ast.types.is_dependent(pattern)
            && self.sema.ast.types.effective_quals(ret).contains(kiln_ast::types::Quals::VOLATILE)
        {
            let d = self
                .sema
                .diag(DiagId::sema_deprecated_volatile_return, loc)
                .arg(self.sema.ty_arg(ret));
            self.sema.emit(d);
        }
    }

    /// [depr.volatile.type]/2: each volatile-qualified parameter type
    /// `params` of the function type substitution rebuilds from the
    /// dependent `pattern`, at `loc` (clang's `BuildFunctionType` checks
    /// every parameter; a non-dependent type is not rebuilt).
    pub(crate) fn check_volatile_params(
        &mut self,
        pattern: QualType,
        params: &[QualType],
        loc: Loc,
    ) {
        if self.sema.std < kiln_lex::Std::Cxx20
            || !Sema::dependent_flags(self.sema.ast.types.qflags(pattern))
        {
            return;
        }
        for &t in params {
            if self.sema.ast.types.effective_quals(t).contains(kiln_ast::types::Quals::VOLATILE) {
                let d = self
                    .sema
                    .diag(DiagId::sema_deprecated_volatile_param, loc)
                    .arg(self.sema.ty_arg(t));
                self.sema.emit(d);
            }
        }
    }

    /// One declarator operator applied to `base` with the declarator's
    /// checks.
    /// [temp.deduct.general]/11.10: a substituted function type whose
    /// return type is an array or function type fails (clang
    /// `BuildFunctionType`).
    pub(crate) fn check_return_type(&mut self, ret: QualType, loc: Loc) -> SubstResult<()> {
        if self.sema.is_fp16_arg_disallowed(ret) {
            return self.fp16_arg_error(true, loc);
        }
        let t = &self.sema.ast.types;
        if !t.is_array(ret) && !t.is_function(ret) {
            return Ok(());
        }
        let which = if t.is_array(ret) { "0" } else { "1" };
        let d = self
            .sema
            .diag(DiagId::sema_func_returning_array_function, loc)
            .arg(which)
            .arg(self.sema.ty_arg(ret));
        self.sema.emit(d);
        Err(SubstFail)
    }

    /// clang's `BuildFunctionType`: a substituted `__fp16` parameter
    /// fails where the target passes no half values (with the `*`
    /// fix-it; the return type is checked by
    /// [`Self::check_return_type`]).
    pub(crate) fn check_param_types(&mut self, params: &[QualType], loc: Loc) -> SubstResult<()> {
        if params.iter().any(|&p| self.sema.is_fp16_arg_disallowed(p)) {
            return self.fp16_arg_error(false, loc);
        }
        Ok(())
    }

    /// `TT<args>` for template template parameter `TT`, substituted with a
    /// template `name`: the arguments `TT`'s own default template
    /// arguments supply ([temp.param]/14, [temp.arg.template]/3: the
    /// template-id is checked against `TT`'s parameters, so a default
    /// argument of `TT`'s parameter is used even when the argument
    /// template has none). Each default is substituted with the
    /// enclosing levels and the arguments before it.
    fn ttp_default_args(
        &mut self,
        ttp: DeclId,
        name: kiln_ast::ids::TemplateNameId,
        v: &mut Vec<TemplateArg>,
        loc: Loc,
    ) -> SubstResult<()> {
        // Still a template template parameter (an outer one): its own
        // parameters are checked when it is substituted.
        if self
            .sema
            .ast
            .types
            .template_decl(name)
            .is_none_or(|d| self.sema.kind_of(d) == DeclKind::TemplateTemplateParm)
        {
            return Ok(());
        }
        let Some(list) = self.sema.ast.template_parm(ttp).and_then(|x| x.params) else {
            return Ok(());
        };
        let pl = *self.sema.ast.templates.param_list(list);
        let params: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(pl.params).collect();
        if v.len() >= params.len()
            || params.iter().any(|&p| self.sema.ast.template_parm(p).is_some_and(|x| x.pack))
            || v.iter().any(|&a| self.sema.is_pack_expansion_arg(a))
        {
            return Ok(());
        }
        let depth = usize::from(pl.depth);
        for &p in &params[v.len()..] {
            let Some(def) = self.sema.template_default(p) else { break };
            let mut levels = self.args.clone();
            levels.levels.truncate(depth);
            while levels.depth() < depth {
                levels.push(kiln_ast::ids::ArgListId::EMPTY, ttp, LevelMode::Retain);
            }
            let prior = self.sema.ast.types.arg_list(v);
            levels.push(prior, ttp, LevelMode::Substitute);
            let a = Substituter::new(self.sema, &levels).template_arg(def, loc)?;
            v.push(a);
        }
        Ok(())
    }

    fn fp16_arg_error(&mut self, ret: bool, loc: Loc) -> SubstResult<()> {
        let d = self
            .sema
            .diag(DiagId::sema_parameters_retval_fp16, loc)
            .arg(if ret { "1" } else { "0" })
            .fixit(kiln_diag::FixIt::insert(loc, "*"));
        self.sema.emit(d);
        Err(SubstFail)
    }

    fn chunk(&mut self, base: QualType, chunk: Chunk, loc: Loc) -> SubstResult<QualType> {
        let mut d = Declarator::new(DeclaratorContext::TypeName, loc);
        d.site = crate::declspec::DeclSite::Subst;
        d.name = self.declarator_name;
        d.chunks.push(chunk);
        let t = self.sema.type_for_declarator_at(base, loc, &d);
        if self.sema.is_error_type(t) { Err(SubstFail) } else { Ok(t) }
    }

    /// The type a type declaration found by lookup names.
    pub(crate) fn decl_type(&mut self, d: DeclId) -> QualType {
        match self.sema.kind_of(d) {
            DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::TemplateTypeParm => {
                self.sema.ast.decls.get(d).ty().unwrap_or(QualType::ERROR)
            }
            // A `using typename B<T>::type;` member instantiated as a
            // using-declaration (WP-169): the type its shadow names.
            DeclKind::Using => {
                let node = *self.sema.ast.decls.get(d);
                let (ctx, name) = (node.ctx(), node.name());
                let target = ctx.and_then(|c| {
                    self.sema.ast.lookup_local(c, name).iter().find_map(|s| {
                        let t = self.sema.ast.strip_shadow(s);
                        (s != t && self.sema.kind_of(t) != DeclKind::Using).then_some(t)
                    })
                });
                target.map_or(QualType::ERROR, |t| self.decl_type(t))
            }
            _ => self.sema.tag_type(d),
        }
    }

    #[allow(clippy::too_many_lines, reason = "one arm per type kind, as clang's TreeTransform")]
    fn ty_node(&mut self, q: QualType, loc: Loc) -> SubstResult<QualType> {
        let view = self.sema.ast.types.view(q.ty());
        match view {
            TypeView::TemplateTypeParm(d) => {
                let Some(info) = self.sema.ast.template_parm(d).copied() else {
                    return Err(SubstFail);
                };
                self.type_parm(q, Some(d), info.depth, info.index, loc)
            }
            TypeView::CanonTemplateTypeParm { depth, index, .. } => {
                self.type_parm(q, None, depth as u16, index as u16, loc)
            }
            TypeView::SubstTemplateTypeParm { replacement, associated, index, pack_index } => {
                let r = self.ty(replacement, loc)?;
                Ok(self.sema.ast.types.subst_template_type_parm(r, associated, index, pack_index))
            }
            // A substituted pack of a partially substituted expansion
            // (§7.2): its element for the current index; it stays while
            // another partial expansion is rebuilt.
            TypeView::SubstTemplateTypeParmPack { associated, index, pack } => {
                if self.pack.keep && self.pack_index.is_none() {
                    let n = self.sema.ast.types.args(pack).len();
                    self.sweep_kept(n, |_| crate::templ::pack::PackName::outer(), loc)?;
                    return Ok(q);
                }
                let args = self.sema.ast.types.args(pack);
                let n = u32::try_from(args.len()).unwrap_or(u32::MAX);
                // Unnamed: an outer pack in the length-mismatch diagnostic.
                let i = self.pack_reached(n, |_| crate::templ::pack::PackName::outer(), loc)?;
                let ArgView::Type(t) = self.sema.ast.types.args(pack)[i as usize].view() else {
                    return Err(SubstFail);
                };
                Ok(self.sema.ast.types.subst_template_type_parm(t, associated, index, Some(i)))
            }
            TypeView::Pointer(p) => {
                let p = self.ty(p, loc)?;
                // The written innermost `*` ([`Self::chunk_loc`]) is the
                // first pointer rebuilt.
                let at = self.chunk_loc.take().unwrap_or(loc);
                self.chunk(p, Chunk::Pointer { quals: kiln_ast::types::Quals::NONE, loc: at }, loc)
            }
            TypeView::LValueRef(p) => {
                let at = self.ref_loc.take().unwrap_or(loc);
                let p = self.ty(p, loc)?;
                self.chunk(p, Chunk::LRef { loc: at }, loc)
            }
            TypeView::RValueRef(p) => {
                let at = self.ref_loc.take().unwrap_or(loc);
                let p = self.ty(p, loc)?;
                self.chunk(p, Chunk::RRef { loc: at }, loc)
            }
            TypeView::MemberPointer { pointee, class } => {
                let p = self.ty(pointee, loc)?;
                let c = self.ty(QualType::unqual(class), loc)?;
                let t = &self.sema.ast.types;
                if t.as_record(c).is_none() && !self.sema.is_dependent_type(c) {
                    let d = self
                        .sema
                        .diag(DiagId::sema_mempointer_in_nonclass_type, loc)
                        .arg(self.sema.ty_arg(c));
                    self.sema.emit(d);
                    return Err(SubstFail);
                }
                Ok(self.sema.ast.types.member_pointer(p, c.ty()))
            }
            TypeView::ConstantArray(a) => {
                let e = self.ty(a.elem, loc)?;
                // The written innermost `[` ([`Self::chunk_loc`]).
                let at = self.chunk_loc.take().unwrap_or(loc);
                if let Some(s) = a.size_expr {
                    return self.chunk(e, Chunk::Array { size: Some(s), loc: at }, loc);
                }
                // [temp.deduct]/11.1: the element checks of a bound
                // written without an expression.
                let t = &self.sema.ast.types;
                if t.is_void(e) || t.is_function(e) || t.is_reference(e) {
                    return self.chunk(e, Chunk::Array { size: None, loc: at }, loc);
                }
                // [temp.deduct.general]/11.1: an array of abstract class
                // type (clang `BuildArrayType`).
                if self.sema.require_non_abstract_element(loc, e) {
                    return Err(SubstFail);
                }
                // clang rebuilds the array (`BuildArrayType`): a zero
                // bound is an error in a SFINAE context (see decl.rs).
                if a.size == 0 && self.sema.inst.stack.is_sfinae() {
                    self.sema.error(DiagId::sema_zero_array_size_sfinae, loc);
                    return Err(SubstFail);
                }
                Ok(self.sema.ast.types.constant_array(e, a.size, None))
            }
            TypeView::IncompleteArray(elem) => {
                let e = self.ty(elem, loc)?;
                let at = self.chunk_loc.take().unwrap_or(loc);
                self.chunk(e, Chunk::Array { size: None, loc: at }, loc)
            }
            TypeView::VariableArray { elem, size } => {
                let e = self.ty(elem, loc)?;
                let at = self.chunk_loc.take().unwrap_or(loc);
                let s = self.expr(size)?;
                self.chunk(e, Chunk::Array { size: Some(s), loc: at }, loc)
            }
            TypeView::DependentSizedArray { elem, size } => {
                // A canonical array's bound is profiled: its first
                // expression stands for it (an implicit guide's, WP-134).
                let s = match size {
                    ExprOperand::Expr(s) => s,
                    ExprOperand::Profile(p) => {
                        self.sema.ast.types.profile_expr(p).ok_or(SubstFail)?
                    }
                };
                let e = self.ty(elem, loc)?;
                let at = self.chunk_loc.take().unwrap_or(loc);
                let s = self.expr(s)?;
                self.chunk(e, Chunk::Array { size: Some(s), loc: at }, loc)
            }
            TypeView::Function(p) => {
                let ret = self.ty(p.ret(), loc)?;
                // clang reports at the function type's begin (its return
                // type, the declaration's decl-specifiers), not the name.
                let at = self.type_begin.unwrap_or(loc);
                self.check_return_type(ret, at)?;
                self.check_volatile_return(p.ret(), ret, at);
                let params = self.sema.ast.types.params(&p).to_vec();
                // A parameter of a function type is a 'type name' of its
                // own (clang names it so).
                let outer = self.type_begin.take();
                let name = self.declarator_name.take();
                let chunk = self.chunk_loc.take();
                let out = self.expand_types(&params, loc);
                (self.type_begin, self.declarator_name, self.chunk_loc) = (outer, name, chunk);
                let out = out?;
                // [temp.deduct.general]/11.10: a `void` parameter (clang
                // `BuildFunctionType`).
                if out.iter().any(|&t| self.sema.ast.types.is_void(t)) {
                    self.sema.error(DiagId::sema_param_with_void_type, loc);
                    return Err(SubstFail);
                }
                self.check_param_types(&out, self.type_begin.unwrap_or(loc))?;
                // [dcl.fct]/5: a parameter that became an array or a
                // function type is adjusted to a pointer, as a written
                // one is (clang `BuildFunctionType`:
                // `getAdjustedParameterType`).
                let out: Vec<QualType> =
                    out.into_iter().map(|t| self.sema.adjust_param_type(t)).collect();
                self.check_volatile_params(q, &out, at);
                let exc = match self.sema.ast.types.exc_spec(&p) {
                    ExcSpec::NoexceptExpr {
                        expr,
                        value: kiln_ast::types::NoexceptValue::Dependent(_),
                    } => {
                        let e = self.expr(expr)?;
                        self.sema.act_on_noexcept_spec(e, loc, loc)
                    }
                    // [temp.inst]/14 (C++14 [except.spec]): the
                    // type-id-list of a dynamic exception specification is
                    // substituted.
                    ExcSpec::Dynamic(l) => {
                        let tys = self.sema.ast.types.type_list_types(l).to_vec();
                        let tys = self.expand_types(&tys, loc)?;
                        // `throw(Ts...)` with an empty pack is `throw()`, non-throwing
                        // ([except.spec]/12 as of C++14).
                        if tys.is_empty() {
                            ExcSpec::DynamicNone
                        } else {
                            ExcSpec::Dynamic(self.sema.ast.types.type_list(&tys))
                        }
                    }
                    // The canonical form of a value-dependent
                    // `noexcept(expr)` ([except.spec]/13, [temp.inst]/14):
                    // its operand is the profile's first expression.
                    ExcSpec::DependentNoexcept(pr) => {
                        let e = self.sema.ast.types.profile_expr(pr).ok_or(SubstFail)?;
                        let e = self.expr(e)?;
                        self.sema.act_on_noexcept_spec(e, loc, loc)
                    }
                    other => other,
                };
                Ok(self.sema.ast.types.function(ret, &out, p.info(), exc))
            }
            TypeView::Typedef { decl, underlying } => {
                // A member typedef of the pattern: the instantiated
                // member's typedef (sugar kept); a local one: the
                // instantiated local.
                let ctx_is_record = self
                    .sema
                    .ast
                    .decls
                    .get(decl)
                    .ctx()
                    .is_some_and(|c| self.sema.kind_of(c) == DeclKind::CXXRecord);
                if ctx_is_record
                    && !self.locals.contains_key(&decl)
                    && self.class_not_instantiated(decl, loc)?
                {
                    // A member typedef of a class pattern that is not
                    // being instantiated (the constructor signature of an
                    // implicit deduction guide): the substituted
                    // underlying type, sugar kept (clang's guide
                    // transform clones the typedef).
                    let u = self.ty(underlying, loc)?;
                    return Ok(self.sema.ast.types.typedef_type(decl, u).into());
                }
                if ctx_is_record
                    || self.locals.contains_key(&decl)
                    || self.failed_locals.contains(&decl)
                {
                    let d = self.find_instantiated_decl(decl, loc)?;
                    if d != decl {
                        // Naming it in the instantiation is a use.
                        self.sema.mark_type_referenced(d);
                        return Ok(self.decl_type(d));
                    }
                }
                let u = self.ty(underlying, loc)?;
                Ok(self.sema.ast.types.typedef_type(decl, u).into())
            }
            TypeView::Using { underlying, .. } => self.ty(underlying, loc),
            TypeView::Elaborated { keyword, nns, named } => {
                let nns = match nns {
                    Some(n) => Some(self.nns(n, loc)?),
                    None => None,
                };
                let named = self.ty(named, loc)?;
                Ok(self.sema.ast.types.elaborated(keyword, nns, named))
            }
            TypeView::TemplateSpecialization { name, args, underlying, alias } => {
                let _ = (underlying, alias);
                let ttp = match self.sema.ast.types.template_name_view(name) {
                    TemplateName::TemplateTemplateParm { decl, .. } => Some(decl),
                    _ => None,
                };
                let qual = match ttp {
                    Some(_) if self.sema.clang_compat >= 22 => self.ttp_arg_qualifier(name, loc),
                    _ => None,
                };
                let name = self.template_name(name, loc)?;
                let args = self.args(args, loc)?;
                let mut v = self.sema.ast.types.args(args).to_vec();
                if let Some(ttp) = ttp {
                    self.ttp_default_args(ttp, name, &mut v, loc)?;
                }
                let l = self.written(WrittenKey::Type(q), loc);
                let saved = std::mem::replace(&mut self.sema.subst_template_id, true);
                let r = self.sema.act_template_id(name, &v, l, l);
                self.sema.subst_template_id = saved;
                match r {
                    Ok(TemplateIdRef::Type(t)) => Ok(match qual {
                        Some(nns) => self.sema.ast.types.elaborated(
                            kiln_ast::types::TagKeyword::None,
                            Some(nns),
                            t,
                        ),
                        None => t,
                    }),
                    Ok(_) | Err(()) => Err(SubstFail),
                }
            }
            TypeView::InjectedClassName { pattern, injected } => {
                if let Some(&(_, i)) = self.pairs.iter().rev().find(|p| p.0 == pattern) {
                    self.outer_used.set(true);
                    return Ok(self.sema.tag_type(i));
                }
                self.ty(injected, loc)
            }
            TypeView::UnresolvedUsing(d) => {
                // The instantiated using-declaration's named type.
                let i = self.find_instantiated_decl(d, loc)?;
                if i == d {
                    return Err(SubstFail);
                }
                Ok(self.decl_type(i))
            }
            TypeView::Record(d) | TypeView::Enum(d) => {
                // A member class of a pattern whose substituted class is
                // still dependent: `typename X<A...>::name`.
                if let Some((nns, name)) = self.dependent_member_nns(d, loc)? {
                    let kw = kiln_ast::types::TagKeyword::Typename;
                    return Ok(self.sema.ast.types.dependent_name(kw, nns, name));
                }
                let i = self.find_instantiated_decl(d, loc)?;
                if i != d {
                    // A member class of the enclosing template (its
                    // injected-class-name inside a member class defined
                    // out of line): names that template's parameters
                    // ([temp.friend]/9; clang's
                    // `ConstraintRefersToContainingTemplateChecker`).
                    self.outer_used.set(true);
                }
                Ok(self.sema.tag_type(i))
            }
            TypeView::DependentName { keyword, nns, name } => {
                let n = self.nns(nns, loc)?;
                if self.nns_still_dependent(n) {
                    // Substituted with dependent arguments (an alias
                    // template named in a template): still dependent.
                    return Ok(self.sema.ast.types.dependent_name(keyword, n, name));
                }
                let ctx = self.nns_context(n, self.written(WrittenKey::Nns(nns), loc))?;
                // Diagnosed at the name as written (§5.8).
                let loc = self.written(WrittenKey::Type(q), loc);
                if matches!(
                    keyword,
                    kiln_ast::types::TagKeyword::Struct
                        | kiln_ast::types::TagKeyword::Class
                        | kiln_ast::types::TagKeyword::Union
                        | kiln_ast::types::TagKeyword::Enum
                ) {
                    return self.dependent_tag_name(keyword, n, name, ctx, loc);
                }
                let found = self.sema.lookup_qualified(ctx, name, LookupKind::Ordinary);
                let Some(&d) = found.decls.first() else {
                    let d = match self.enable_if_failure(nns, n, name, ctx) {
                        Some(d) => d,
                        None => self
                            .sema
                            .diag(DiagId::sema_typename_nested_not_found, loc)
                            .arg(crate::decl::sym_str(self.sema, name))
                            .arg(self.ctx_arg(ctx)),
                    };
                    self.sema.emit(d);
                    return Err(SubstFail);
                };
                if self.sema.kind_of(d) == DeclKind::ClassTemplate && self.sema.is_ctad_template(d)
                {
                    // `typename T::X x = e;` with `X` a member class
                    // template: a placeholder for a deduced class type
                    // ([dcl.type.class.deduct]/1, [temp.res.general]/5),
                    // only as a variable's whole declared type.
                    if self.ctad_typename != Some(q.without_quals()) {
                        let at = self.sema.ast.decls.get(d).loc();
                        let note = self.sema.diag(DiagId::note_template_decl_here, at);
                        let d = self
                            .sema
                            .diag(DiagId::sema_typename_deduced_tst, loc)
                            .arg("0")
                            .arg(self.ctx_arg(ctx))
                            .with_note(note);
                        self.sema.emit(d);
                        return Err(SubstFail);
                    }
                    self.sema.mark_type_referenced(d);
                    self.sema.check_found_access(&found, loc);
                    return Ok(self.sema.ctad_placeholder(d, Some(n), (loc, loc)));
                }
                if !matches!(
                    self.sema.kind_of(d),
                    DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::CXXRecord | DeclKind::Enum
                ) {
                    let at = self.sema.ast.decls.get(d).loc();
                    let n = crate::decl::sym_str(self.sema, name);
                    let note = self.sema.diag(DiagId::note_typename_member_refers_here, at).arg(n);
                    let d = self
                        .sema
                        .diag(DiagId::sema_typename_nested_not_type, loc)
                        .arg(crate::decl::sym_str(self.sema, name))
                        .arg(self.ctx_arg(ctx))
                        .with_note(note);
                    self.sema.emit(d);
                    return Err(SubstFail);
                }
                self.sema.mark_type_referenced(d);
                // [class.access]/4, [temp.deduct.general]/8: checked now,
                // a substitution failure in a SFINAE context.
                self.sema.check_found_access(&found, loc);
                let t = self.decl_type(d);
                // The keyword is kept as written (clang: `typename S::type`).
                Ok(self.sema.ast.types.elaborated(keyword, Some(n), t))
            }
            // spec/sema-templates.md §9.2.
            TypeView::UnaryTransform { kind, base, .. } => {
                let b = self.ty(base, loc)?;
                self.sema.build_unary_transform(kind, b, loc).ok_or(SubstFail)
            }
            TypeView::Decltype { expr, .. } => {
                self.sema.enter_decltype();
                let e = self.expr(expr);
                self.sema.exit_decltype(e.ok());
                let e = e?;
                self.sema.act_on_decltype(e, loc, loc).ok_or(SubstFail)
            }
            // `typename [: r :]` with a dependent operand
            // (spec/reflection.md §5): the substituted operand's type.
            TypeView::Splice { operand, designated: None } => {
                let e = match operand {
                    ExprOperand::Expr(e) => e,
                    ExprOperand::Profile(p) => {
                        self.sema.ast.types.profile_expr(p).ok_or(SubstFail)?
                    }
                };
                let e = self.expr(e)?;
                self.sema.act_on_splice_type(e, loc, loc).ok_or(SubstFail)
            }
            // GNU `typeof` (spec/sema.md §5.8).
            TypeView::TypeOf { operand: TypeOfOperand::Type(t), unqual, .. } => {
                let t = self.ty(t, loc)?;
                Ok(self.sema.ast.types.typeof_type(t, unqual))
            }
            TypeView::TypeOf { operand: TypeOfOperand::Expr(e), unqual, .. } => {
                self.sema.enter_unevaluated();
                let e = self.expr(e);
                self.sema.exit_unevaluated();
                self.sema.act_on_typeof_expr(e?, loc, unqual).ok_or(SubstFail)
            }
            TypeView::Auto { keyword, deduced, concept, args, pack } => match deduced {
                // Deduced from a type-dependent initializer
                // (`Sema::dependent_auto`): deduced again from the
                // substituted one.
                Some(d) if d == QualType::DEPENDENT => {
                    Ok(self.sema.ast.types.auto_type(keyword, None, concept, args, pack))
                }
                Some(d) => {
                    let d = self.ty(d, loc)?;
                    Ok(self.sema.ast.types.auto_type(keyword, Some(d), concept, args, pack))
                }
                // An undeduced placeholder whose type-constraint names
                // outer parameters (`template<int N, C<N> auto V>`): the
                // constraint's arguments are substituted, so the type is
                // no longer dependent (clang `TransformAutoType`).
                None if !pack && self.sema.ast.types.is_dependent(q) => match args {
                    Some(a) => {
                        let a = self.args(a, loc)?;
                        Ok(self.sema.ast.types.auto_type(keyword, None, concept, Some(a), pack))
                    }
                    None => Ok(q),
                },
                None => Ok(q),
            },
            // A class template placeholder deduced from type-dependent
            // arguments: deduced again from the substituted ones
            // ([over.match.class.deduct], WP-071b).
            // A template template parameter's placeholder names its
            // argument's template ([dcl.type.class.deduct]).
            TypeView::DeducedTemplateSpecialization { name, deduced } => match deduced {
                Some(d) if d == QualType::DEPENDENT => {
                    let name = self.template_name(name, loc)?;
                    Ok(self.sema.ast.types.deduced_template_specialization(name, None))
                }
                Some(d) => {
                    let d = self.ty(d, loc)?;
                    Ok(self.sema.ast.types.deduced_template_specialization(name, Some(d)))
                }
                None if self.sema.ast.types.is_dependent(q) => {
                    let name = self.template_name(name, loc)?;
                    Ok(self.sema.ast.types.deduced_template_specialization(name, None))
                }
                None => Ok(q),
            },
            TypeView::Paren(inner) => {
                let i = self.ty(inner, loc)?;
                Ok(self.sema.ast.types.paren_type(i))
            }
            TypeView::Attributed { kind, modified, equivalent } => {
                let at = self.sema.nullability_loc(q, loc);
                let m = self.ty(modified, loc)?;
                let e = self.ty(equivalent, loc)?;
                // A nullability qualifier on a type that became a
                // non-pointer (clang `TransformAttributedType`).
                if crate::nullability::is_nullability(kind) && !self.sema.can_have_nullability(m) {
                    let d = self
                        .sema
                        .diag(DiagId::sema_nullability_nonpointer, at)
                        .arg(crate::nullability::spelling(kind))
                        .arg(self.sema.ty_arg(m));
                    self.sema.emit(d);
                    return Err(SubstFail);
                }
                Ok(self.sema.ast.types.attributed(kind, m, e))
            }
            TypeView::Complex(e) => {
                let e = self.ty(e, loc)?;
                Ok(self.sema.ast.types.complex(e))
            }
            TypeView::Atomic(v) => {
                let at = self.written(WrittenKey::Type(q), loc);
                let v = self.ty(v, loc)?;
                self.sema.act_on_atomic_type(v, at).ok_or(SubstFail)
            }
            // [temp.inst] for clang's vector types (spec/types.md §1).
            TypeView::Vector { elem, count, kind } => {
                let e = self.ty(elem, loc)?;
                Ok(self.sema.ast.types.vector(e, count, kind))
            }
            TypeView::DependentVector { elem, size: ExprOperand::Expr(s), kind } => {
                let e = self.ty(elem, loc)?;
                // Diagnosed at the attribute, as clang does.
                let at = match self.sema.inst.vector_attr_locs.get(&s) {
                    Some(&l) => l,
                    None => self.sema.begin_loc(s),
                };
                let s = self.expr(s)?;
                let v = self.sema.build_vector_type(e, s, kind, at);
                if !self.sema.is_error_type(v) {
                    Ok(v)
                } else if self.sema.inst.stack.is_sfinae() {
                    Err(SubstFail)
                } else {
                    // clang leaves the type unchanged after the error.
                    Ok(e)
                }
            }
            TypeView::DependentTemplateSpecialization { keyword, nns, name, args } => {
                let n = self.nns(nns, loc)?;
                let args = self.args(args, loc)?;
                if self.nns_still_dependent(n) {
                    return Ok(self
                        .sema
                        .ast
                        .types
                        .dependent_template_specialization(keyword, n, name, args));
                }
                // `T::template X<args>` ([temp.names]/5): the member
                // template of the substituted class, diagnosed at `X`.
                // A non-class qualifier is diagnosed at the `T` (§5.8).
                self.nns_context(n, self.written(WrittenKey::Nns(nns), loc))?;
                let loc = self.written(WrittenKey::Type(q), loc);
                let d = self.member_template_named(n, name, loc)?;
                let underlying = self.sema.template_name_for(d);
                let tn = self.sema.ast.types.template_name(TemplateName::Qualified {
                    nns: n,
                    template_kw: true,
                    underlying,
                });
                let v = self.sema.ast.types.args(args).to_vec();
                let Ok(TemplateIdRef::Type(t)) = self.sema.act_template_id(tn, &v, loc, loc) else {
                    return Err(SubstFail);
                };
                let named = match self.sema.ast.types.view(t.ty()) {
                    TypeView::Elaborated { nns: None, named, .. } => named,
                    _ => t,
                };
                Ok(self.sema.ast.types.elaborated(keyword, Some(n), named))
            }
            TypeView::PackIndexing { pattern, index, selected: None } => {
                let index = match index {
                    ExprOperand::Expr(e) => e,
                    ExprOperand::Profile(p) => {
                        self.sema.ast.types.profile_expr(p).ok_or(SubstFail)?
                    }
                };
                self.pack_indexing_type(pattern, index, loc)
            }
            TypeView::PackIndexing { selected: Some(s), .. } => self.ty(s, loc),
            TypeView::PackExpansion { .. } => self.unimplemented("pack expansion", loc),
            // `const A` with `using A = T[3]`: [dcl.array]/6 pushes the
            // cv-qualifiers into the substituted element type (clang
            // `TransformQualifiedType` re-applying the qualifiers).
            TypeView::QualifiedArray { quals, base } => {
                let b = self.ty(base.into(), loc)?;
                Ok(self.sema.ast.types.qualify(b, quals))
            }
            // clang `TransformDependentBitIntType`: the width is
            // substituted and the type rebuilt (`BuildBitIntType`; its
            // errors at the type's start, clang: the `_BitInt` keyword).
            TypeView::DependentBitInt { signed, width } => {
                let w = self.sema.ast.types.profile_expr(width).ok_or(SubstFail)?;
                let w = self.expr(w)?;
                let at = self.type_begin.unwrap_or(loc);
                let t = self.sema.build_bit_int_type(!signed, w, at);
                if self.sema.is_error_type(t) {
                    return Err(SubstFail);
                }
                Ok(t)
            }
            // Remaining kinds cannot be dependent today.
            _ => self.unimplemented("substitution into this type", loc),
        }
    }

    /// Template type parameter `(depth, index)` (§5.3): its argument as
    /// `SubstTemplateTypeParmType` sugar, or itself when retained.
    fn type_parm(
        &mut self,
        q: QualType,
        decl: Option<DeclId>,
        depth: u16,
        index: u16,
        loc: Loc,
    ) -> SubstResult<QualType> {
        match self.lookup(depth, index) {
            ArgLookup::Arg(a) => {
                if self.pack.keep
                    && self.pack_index.is_none()
                    && let ArgView::Pack(l) = a.view()
                {
                    // Rebuilding a partially substituted expansion (§7.2).
                    let n = self.sema.ast.types.args(l).len();
                    self.sweep_kept(n, |s| s.parm_pack_name(decl), loc)?;
                    let assoc = self.assoc(depth).ok_or(SubstFail)?;
                    let p = self.sema.ast.types.subst_template_type_parm_pack(
                        assoc,
                        u32::from(index),
                        l,
                    );
                    return Ok(QualType::new(p.ty(), q.quals_bits() | p.quals_bits()));
                }
                let a = self.element(a, decl, loc)?;
                let ArgView::Type(t) = a.view() else { return Err(SubstFail) };
                let Some(assoc) = self.assoc(depth).filter(|_| !self.final_subst) else {
                    return Ok(t);
                };
                // A pack element that is itself an expansion (`Ts` = <`Us...`>,
                // an alias template guide's) stays visibly one (WP-134).
                if self.pack_index.is_some()
                    && matches!(self.sema.ast.types.view(t.ty()), TypeView::PackExpansion { .. })
                {
                    return Ok(t);
                }
                Ok(self.sema.ast.types.subst_template_type_parm(
                    t,
                    assoc,
                    u32::from(index),
                    self.pack_index,
                ))
            }
            ArgLookup::Retain(d) if d == depth => Ok(q),
            // A parameter of a member template redeclared one level
            // shallower ([temp.mem]; §4.6): its new declaration.
            ArgLookup::Retain(d) => {
                if let Some(p) = decl
                    && let Ok(np) = self.find_instantiated_decl(p, loc)
                    && np != p
                    && let Some(t) = self.sema.ast.decls.get(np).ty()
                {
                    return Ok(QualType::new(t.ty(), q.quals_bits() | t.quals_bits()));
                }
                let pack =
                    matches!(
                        self.sema.ast.types.view(q.ty()),
                        TypeView::CanonTemplateTypeParm { pack: true, .. }
                    ) || decl.and_then(|p| self.sema.ast.template_parm(p)).is_some_and(|i| i.pack);
                let t = self.sema.ast.types.canon_template_type_parm(
                    u32::from(d),
                    u32::from(index),
                    pack,
                );
                Ok(QualType::new(t, q.quals_bits()))
            }
            ArgLookup::Missing => Err(SubstFail),
        }
    }

    /// The member template `name` of the context `n` names after
    /// substitution (`T::template X`, [temp.names]/5); diagnosed when
    /// there is none or it is not a template.
    pub(crate) fn member_template_named(
        &mut self,
        n: kiln_ast::ids::NnsId,
        name: kiln_source::symbol::Symbol,
        loc: Loc,
    ) -> SubstResult<DeclId> {
        let ctx = self.nns_context(n, loc)?;
        let found = self.sema.lookup_qualified(ctx, name, LookupKind::Ordinary);
        let Some(&d) = found.decls.first() else {
            let d = self
                .sema
                .diag(DiagId::sema_no_member, loc)
                .arg(crate::decl::sym_str(self.sema, name))
                .arg(self.ctx_arg(ctx));
            self.sema.emit(d);
            return Err(SubstFail);
        };
        // An overload set that has a function template besides
        // non-template overloads names a template ([temp.names]/3): the
        // template-id is resolved over the whole set (reflect-cpp's
        // `Literal::contains(const std::string&)` beside
        // `template <StringLiteral> contains()`).
        let t = found.decls.iter().find_map(|&x| self.sema.template_of(x));
        if let Some(t) = t {
            // [class.access]/4: a member class or alias template is
            // checked now (a SFINAE failure in a deduction); a function
            // template by overload resolution.
            if matches!(self.sema.kind_of(t), DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate)
            {
                self.sema.check_found_access(&found, loc);
            }
            return Ok(t);
        }
        let at = self.sema.ast.decls.get(d).loc();
        let note = self.sema.diag(DiagId::note_declared_non_template, at);
        let d = self
            .sema
            .diag(DiagId::sema_template_kw_refers_to_non_template, loc)
            .arg(crate::decl::sym_str(self.sema, name))
            .with_note(note);
        self.sema.emit(d);
        Err(SubstFail)
    }

    /// A context for diagnostics: `'X<int>'` or `namespace 'n'`.
    /// clang `isEnableIf` + `CheckTypenameType`: `typename
    /// enable_if<C, T>::type` names no type. `written` is the qualifier
    /// as written, `n` as substituted. With a condition that is not a
    /// literal (after substitution) the conjunct that failed is named at
    /// its location; otherwise the `enable_if` is, at its first argument
    /// as written (spec/sema-templates.md §5.7.1).
    fn enable_if_failure(
        &mut self,
        written: kiln_ast::ids::NnsId,
        n: kiln_ast::ids::NnsId,
        name: kiln_source::symbol::Symbol,
        ctx: DeclId,
    ) -> Option<kiln_diag::Diag> {
        if crate::decl::sym_str(self.sema, name) != "type" {
            return None;
        }
        let first_arg = |s: &Self, nns| {
            let kiln_ast::types::Nns::Type { ty, .. } = s.sema.ast.types.nns_view(nns) else {
                return None;
            };
            let TypeView::TemplateSpecialization { name, args, .. } =
                s.sema.ast.types.view(ty.ty())
            else {
                return None;
            };
            Some((name, s.sema.ast.types.args(args).first().copied()?))
        };
        let (tname, arg) = first_arg(self, n)?;
        let tmpl = self.sema.ast.types.template_decl(tname)?;
        if self.sema.kind_of(tmpl) != DeclKind::ClassTemplate
            || self.sema.decl_name(tmpl) != "enable_if"
        {
            return None;
        }
        let cond = match arg.view() {
            ArgView::Expr { expr, .. } => {
                let k = self.sema.ast.exprs.get(self.sema.ignore_paren_casts(expr)).kind();
                (k != kiln_ast::expr::ExprKind::CXXBoolLiteralExpr).then_some(expr)
            }
            _ => None,
        };
        if let Some(c) = cond {
            let failed = self.sema.failed_condition(c);
            let at = self.sema.begin_loc(failed);
            let text = self.sema.requirement_text(failed);
            return Some(
                self.sema
                    .diag(DiagId::sema_typename_nested_not_found_requirement, at)
                    .arg(text)
                    .range(self.sema.expr_range(failed)),
            );
        }
        let at = match first_arg(self, written).map(|(_, a)| a.view()) {
            Some(ArgView::Expr { expr, .. }) => self.sema.begin_loc(expr),
            _ => self.sema.ast.decls.get(ctx).loc(),
        };
        Some(
            self.sema
                .diag(DiagId::sema_typename_nested_not_found_enable_if, at)
                .arg(self.ctx_arg(ctx)),
        )
    }

    /// `class-key T::name` with `T` substituted to `ctx`
    /// ([dcl.type.elab]/1-3, clang `RebuildDependentNameType` with a tag
    /// keyword): a tag of `ctx` whose kind agrees with the class-key.
    fn dependent_tag_name(
        &mut self,
        keyword: kiln_ast::types::TagKeyword,
        nns: kiln_ast::NnsId,
        name: kiln_source::symbol::Symbol,
        ctx: DeclId,
        loc: Loc,
    ) -> SubstResult<QualType> {
        use kiln_ast::types::TagKeyword as K;
        let found = self.sema.lookup_qualified(ctx, name, LookupKind::Tag);
        let Some(&d) = found.decls.first() else {
            let kind = match keyword {
                K::Union => 2u8,
                K::Class => 3,
                K::Enum => 4,
                _ => 0,
            };
            let d = self
                .sema
                .diag(DiagId::sema_not_tag_in_scope, loc)
                .arg(kind)
                .arg(crate::decl::sym_str(self.sema, name))
                .arg(self.ctx_arg(ctx));
            self.sema.emit(d);
            return Err(SubstFail);
        };
        if !self.sema.tag_matches(d, keyword) {
            // At the class-key, as clang (the name is `loc`).
            let kw = self.sema.tag_keyword_loc(loc);
            self.sema.diag_wrong_tag(crate::decl::sym_str(self.sema, name), kw, d);
            return Err(SubstFail);
        }
        self.sema.mark_type_referenced(d);
        self.sema.check_found_access(&found, loc);
        let t = self.decl_type(d);
        Ok(self.sema.ast.types.elaborated(keyword, Some(nns), t))
    }

    pub(crate) fn ctx_arg(&self, ctx: DeclId) -> String {
        match self.sema.kind_of(ctx) {
            // Fully qualified, unsugared (clang: `'S::In'`, `'S::M<int>'`).
            DeclKind::CXXRecord | DeclKind::Enum => {
                format!("'{}'", self.sema.ty_canonical_str(self.sema.tag_type(ctx)))
            }
            _ => format!("'{}'", self.sema.decl_name(ctx)),
        }
    }
}
