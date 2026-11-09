//! Member declarations (spec/sema-class.md §3; WP-066b): member access,
//! static data members ([class.static.data]), static member function
//! qualifiers ([class.static.mfct]/2) and base-specifiers
//! ([class.derived.general]).

use kiln_ast::decl::{
    Access, BaseSpec, DeclFlags, DeclKind, DefinitionBits, TagKind, VarSpec, VarStorage,
};
use kiln_ast::names::DeclName;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{Quals, RefQual, TypeView};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;
use kiln_source::fx::FxHashSet;
use kiln_source::symbol::Symbol;

use super::BaseSpecInfo;
use crate::Sema;
use crate::decl::sym_str;
use crate::declspec::{DeclSpec, Declarator, StorageSpec};
use crate::lookup::LookupKind;

impl Sema<'_, '_> {
    /// Gives member `d` its access ([class.access.spec]/1): the current
    /// access of the class being defined when declared inside it, else
    /// (an out-of-line definition) the access of its first declaration.
    pub(crate) fn set_member_access(&mut self, d: DeclId) {
        let Some(ctx) = self.ast.decls.get(d).ctx() else { return };
        if self.kind_of(ctx) != DeclKind::CXXRecord {
            return;
        }
        let lexical = self.ast.decls.lexical_ctx(d).unwrap_or(ctx);
        let access = match self.classes.last() {
            Some(c) if c.record == lexical => c.access,
            _ => match self.ast.prev(d) {
                Some(p) => self.ast.decls.get(p).misc().access(),
                None => return,
            },
        };
        self.ast.decls.set_access(d, access);
    }

    /// The class `r` is local ([class.local]): nested (through classes)
    /// in a function.
    pub(crate) fn is_local_class(&self, r: DeclId) -> bool {
        let mut c = self.ast.decls.get(r).ctx();
        let mut depth = 0;
        while let Some(x) = c {
            match self.kind_of(x) {
                DeclKind::CXXRecord if depth < 1024 => {
                    c = self.ast.decls.get(x).ctx();
                    depth += 1;
                }
                DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion => return true,
                _ => return false,
            }
        }
        false
    }

    /// The innermost unnamed class enclosing (or being) `r`, through
    /// classes only (clang's anonymous-struct check for static members).
    fn unnamed_class_of(&self, r: DeclId) -> Option<DeclId> {
        let mut c = Some(r);
        let mut depth = 0;
        while let Some(x) = c {
            if self.kind_of(x) != DeclKind::CXXRecord || depth > 1024 {
                return None;
            }
            if self.ast.decls.get(x).name() == DeclName::EMPTY {
                return Some(x);
            }
            c = self.ast.decls.get(x).ctx();
            depth += 1;
        }
        None
    }

    fn tag_keyword(&self, r: DeclId) -> &'static str {
        match self.ast.decls.record(r).map(|x| x.tag) {
            Some(TagKind::Class) => "class",
            Some(TagKind::Union) => "union",
            _ => "struct",
        }
    }

    /// [class.static.mfct]/2: a static member function has no
    /// cv-qualifier or ref-qualifier. clang reports them in one
    /// diagnostic and drops them from the type.
    pub(crate) fn check_static_method_quals(
        &mut self,
        ty: QualType,
        d: &Declarator,
        begin: Loc,
    ) -> QualType {
        let Some(p) = self.ast.types.proto(ty).copied() else { return ty };
        let info = p.info();
        let (q, r) = (info.this_quals(), info.ref_qual());
        if q.is_empty() && r == RefQual::None {
            self.diag_nullability_only_fn_quals(DiagId::sema_static_member_fn_quals, d, begin);
            return ty;
        }
        let s = method_quals_spelling(q, r);
        // At the first qualifier written, else the parameter list's `)`.
        let loc = crate::decl::first_fn_qual_loc(d)
            .or_else(|| {
                d.chunks.iter().find_map(|c| match c {
                    crate::declspec::Chunk::Function { rparen, .. } => Some(*rparen),
                    _ => None,
                })
            })
            .unwrap_or(d.loc);
        let msg = self.diag(DiagId::sema_static_member_fn_quals, loc).arg(s);
        self.emit(msg);
        let params = self.ast.types.params(&p).to_vec();
        let exc = self.ast.types.exc_spec(&p);
        let info = info.with_this_quals(Quals::NONE).with_ref_qual(RefQual::None);
        self.ast.types.function(p.ret(), &params, info, exc)
    }

    /// A static data member: declared in its class ([class.static.data]/2),
    /// or defined out of line with the class as qualifier (/3).
    #[allow(
        clippy::too_many_lines,
        reason = "one check per paragraph of [class.static.data], in clang's order"
    )]
    pub(crate) fn act_on_static_member(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        name: Symbol,
        mut spec: VarSpec,
    ) -> DeclId {
        let (ctx, lexical) = self.decl_contexts(d);
        let ctx = self.ast.definition(ctx).unwrap_or(ctx);
        let loc = d.name_loc();
        let mut prev_ok = None;
        let mut invalid = false;
        if d.qualifier.is_none() {
            if self.templ.local_template {
                invalid = true;
            } else if self.is_local_class(ctx) {
                let msg = self
                    .diag(DiagId::sema_static_member_local_class, loc)
                    .arg(sym_str(self, name))
                    .arg(self.tag_keyword(ctx))
                    .arg(self.decl_name(ctx));
                self.emit(msg);
                invalid = true;
            } else if let Some(anon) = self.unnamed_class_of(ctx) {
                let msg = self
                    .diag(DiagId::sema_static_member_anon_class, loc)
                    .arg(sym_str(self, name))
                    .arg(self.tag_keyword(anon));
                self.emit(msg);
                invalid = true;
            }
            // [dcl.constexpr]/1: a constexpr static data member is
            // implicitly inline.
            if spec.is_constexpr() && !self.inst.deferring_init {
                spec = spec.with_inline(true);
            }
            // A class or enumeration of the same name is hidden
            // ([basic.scope.hiding]/2).
            if let Some(&p) = self.lookup_in(ctx, name, LookupKind::Ordinary).decls.first()
                && !crate::lookup::is_tag_kind(self.kind_of(p))
            {
                let id = if self.kind_of(p) == DeclKind::Var
                    || self.kind_of(p) == DeclKind::Field
                    || self.kind_of(p) == DeclKind::Typedef
                {
                    DiagId::sema_duplicate_member
                } else {
                    DiagId::sema_redefinition_different_kind
                };
                let msg = self.diag(id, loc).arg(sym_str(self, name));
                let msg = self.note_prev(msg, p, id == DiagId::sema_redefinition_different_kind);
                self.emit(msg);
                invalid = true;
            }
        } else {
            // [class.static.data]/2-3: the out-of-class definition takes
            // no storage class (clang `err_static_out_of_line`,
            // `err_storage_class_for_static_member`); `mutable` was
            // diagnosed by `var_spec` and makes it invalid.
            match ds.storage {
                Some((StorageSpec::Static, sloc)) => {
                    self.error(DiagId::sema_static_out_of_class, sloc);
                }
                Some((StorageSpec::Extern | StorageSpec::Register, sloc)) => {
                    self.error(DiagId::sema_storage_class_for_static_member, sloc);
                }
                Some((StorageSpec::Mutable, _)) => invalid = true,
                _ => {}
            }
            spec = spec.with_storage(VarStorage::None);
            match self.find_prev(name, d.qualifier) {
                None => {
                    let msg = self
                        .diag(DiagId::sema_no_member, loc)
                        .arg(sym_str(self, name))
                        .arg(self.qualifier_desc(ctx));
                    self.emit(msg);
                    invalid = true;
                }
                Some(p) if self.kind_of(p) == DeclKind::Field => {
                    self.error(DiagId::sema_nonstatic_member_out_of_line, loc);
                    invalid = true;
                }
                // A declaration with no type (`S::m;`, after "a type
                // specifier is required") is invalid and merged with
                // nothing (clang's `MergeVarDecl`; WP-411).
                Some(_) if ds.type_error || !ds.has_type_specifier() => invalid = true,
                Some(p) if self.kind_of(p) == DeclKind::Var => {
                    let p = self.ast.latest(p);
                    let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                    let compatible = self.redecl_var_types_compatible(pty, ty);
                    let prev_def = self.ast.redecls_of(p).find(|&x| self.is_var_definition(x));
                    // [depr.static.constexpr]: an out-of-class declaration
                    // without an initializer of an inline (constexpr)
                    // static data member is a redundant redeclaration.
                    let redundant = d.init_style.is_none()
                        && prev_def.is_some_and(|x| self.is_in_class_static(x));
                    // [temp.expl.spec]/14: `template<> int C<char>::n;`
                    // after the definition only redeclares it.
                    let spec_decl = d.init_style.is_none()
                        && self.has_explicit_spec_head()
                        && self.ast.member_spec_info(self.ast.first(p)).is_some();
                    let prev_def = prev_def.filter(|_| !redundant);
                    if compatible {
                        // [dcl.stc]/4: one thread storage kind.
                        let n = sym_str(self, name);
                        self.check_tls_redecl(p, spec.tls(), prev_def.is_some(), loc, &n);
                    }
                    if !compatible {
                        let msg = self
                            .diag(DiagId::sema_redefinition_different_type, loc)
                            .arg(sym_str(self, name))
                            .arg(self.ty_arg(ty))
                            .arg(self.ty_arg(pty));
                        let msg = self.note_prev(msg, p, prev_def.is_some());
                        self.emit(msg);
                        invalid = true;
                    } else if let Some(def) = prev_def.filter(|_| !spec_decl) {
                        let msg =
                            self.diag(DiagId::sema_redefinition, loc).arg(sym_str(self, name));
                        let msg = self.note_prev(msg, def, true);
                        self.emit(msg);
                        invalid = true;
                    } else if self.has_explicit_spec_head()
                        && !self.act_on_explicit_member_var_spec(p, loc)
                    {
                        invalid = true;
                    } else {
                        prev_ok = Some(p);
                        let pspec = self.ast.decls.var(p).map(|x| x.spec).unwrap_or_default();
                        if pspec.is_inline() {
                            spec = spec.with_inline(true);
                        }
                        if pspec.is_constexpr() {
                            spec = spec.with_constexpr(true);
                        }
                    }
                }
                Some(p) => {
                    let msg = self
                        .diag(DiagId::sema_redefinition_different_kind, loc)
                        .arg(sym_str(self, name));
                    let msg = self.note_prev(msg, p, true);
                    self.emit(msg);
                    invalid = true;
                }
            }
        }
        let ty = prev_ok.map_or(ty, |p| self.redecl_var_type(p, ty));
        let mut h = DeclHead::new(ctx, DeclName::ident(name), loc);
        h.lexical = Some(lexical);
        self.mark_pattern_next();
        let v = self.builder().var(h, ty, spec, prev_ok);
        // The decl-specifiers' start, for the instantiation's written
        // locations (§5.8): `static int (*s)(typename T::type);`.
        if ds.loc.is_valid() && ds.loc != loc && self.is_instantiation_dependent_type(ty) {
            let _ = self.ast.decls.cold.begin_loc.insert(v, ds.loc);
            self.note_type_spec_loc(v, ds, loc);
        }
        self.note_ref_loc(v, d, ty);
        self.lambda_mangle.var = Some(v);
        if invalid {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
        self.check_sizeless_var(v, false);
        self.record_constinit(v, ds);
        self.check_constexpr_var_literal(v);
        if d.qualifier.is_some() {
            // [class.static.data]/3: the initializer is in the scope of
            // the class (and its access context, [class.access]/7).
            self.initializing = Some(v);
        }
        v
    }

    /// Whether static data member `v` is declared inside its class (not
    /// an out-of-line definition).
    pub(crate) fn is_in_class_static(&self, v: DeclId) -> bool {
        let Some(ctx) = self.ast.decls.get(v).ctx() else { return false };
        self.kind_of(ctx) == DeclKind::CXXRecord
            && self
                .ast
                .decls
                .lexical_ctx(v)
                .is_some_and(|l| self.ast.first(l) == self.ast.first(ctx))
    }

    /// Whether `t` is a literal type ([basic.types.general]/10), as far as
    /// kiln tracks it: scalars, references, arrays of literal types and
    /// classes with the `LITERAL` bit.
    pub(crate) fn is_literal_type(&self, t: QualType) -> bool {
        let types = &self.ast.types;
        match types.canonical_view(t) {
            TypeView::Record(r) => {
                self.ast.definition(r).and_then(|d| self.ast.decls.record(d)).is_some_and(|x| {
                    x.defn.contains(DefinitionBits::COMPLETE)
                        && x.defn.contains(DefinitionBits::LITERAL)
                })
            }
            TypeView::ConstantArray(_) | TypeView::IncompleteArray(_) => {
                types.element(t).is_some_and(|e| self.is_literal_type(e))
            }
            TypeView::VariableArray { .. } | TypeView::Function(_) | TypeView::Invalid => false,
            _ => true,
        }
    }

    /// `msg` (reporting that `ty` is not a literal type) with clang
    /// `RequireLiteralType`'s explanation of why the class element type
    /// of `ty` is not literal ([basic.types.general]/10): its virtual
    /// bases, the lack of a constexpr constructor, the first non-literal
    /// (or volatile) base or member, or a non-constexpr destructor.
    pub(crate) fn with_non_literal_notes(&self, mut msg: Diag, ty: QualType) -> Diag {
        let elem = self.base_element(ty);
        let Some(def) = self.ast.types.as_record(elem).and_then(|r| self.ast.definition(r)) else {
            return msg;
        };
        let Some(rec) = self.ast.decls.record(def) else { return msg };
        let bits = rec.defn;
        // clang streams the `CXXRecordDecl` (`getNameForDiagnostic`), not its type.
        let rd = format!("'{}'", self.name_for_diagnostic(def));
        let def_loc = self.ast.decls.get(def).loc();
        // Not a reason in C++26 (P3533R2).
        if self.vbases_forbid_constexpr(bits) {
            let vbases = self.virtual_bases(def);
            let tag = 2 * u8::from(rec.tag == TagKind::Class);
            msg = msg.with_note(
                self.diag(DiagId::note_non_literal_virtual_base, def_loc)
                    .arg(tag)
                    .arg(u8::from(vbases.len() != 1)),
            );
            for (_, b) in vbases {
                msg = msg.with_note(self.diag(DiagId::note_constexpr_virtual_base_here, b.loc));
            }
            return msg;
        }
        if !bits.contains(DefinitionBits::AGGREGATE)
            && !bits.contains(DefinitionBits::HAS_CONSTEXPR_CTOR)
            && !bits.contains(DefinitionBits::TRIVIAL_DEFAULT_CTOR)
            && !bits.contains(DefinitionBits::LAMBDA)
        {
            return msg.with_note(
                self.diag(DiagId::note_non_literal_no_constexpr_ctors, def_loc).arg(rd),
            );
        }
        for b in self.ast.decls.bases(def, &self.ast.lists) {
            if !self.is_literal_type(b.ty) {
                return msg.with_note(
                    self.diag(DiagId::note_non_literal_base_class, b.loc)
                        .arg(&rd)
                        .arg(self.ty_arg(b.ty)),
                );
            }
        }
        let fields: Vec<DeclId> = self
            .ast
            .members(def)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        for f in fields {
            let n = self.ast.decls.get(f);
            let Some(q) = n.ty() else { continue };
            let volatile = self.ast.types.effective_quals(q).is_volatile();
            if !self.is_literal_type(q) || volatile {
                return msg.with_note(
                    self.diag(DiagId::note_non_literal_field, n.loc())
                        .arg(&rd)
                        .arg(self.decl_arg(f))
                        .arg(self.ty_arg(q))
                        .arg(u8::from(volatile)),
                );
            }
        }
        let dtor = self
            .ast
            .members(def)
            .find(|&m| self.ast.decls.get(m).kind() == DeclKind::CXXDestructor);
        if let Some(dtor) = dtor
            && !self
                .ast
                .decls
                .function(dtor)
                .is_some_and(|f| f.spec.constexpr_kind() != kiln_ast::decl::ConstexprKind::None)
        {
            // Before C++20 the rule is a trivial destructor and clang
            // names a user-provided one (`RequireLiteralType`). WP-1123.
            let id = if self.std >= kiln_lex::Std::Cxx20 {
                Some(DiagId::note_non_literal_non_constexpr_dtor)
            } else if self.is_user_provided(dtor) {
                Some(DiagId::note_non_literal_user_provided_dtor)
            } else {
                None
            };
            if let Some(id) = id {
                msg = msg.with_note(self.diag(id, self.ast.decls.get(dtor).loc()).arg(rd));
            }
        }
        msg
    }

    /// clang's in-class initializer checks for a non-inline static data
    /// member ([class.static.data]/4): a const integral or enumeration
    /// member with a constant initializer, else `constexpr` is needed.
    pub(crate) fn check_in_class_static_init(&mut self, v: DeclId, e: kiln_ast::ExprId) {
        let vn = *self.ast.decls.get(v);
        let ty = vn.ty().unwrap_or(QualType::ERROR);
        if self.is_error_type(ty) || self.has_errors_in(e) {
            return;
        }
        if self.is_dependent_type(ty) || self.expr_is_value_dependent(e) {
            // Checked when instantiated ([temp.res.general]/6).
            return;
        }
        let quals = self.ast.types.effective_quals(ty);
        let int_like = self.ast.types.is_integral(ty) || self.ast.types.as_enum(ty).is_some();
        if !quals.is_const() || self.ast.types.is_reference(ty) {
            self.error(DiagId::sema_static_member_nonconst_init, vn.loc());
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        } else if int_like {
            if quals.is_volatile() {
                self.error(DiagId::sema_static_member_volatile_init, vn.loc());
                self.ast.decls.set_flags(v, DeclFlags::INVALID);
                return;
            }
            let loc = self.begin_loc(e);
            let ctx = kiln_oven::EvalContext::new(kiln_oven::EvalKind::Other, loc);
            // Any constant value of the member's type will do: not
            // `eval_int`, whose `i128` cannot hold every `unsigned
            // __int128` (`~(unsigned __int128)0`, WP-275).
            if let Err(f) = self.eval_const(e, ctx) {
                if self.clang_compat >= 22 {
                    // clang 22 reports at the member, with the
                    // evaluation's notes.
                    let d = self.diag(DiagId::sema_static_member_init_not_ice, vn.loc());
                    let d = Self::with_failure_notes(d, f);
                    self.emit(d);
                } else {
                    // clang 19 reports at the initializer's `getExprLoc`.
                    let at = self.culprit_loc(e);
                    self.error(DiagId::sema_static_member_init_not_ice, at);
                }
                self.ast.decls.set_flags(v, DeclFlags::INVALID);
            }
        } else if self.is_literal_type(ty) {
            if self.ast.types.is_floating(ty) {
                let msg = self
                    .diag(DiagId::sema_static_float_init, self.begin_loc(e))
                    .arg(self.ty_arg(ty));
                // clang points the note at the declaration's start; the
                // decl-specifiers' location is gone by now (spec gap).
                let msg = msg.with_note(self.diag(DiagId::note_add_constexpr, vn.loc()));
                self.emit(msg);
            } else {
                let msg = self
                    .diag(DiagId::sema_static_member_literal_init, vn.loc())
                    .arg(self.ty_arg(ty));
                self.emit(msg);
                self.ast.decls.set_flags(v, DeclFlags::INVALID);
            }
        } else {
            let msg = self.diag(DiagId::sema_static_member_bad_init, vn.loc()).arg(self.ty_arg(ty));
            self.emit(msg);
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
    }

    /// The base-clause of class `r` (clang's `CheckBaseSpecifier` and
    /// `AttachBaseSpecifiers`). False when a specifier was dropped (an
    /// instantiated class is then invalid, clang `SubstBaseSpecifiers`).
    pub(crate) fn attach_bases(&mut self, r: DeclId, specs: &[BaseSpecInfo]) -> bool {
        self.attach_bases_annotated(r, specs, &[])
    }

    /// [`Sema::attach_bases`]; `inst[i]` are annotations of `specs[i]`
    /// already made (an instantiated base-specifier's, substituted from
    /// its pattern's), after those its own attributes give
    /// ([dcl.attr.annotation]/1; spec/annotations.md §3).
    pub(crate) fn attach_bases_annotated(
        &mut self,
        r: DeclId,
        specs: &[BaseSpecInfo],
        inst: &[Vec<kiln_ast::AnnId>],
    ) -> bool {
        if specs.is_empty() {
            return true;
        }
        let tag = self.ast.decls.record(r).map_or(TagKind::Struct, |x| x.tag);
        if tag == TagKind::Union {
            self.error(DiagId::sema_union_has_base, self.ast.decls.get(r).loc());
            return false;
        }
        let mut dropped = false;
        let mut out: Vec<BaseSpec> = Vec::new();
        // (index in `out`, index in `specs`) of the kept specifiers.
        let mut kept: Vec<(u32, usize)> = Vec::new();
        let mut seen: Vec<DeclId> = Vec::new();
        for (si, s) in specs.iter().enumerate() {
            if self.is_error_type(s.ty) {
                dropped = true;
                continue;
            }
            // A dependent base ([temp.dep.type]/1) is checked at
            // instantiation (WP-070a); lookup does not look into it
            // ([temp.dep.general]/4).
            if self.is_dependent_type(s.ty) {
                kept.push((out.len() as u32, si));
                out.push(BaseSpec {
                    ty: s.ty,
                    loc: s.loc,
                    is_virtual: s.is_virtual,
                    access: s.access,
                    pack_expansion: s.pack_expansion,
                });
                continue;
            }
            if s.pack_expansion {
                self.unsupported("pack expansions in a base-specifier list", s.loc);
                dropped = true;
                continue;
            }
            let Some(b) = self.ast.types.as_record(s.ty) else {
                self.error(DiagId::sema_base_not_class, s.loc);
                self.invalidate_defining(r);
                dropped = true;
                continue;
            };
            let btag = self.ast.decls.record(b).map_or(TagKind::Struct, |x| x.tag);
            if btag == TagKind::Union {
                self.error(DiagId::sema_base_is_union, s.loc);
                self.invalidate_defining(r);
                dropped = true;
                continue;
            }
            // clang `CheckBaseSpecifier`'s `RequireCompleteType`: a base
            // whose instantiation fails here is dropped, quietly; lookup
            // in the derived class does not find its members.
            if self.instantiation_fails(s.ty, s.loc) {
                self.invalidate_defining(r);
                dropped = true;
                continue;
            }
            if !self.require_complete(s.ty, s.loc) {
                let msg = self.diag(DiagId::sema_base_incomplete, s.loc);
                // A class being defined gets clang's "not complete until
                // the closing '}'" note from `incomplete_note`.
                let msg = self.incomplete_note(msg, s.ty);
                self.emit(msg);
                self.invalidate_defining(r);
                dropped = true;
                continue;
            }
            let def = self.ast.definition(b).unwrap_or(b);
            // clang `CheckBaseSpecifier`: "if the base class is invalid
            // the derived class is as well" (spec/sema-class.md §2.3).
            if self.ast.decls.get(def).has(DeclFlags::INVALID) {
                self.invalidate_defining(r);
            }
            if self.ast.decls.record(def).is_some_and(|x| x.defn.contains(DefinitionBits::FINAL)) {
                let n = format!("'{}'", self.decl_name(def));
                let msg = self.diag(DiagId::sema_base_final, s.loc).arg(n.clone());
                let msg = msg.with_note(
                    self.diag(DiagId::note_entity_declared_here, self.ast.decls.get(def).loc())
                        .arg(n),
                );
                self.emit(msg);
                self.invalidate_defining(r);
                dropped = true;
                continue;
            }
            if seen.iter().any(|&x| self.ast.first(x) == self.ast.first(def)) {
                let unq = self.ast.types.canonical(s.ty).without_quals();
                let msg = self.diag(DiagId::sema_base_duplicate, s.loc).arg(self.ty_arg(unq));
                self.emit(msg);
                dropped = true;
                continue;
            }
            seen.push(def);
            kept.push((out.len() as u32, si));
            out.push(BaseSpec {
                ty: s.ty,
                loc: s.loc,
                is_virtual: s.is_virtual,
                access: s.access,
                pack_expansion: false,
            });
        }
        if !out.is_empty() {
            let ast = &mut self.ast;
            let _ = ast.decls.set_bases(r, &out, &mut ast.lists);
            self.check_inaccessible_bases(r, &out);
        }
        self.annotate_bases(r, specs, inst, &kept);
        !dropped
    }

    /// The annotations of the kept base-specifiers of `r` (`kept`: index
    /// in the base list, index in `specs`), recorded under `r`'s first
    /// declaration ([dcl.attr.annotation]/1).
    fn annotate_bases(
        &mut self,
        r: DeclId,
        specs: &[BaseSpecInfo],
        inst: &[Vec<kiln_ast::AnnId>],
        kept: &[(u32, usize)],
    ) {
        let first = self.ast.first(r);
        for &(i, si) in kept {
            let mut anns: Vec<kiln_ast::AnnId> = Vec::new();
            for &a in &specs[si].attrs {
                let n = *self.ast.attrs.get(a);
                if n.kind == kiln_ast::attr::AttrKind::ANNOTATION
                    && let Some(e) = <kiln_ast::ExprId as kiln_ast::ids::RawId>::from_raw(n.a)
                    && let Some(id) = self.annotate(r, e, n.loc)
                {
                    anns.push(id);
                }
            }
            anns.extend(inst.get(si).into_iter().flatten().copied());
            if !anns.is_empty() {
                let l = self.ast.lists.push(&anns);
                self.ast.annots.attach_base(first, i, l);
            }
        }
    }

    /// The access a base-specifier of `r` grants ([class.access.base]/2):
    /// as written, else `private` for a `class` and `public` otherwise.
    pub(crate) fn base_access(&self, r: DeclId, written: Access) -> Access {
        match written {
            Access::None => {
                if self.ast.decls.record(r).is_some_and(|x| x.tag == TagKind::Class) {
                    Access::Private
                } else {
                    Access::Public
                }
            }
            a => a,
        }
    }

    /// [temp.dep.type]/7, [dcl.fct.default]/5: a parameter of (reference
    /// to) a class type that is dependent, such as a nested class of a
    /// class template, has its default argument checked only when the
    /// argument is instantiated, where the class is complete. Its
    /// constructors and default member initializers are not looked at
    /// in the template definition (clang keeps the argument as written).
    pub(crate) fn is_dependent_class_param(&self, pty: QualType) -> bool {
        let t = if self.ast.types.is_reference(pty) {
            self.ast.types.pointee(pty).unwrap_or(pty)
        } else {
            pty
        };
        self.ast.types.is_dependent(t) && self.ast.types.as_record(t).is_some()
    }

    /// The parameter's default argument ([dcl.fct.default]): copy-
    /// initialized to the parameter type, its own full-expression.
    pub(crate) fn attach_default_arg(&mut self, p: DeclId, e: kiln_ast::ExprId) {
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        let c = self.defer_default_arg_uses(|s| {
            let c = if s.is_dependent_class_param(pty) {
                // Converted at instantiation, as for a variable of that
                // type (`dependent_init_form`).
                e
            } else if s.is_error_type(pty) || s.ast.types.is_dependent(pty) {
                let loc = s.ast.exprs.get(e).loc();
                s.perform_init(pty, crate::init::Initializer::Expr(e), loc, false)
            } else {
                let note = s.param_decl_note(p);
                let ploc = s.ast.decls.get(p).loc();
                s.convert_default_arg(e, pty, ploc, note)
            };
            s.note_default_init_full_expr();
            s.act_on_full_expr(c)
        });
        if !self.check_default_arg_locals(c) {
            self.ast.decls.set_flags(p, DeclFlags::INVALID);
        }
        let _ = self.ast.decls.set_default_arg(p, c);
    }

    /// On entering the definition of function `f`: the scopes of the
    /// contexts between its lexical and its semantic context
    /// ([basic.lookup.unqual]/1: names after the declarator-id of an
    /// out-of-line member or qualified definition are looked up in the
    /// scopes of its class and namespaces), outermost first, and the
    /// implicit `inline` of a function defined in its class when it is
    /// attached to the global module ([class.mfct]/1, [class.friend]/7;
    /// one attached to a named module is not inline, spec/modules.md §7).
    /// An instantiation is inline exactly when its pattern's definition
    /// is (clang `InstantiateFunctionDefinition` copies `isInlined`): an
    /// instantiated member is declared in its class, but one defined out
    /// of class from a non-inline pattern is not inline
    /// ([temp.explicit]/12 depends on it). Returns how many scopes were
    /// pushed.
    pub(crate) fn enter_definition_scopes(&mut self, f: DeclId) -> usize {
        let Some(ctx) = self.ast.decls.get(f).ctx() else { return 0 };
        let lexical = self.ast.decls.lexical_ctx(f).unwrap_or(ctx);
        if let Some(inline) = self.instantiated_pattern_inline(f) {
            if inline && !self.ast.decls.function(f).is_some_and(|x| x.spec.is_inline()) {
                self.ast
                    .decls
                    .mark_function(f, kiln_ast::decl::FnSpec::new().with_implicit_inline(true));
            }
        } else if self.kind_of(lexical) == DeclKind::CXXRecord
            && matches!(
                self.kind_of(f),
                DeclKind::CXXMethod
                    | DeclKind::Function
                    | DeclKind::CXXConstructor
                    | DeclKind::CXXDestructor
                    | DeclKind::CXXConversion
            )
            && self.ast.module_attachment(lexical).is_none()
        {
            self.ast
                .decls
                .mark_function(f, kiln_ast::decl::FnSpec::new().with_implicit_inline(true));
        }
        self.push_scopes_between(ctx, lexical)
    }

    /// For a function instantiated from a template pattern: whether the
    /// pattern's definition is inline (clang `FunctionDecl::isInlined`:
    /// `inline` on a redeclaration of the pattern, or defined in its
    /// class). `None` when `f` is not an instantiation (also when it is
    /// explicitly specialized: [temp.expl.spec]/12, inline only as
    /// declared) or its pattern has no definition.
    pub(crate) fn instantiated_pattern_inline(&self, f: DeclId) -> Option<bool> {
        let first = self.ast.first(f);
        let explicit_spec = |k: SpecKind| k == SpecKind::ExplicitSpecialization;
        if self.ast.member_spec_info(first).is_some_and(|(_, x)| explicit_spec(x.kind))
            || self.ast.spec_info(first).is_some_and(|(_, x)| explicit_spec(x.kind))
        {
            return None;
        }
        let pattern = self.ast.instantiated_from(first)?;
        let pdef = self.ast.definition(pattern)?;
        let spec = |d: DeclId| self.ast.decls.function(d).map(|x| x.spec);
        let plex = self.ast.decls.lexical_ctx(pdef).or_else(|| self.ast.decls.get(pdef).ctx());
        let in_class = plex.is_some_and(|l| {
            self.kind_of(l) == DeclKind::CXXRecord && self.ast.module_attachment(l).is_none()
        });
        Some(
            in_class
                || spec(pdef).is_some_and(kiln_ast::decl::FnSpec::implicit_inline)
                || self
                    .ast
                    .redecls_of(pattern)
                    .any(|r| spec(r).is_some_and(kiln_ast::decl::FnSpec::is_inline)),
        )
    }

    /// Pushes the scopes of `ctx` and its enclosing contexts up to (not
    /// including) the first one that encloses `lexical`, outermost
    /// first. Returns how many were pushed.
    pub(crate) fn push_scopes_between(&mut self, ctx: DeclId, lexical: DeclId) -> usize {
        let mut chain = Vec::new();
        let mut c = Some(ctx);
        while let Some(x) = c {
            if chain.len() > 1024 || self.encloses(x, lexical) {
                break;
            }
            chain.push(x);
            c = self.ast.decls.get(x).ctx();
        }
        let n = chain.len();
        for x in chain.into_iter().rev() {
            let kind = if self.ast.decls.record(x).is_some() {
                crate::scope::ScopeKind::Record
            } else {
                crate::scope::ScopeKind::Namespace
            };
            let x = if kind == crate::scope::ScopeKind::Record {
                self.ast.definition(x).unwrap_or(x)
            } else {
                x
            };
            self.scopes.push(crate::scope::Scope::new(kind, Some(x)));
        }
        n
    }

    /// The type of `this` in the body of `f` ([expr.prim.this]/2): `cv C*`
    /// for a non-static member function of `C` with cv-qualifiers `cv`.
    pub(crate) fn method_this_type(&mut self, f: DeclId) -> Option<QualType> {
        if !matches!(
            self.kind_of(f),
            DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        ) {
            return None;
        }
        if self.ast.decls.function(f).is_some_and(|x| {
            x.spec.storage() == kiln_ast::decl::FnStorage::Static || x.spec.has_explicit_object()
        }) {
            return None;
        }
        let class = self.ast.decls.get(f).ctx()?;
        let fty = self.ast.decls.get(f).ty()?;
        let q = self.ast.types.proto(fty).map_or(Quals::NONE, |p| p.info().this_quals());
        let t = self.this_class_type(class);
        Some(self.this_pointer_type(t, q))
    }

    /// The type of `this` in a member function of class type `t` with
    /// method qualifiers `q` ([expr.prim.this]/2): pointer to cv `t`; a
    /// GNU `__restrict` method qualifier makes the pointer itself
    /// restrict-qualified (clang `getThisType`'s `X * __restrict`).
    pub(crate) fn this_pointer_type(&mut self, t: QualType, q: Quals) -> QualType {
        let cv = q - Quals::RESTRICT;
        let t = if cv.is_empty() { t } else { self.ast.types.qualify(t, cv) };
        let p = self.ast.types.pointer_to(t);
        if q.is_restrict() { self.ast.types.qualify(p, Quals::RESTRICT) } else { p }
    }

    /// The definition bits of class `r` at its `}` that depend on its
    /// bases, member access and default member initializers ([class.prop],
    /// [dcl.init.aggr]/1, [temp.param]/7), on top of `bits` (computed from
    /// the fields alone). clang's `CXXRecordDecl::setBases` / `addedMember`.
    #[allow(
        clippy::too_many_lines,
        reason = "one pass over bases and members, as clang's addedMember"
    )]
    pub(crate) fn class_bits(
        &self,
        r: DeclId,
        build: Option<&super::ClassBuild>,
        mut bits: DefinitionBits,
    ) -> DefinitionBits {
        const INHERITED: [DefinitionBits; 9] = [
            DefinitionBits::TRIVIALLY_COPYABLE,
            DefinitionBits::TRIVIAL_DEFAULT_CTOR,
            DefinitionBits::TRIVIAL_COPY_CTOR,
            DefinitionBits::TRIVIAL_MOVE_CTOR,
            DefinitionBits::TRIVIAL_COPY_ASSIGN,
            DefinitionBits::TRIVIAL_MOVE_ASSIGN,
            DefinitionBits::TRIVIAL_DTOR,
            DefinitionBits::LITERAL,
            DefinitionBits::STRUCTURAL,
        ];
        let and_with = |bits: &mut DefinitionBits, other: DefinitionBits| {
            for b in INHERITED {
                if !other.contains(b) {
                    *bits = bits.without(b);
                }
            }
            if other.contains(DefinitionBits::HAS_MUTABLE_FIELDS) {
                *bits = bits.with(DefinitionBits::HAS_MUTABLE_FIELDS);
            }
        };
        let not_sl =
            |bits: &mut DefinitionBits| *bits = bits.without(DefinitionBits::STANDARD_LAYOUT);
        // Bases ([class.prop]/3, [dcl.init.aggr]/1.4).
        let bases: Vec<BaseSpec> = self.ast.decls.bases(r, &self.ast.lists).copied().collect();
        let mut bases_with_fields = false;
        // Before C++17 (P0017R1) an aggregate has no base classes at all
        // (C++11/14 [dcl.init.aggr]/1; clang `setBases`; WP-1090).
        if self.std < kiln_lex::Std::Cxx17 && !bases.is_empty() {
            bits = bits.without(DefinitionBits::AGGREGATE);
        }
        for b in &bases {
            let Some(bd) = self.ast.types.as_record(b.ty).and_then(|d| self.ast.definition(d))
            else {
                continue;
            };
            let bb = self.ast.decls.record(bd).map(|x| x.defn).unwrap_or_default();
            and_with(&mut bits, bb);
            if b.is_virtual || self.base_access(r, b.access) != Access::Public {
                bits = bits.without(DefinitionBits::AGGREGATE);
            }
            if self.base_access(r, b.access) != Access::Public {
                bits = bits.without(DefinitionBits::STRUCTURAL);
            }
            if !bb.contains(DefinitionBits::EMPTY) {
                bits = bits.without(DefinitionBits::EMPTY);
            }
            if !bb.contains(DefinitionBits::STANDARD_LAYOUT) {
                not_sl(&mut bits);
            }
            if self.class_has_fields(bd) {
                if bases_with_fields {
                    not_sl(&mut bits);
                }
                bases_with_fields = true;
            }
        }
        // At most one base class subobject of any given type.
        let mut seen: Vec<DeclId> = Vec::new();
        if self.repeated_base(r, &mut seen, 0) {
            not_sl(&mut bits);
        }
        let empty_bases: Vec<DeclId> = seen
            .iter()
            .copied()
            .filter(|&b| {
                self.ast.decls.record(b).is_some_and(|x| x.defn.contains(DefinitionBits::EMPTY))
            })
            .collect();
        // Fields.
        let union = self.ast.decls.record(r).is_some_and(|x| x.tag == TagKind::Union);
        let mut accesses = [false; 3];
        let mut first = true;
        let mut any_literal_field = false;
        let fields: Vec<DeclId> = self.ast.decls.members(r).collect();
        for f in fields {
            if self.kind_of(f) != DeclKind::Field {
                continue;
            }
            let fd = self.ast.decls.field(f).copied();
            let unnamed_bf = fd.is_some_and(|x| x.bit_width.is_some())
                && self.ast.decls.get(f).name().is_empty();
            if unnamed_bf {
                continue;
            }
            match self.ast.decls.get(f).misc().access() {
                Access::Private => {
                    accesses[2] = true;
                    bits = bits.with(DefinitionBits::HAS_PRIVATE_FIELDS);
                }
                Access::Protected => {
                    accesses[1] = true;
                    bits = bits.with(DefinitionBits::HAS_PROTECTED_FIELDS);
                }
                _ => accesses[0] = true,
            }
            if fd.is_some_and(|x| x.flags & kiln_ast::decl::FieldData::MUTABLE != 0) {
                bits = bits.without(DefinitionBits::STRUCTURAL);
            }
            if bases_with_fields {
                not_sl(&mut bits);
            }
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            if self.ast.types.is_reference(fty) {
                not_sl(&mut bits);
            }
            let mut et = fty;
            for _ in 0..64 {
                match self.ast.types.element(et) {
                    Some(e) if self.ast.types.is_array(et) => et = e,
                    _ => break,
                }
            }
            // [basic.types.general]/10.5: the members of a literal class
            // are of non-volatile literal types; a union needs only one.
            let literal =
                self.is_literal_type(fty) && !self.ast.types.effective_quals(fty).is_volatile();
            if union {
                any_literal_field |= literal;
            } else if !literal {
                bits = bits.without(DefinitionBits::LITERAL).without(DefinitionBits::STRUCTURAL);
            }
            if let Some(fr) = self.ast.types.as_record(et).and_then(|d| self.ast.definition(d)) {
                let mut fb = self.ast.decls.record(fr).map(|x| x.defn).unwrap_or_default();
                if union {
                    fb = fb.with(DefinitionBits::LITERAL);
                    // P3074R7 [class.default.ctor]/3.4, [class.dtor]/8.3:
                    // a union's variant members do not make its default
                    // constructor or destructor non-trivial.
                    if self.trivial_unions() {
                        fb = fb
                            .with(DefinitionBits::TRIVIAL_DEFAULT_CTOR)
                            .with(DefinitionBits::TRIVIAL_DTOR);
                    }
                }
                and_with(&mut bits, fb);
                if !fb.contains(DefinitionBits::STANDARD_LAYOUT) {
                    not_sl(&mut bits);
                }
                if (union || first)
                    && !empty_bases.is_empty()
                    && self.zero_offset_hits(fr, &empty_bases)
                {
                    not_sl(&mut bits);
                }
            }
            first = false;
        }
        if union && !first && !any_literal_field {
            bits = bits.without(DefinitionBits::LITERAL).without(DefinitionBits::STRUCTURAL);
        }
        if accesses.iter().filter(|&&a| a).count() > 1 {
            not_sl(&mut bits);
        }
        if accesses[1] || accesses[2] {
            bits = bits.without(DefinitionBits::AGGREGATE).without(DefinitionBits::STRUCTURAL);
        }
        if let Some(b) = build {
            bits = bits.with(b.bits);
            if b.bits.contains(DefinitionBits::HAS_IN_CLASS_INIT) {
                bits = bits.without(DefinitionBits::TRIVIAL_DEFAULT_CTOR);
            }
        }
        bits
    }

    /// Whether class `c` or one of its bases declares a non-static data
    /// member (clang's `HasBasesWithFields` and `field_empty`).
    /// Each class is looked at once (a virtual lattice is not walked
    /// per path; WP-400).
    fn class_has_fields(&self, c: DeclId) -> bool {
        let mut work = vec![c];
        let mut walked: FxHashSet<DeclId> = FxHashSet::default();
        while let Some(c) = work.pop() {
            if !walked.insert(c) || walked.len() > 4096 {
                continue;
            }
            if self.ast.decls.members(c).any(|m| self.kind_of(m) == DeclKind::Field) {
                return true;
            }
            work.extend(self.ast.decls.bases(c, &self.ast.lists).filter_map(|b| {
                self.ast.types.as_record(b.ty).and_then(|d| self.ast.definition(d))
            }));
        }
        false
    }

    /// Collects every base class subobject type of `c` into `seen`;
    /// true if one occurs twice.
    fn repeated_base(&self, c: DeclId, seen: &mut Vec<DeclId>, depth: usize) -> bool {
        if depth > 256 || seen.len() > 4096 {
            return false;
        }
        let bases: Vec<QualType> = self.ast.decls.bases(c, &self.ast.lists).map(|b| b.ty).collect();
        let mut rep = false;
        for t in bases {
            let Some(d) = self.ast.types.as_record(t).and_then(|d| self.ast.definition(d)) else {
                continue;
            };
            // A class seen before had its bases collected then: they all
            // repeat, so its subtree is not walked again (WP-400).
            if seen.contains(&d) {
                rep = true;
            } else {
                seen.push(d);
                rep |= self.repeated_base(d, seen, depth + 1);
            }
        }
        rep
    }

    /// Whether a subobject at offset zero of an object of class `x` has
    /// one of the types `empty_bases` (clang's
    /// `hasSubobjectAtOffsetZeroOfEmptyBaseType`, the set M(X) of
    /// [class.prop]/3).
    fn zero_offset_hits(&self, x: DeclId, empty_bases: &[DeclId]) -> bool {
        let mut work = vec![x];
        let mut seen = vec![x];
        while let Some(x) = work.pop() {
            if empty_bases.contains(&x) {
                return true;
            }
            if seen.len() > 4096 {
                return false;
            }
            let union = self.ast.decls.record(x).is_some_and(|r| r.tag == TagKind::Union);
            for f in self.ast.decls.members(x) {
                if self.kind_of(f) != DeclKind::Field {
                    continue;
                }
                let fd = self.ast.decls.field(f);
                if fd.is_some_and(|d| d.bit_width.is_some())
                    && self.ast.decls.get(f).name().is_empty()
                {
                    continue;
                }
                let mut t = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                for _ in 0..64 {
                    match self.ast.types.element(t) {
                        Some(e) if self.ast.types.is_array(t) => t = e,
                        _ => break,
                    }
                }
                if let Some(rd) = self.ast.types.as_record(t).and_then(|d| self.ast.definition(d))
                    && !seen.contains(&rd)
                {
                    seen.push(rd);
                    work.push(rd);
                }
                if !union {
                    break;
                }
            }
        }
        false
    }
}

/// A function type's cv-qualifiers and ref-qualifier as clang spells them
/// in diagnostics (`getFunctionQualifiersAsString`): `const volatile &&`.
pub(crate) fn method_quals_spelling(q: Quals, r: RefQual) -> String {
    let mut s = String::new();
    if q.contains(Quals::CONST) {
        s.push_str("const");
    }
    if q.contains(Quals::VOLATILE) {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str("volatile");
    }
    if q.contains(Quals::RESTRICT) {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str("__restrict");
    }
    let rs = match r {
        RefQual::None => "",
        RefQual::LValue => "&",
        RefQual::RValue => "&&",
    };
    if !rs.is_empty() {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(rs);
    }
    s
}
