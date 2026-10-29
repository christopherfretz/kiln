//! Member declarations (spec/sema-class.md §3; WP-066b): member access,
//! static data members ([class.static.data]), static member function
//! qualifiers ([class.static.mfct]/2) and base-specifiers
//! ([class.derived.general]).

use kiln_ast::decl::{Access, BaseSpec, DeclKind, DefinitionBits, TagKind, VarSpec, VarStorage};
use kiln_ast::names::DeclName;
use kiln_ast::types::{Quals, RefQual, TypeView};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::DiagId;
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
                DeclKind::Function | DeclKind::CXXMethod => return true,
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
    pub(crate) fn check_static_method_quals(&mut self, ty: QualType, d: &Declarator) -> QualType {
        let Some(p) = self.ast.types.proto(ty).copied() else { return ty };
        let info = p.info();
        let (q, r) = (info.this_quals(), info.ref_qual());
        if q.is_empty() && r == RefQual::None {
            return ty;
        }
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
            s.push_str("restrict");
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
        // The qualifiers follow the parameter list's `)` (their own
        // locations are not in the declarator).
        let loc = d
            .chunks
            .iter()
            .find_map(|c| match c {
                crate::declspec::Chunk::Function { rparen, .. } => Some(*rparen),
                _ => None,
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
            if self.is_local_class(ctx) {
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
            if spec.is_constexpr() {
                spec = spec.with_inline(true);
            }
            if let Some(&p) = self.lookup_in(ctx, name, LookupKind::Ordinary).decls.first() {
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
            if let Some((StorageSpec::Static, sloc)) = ds.storage {
                self.error(DiagId::sema_static_out_of_class, sloc);
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
                Some(p) if self.kind_of(p) == DeclKind::Var => {
                    let p = self.ast.latest(p);
                    let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                    let t = &self.ast.types;
                    let compatible = t.same_type(pty, ty)
                        || (t.is_array(pty)
                            && t.is_array(ty)
                            && t.array_size(pty).is_none()
                            && t.element(pty)
                                .zip(t.element(ty))
                                .is_some_and(|(a, b)| t.same_type(a, b)));
                    let prev_def =
                        self.ast.redecls_of(p).find(|&x| self.is_var_definition(x));
                    if !compatible {
                        let msg = self
                            .diag(DiagId::sema_redefinition_different_type, loc)
                            .arg(sym_str(self, name))
                            .arg(self.ty_arg(ty))
                            .arg(self.ty_arg(pty));
                        let msg = self.note_prev(msg, p, prev_def.is_some());
                        self.emit(msg);
                        invalid = true;
                    } else if let Some(def) = prev_def {
                        let msg =
                            self.diag(DiagId::sema_redefinition, loc).arg(sym_str(self, name));
                        let msg = self.note_prev(msg, def, true);
                        self.emit(msg);
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
        let mut h = DeclHead::new(ctx, DeclName::ident(name), loc);
        h.lexical = Some(lexical);
        let v = self.builder().var(h, ty, spec, prev_ok);
        if invalid {
            self.ast.decls.set_flags(v, kiln_ast::decl::DeclFlags::INVALID);
        }
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
            && self.ast.decls.lexical_ctx(v).is_some_and(|l| self.ast.first(l) == self.ast.first(ctx))
    }

    /// Whether `t` is a literal type ([basic.types.general]/10), as far as
    /// kiln tracks it: scalars, references, arrays of literal types and
    /// classes with the `LITERAL` bit.
    pub(crate) fn is_literal_type(&self, t: QualType) -> bool {
        let types = &self.ast.types;
        match types.canonical_view(t) {
            TypeView::Record(r) => self
                .ast
                .definition(r)
                .and_then(|d| self.ast.decls.record(d))
                .is_some_and(|x| {
                    x.defn.contains(DefinitionBits::COMPLETE)
                        && x.defn.contains(DefinitionBits::LITERAL)
                }),
            TypeView::ConstantArray(_) | TypeView::IncompleteArray(_) => {
                types.element(t).is_some_and(|e| self.is_literal_type(e))
            }
            TypeView::VariableArray { .. } | TypeView::Function(_) | TypeView::Invalid => false,
            _ => true,
        }
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
        let quals = self.ast.types.effective_quals(ty);
        let int_like =
            self.ast.types.is_integral(ty) || self.ast.types.as_enum(ty).is_some();
        if !quals.is_const() || self.ast.types.is_reference(ty) {
            self.error(DiagId::sema_static_member_nonconst_init, vn.loc());
            self.ast.decls.set_flags(v, kiln_ast::decl::DeclFlags::INVALID);
        } else if int_like {
            if quals.is_volatile() {
                self.error(DiagId::sema_static_member_volatile_init, vn.loc());
                self.ast.decls.set_flags(v, kiln_ast::decl::DeclFlags::INVALID);
                return;
            }
            let loc = self.begin_loc(e);
            let ctx = kiln_oven::EvalContext::new(kiln_oven::EvalKind::Other, loc);
            if self.eval_int(e, ctx).is_err() {
                self.error(DiagId::sema_static_member_init_not_ice, loc);
                self.ast.decls.set_flags(v, kiln_ast::decl::DeclFlags::INVALID);
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
                self.ast.decls.set_flags(v, kiln_ast::decl::DeclFlags::INVALID);
            }
        } else {
            let msg =
                self.diag(DiagId::sema_static_member_bad_init, vn.loc()).arg(self.ty_arg(ty));
            self.emit(msg);
            self.ast.decls.set_flags(v, kiln_ast::decl::DeclFlags::INVALID);
        }
    }

    /// The base-clause of class `r` (clang's `CheckBaseSpecifier` and
    /// `AttachBaseSpecifiers`).
    pub(crate) fn attach_bases(&mut self, r: DeclId, specs: &[BaseSpecInfo]) {
        if specs.is_empty() {
            return;
        }
        let tag = self.ast.decls.record(r).map_or(TagKind::Struct, |x| x.tag);
        if tag == TagKind::Union {
            self.error(DiagId::sema_union_has_base, self.ast.decls.get(r).loc());
            return;
        }
        let mut out: Vec<BaseSpec> = Vec::new();
        let mut seen: Vec<DeclId> = Vec::new();
        for s in specs {
            if self.is_error_type(s.ty) {
                continue;
            }
            if s.pack_expansion {
                self.unsupported("pack expansions in a base-specifier list", s.loc);
                continue;
            }
            let Some(b) = self.ast.types.as_record(s.ty) else {
                self.error(DiagId::sema_base_not_class, s.loc);
                continue;
            };
            let btag = self.ast.decls.record(b).map_or(TagKind::Struct, |x| x.tag);
            if btag == TagKind::Union {
                self.error(DiagId::sema_base_is_union, s.loc);
                continue;
            }
            if !self.is_complete(s.ty) {
                let msg = self.diag(DiagId::sema_base_incomplete, s.loc);
                let msg = self.incomplete_note(msg, s.ty);
                let msg = match self.ast.definition(b) {
                    Some(def) if self.defining.contains(&def) => msg.with_note(
                        self.diag(DiagId::note_definition_not_complete, self.ast.decls.get(def).loc())
                            .arg(format!("'{}'", self.qualified_decl_name(def))),
                    ),
                    _ => msg,
                };
                self.emit(msg);
                continue;
            }
            let def = self.ast.definition(b).unwrap_or(b);
            if self
                .ast
                .decls
                .record(def)
                .is_some_and(|x| x.defn.contains(DefinitionBits::FINAL))
            {
                let n = format!("'{}'", self.decl_name(def));
                let msg = self.diag(DiagId::sema_base_final, s.loc).arg(n.clone());
                let msg = msg.with_note(
                    self.diag(DiagId::note_entity_declared_here, self.ast.decls.get(def).loc())
                        .arg(n),
                );
                self.emit(msg);
                continue;
            }
            if seen.iter().any(|&x| self.ast.first(x) == self.ast.first(def)) {
                let unq = self.ast.types.canonical(s.ty).without_quals();
                let msg = self.diag(DiagId::sema_base_duplicate, s.loc).arg(self.ty_arg(unq));
                self.emit(msg);
                continue;
            }
            if s.is_virtual {
                self.unsupported("virtual base classes", s.loc);
                continue;
            }
            seen.push(def);
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

    /// The parameter's default argument ([dcl.fct.default]): copy-
    /// initialized to the parameter type, its own full-expression.
    pub(crate) fn attach_default_arg(&mut self, p: DeclId, e: kiln_ast::ExprId) {
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        let loc = self.ast.exprs.get(e).loc();
        let c = self.perform_init(pty, crate::init::Initializer::Expr(e), loc, false);
        let c = self.act_on_full_expr(c);
        let _ = self.ast.decls.set_default_arg(p, c);
    }

    /// On entering the definition of function `f`: the scopes of the
    /// contexts between its lexical and its semantic context
    /// ([basic.lookup.unqual]/1: names after the declarator-id of an
    /// out-of-line member or qualified definition are looked up in the
    /// scopes of its class and namespaces), outermost first, and the
    /// implicit `inline` of a function defined in its class
    /// ([class.mfct]/1). Returns how many scopes were pushed.
    pub(crate) fn enter_definition_scopes(&mut self, f: DeclId) -> usize {
        let Some(ctx) = self.ast.decls.get(f).ctx() else { return 0 };
        let lexical = self.ast.decls.lexical_ctx(f).unwrap_or(ctx);
        if self.kind_of(lexical) == DeclKind::CXXRecord
            && matches!(self.kind_of(f), DeclKind::CXXMethod | DeclKind::Function)
        {
            self.ast
                .decls
                .mark_function(f, kiln_ast::decl::FnSpec::new().with_implicit_inline(true));
        }
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
            let kind = if self.kind_of(x) == DeclKind::CXXRecord {
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
        if self.kind_of(f) != DeclKind::CXXMethod {
            return None;
        }
        if self.ast.decls.function(f).is_some_and(|x| {
            x.spec.storage() == kiln_ast::decl::FnStorage::Static
        }) {
            return None;
        }
        let class = self.ast.decls.get(f).ctx()?;
        let fty = self.ast.decls.get(f).ty()?;
        let q = self.ast.types.proto(fty).map_or(Quals::NONE, |p| p.info().this_quals());
        let t = self.tag_type(class);
        let t = if q.is_empty() { t } else { self.ast.types.qualify(t, q) };
        Some(self.ast.types.pointer_to(t))
    }

    /// The definition bits of class `r` at its `}` that depend on its
    /// bases, member access and default member initializers ([class.prop],
    /// [dcl.init.aggr]/1, [temp.param]/7), on top of `bits` (computed from
    /// the fields alone). clang's `CXXRecordDecl::setBases` / `addedMember`.
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
        let not_sl = |bits: &mut DefinitionBits| *bits = bits.without(DefinitionBits::STANDARD_LAYOUT);
        // Bases ([class.prop]/3, [dcl.init.aggr]/1.4).
        let bases: Vec<BaseSpec> = self.ast.decls.bases(r, &self.ast.lists).copied().collect();
        let mut bases_with_fields = false;
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
            if self.class_has_fields(bd, 0) {
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
                self.ast
                    .decls
                    .record(b)
                    .is_some_and(|x| x.defn.contains(DefinitionBits::EMPTY))
            })
            .collect();
        // Fields.
        let union = self.ast.decls.record(r).is_some_and(|x| x.tag == TagKind::Union);
        let mut accesses = [false; 3];
        let mut first = true;
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
            if let Some(fr) = self.ast.types.as_record(et).and_then(|d| self.ast.definition(d)) {
                let fb = self.ast.decls.record(fr).map(|x| x.defn).unwrap_or_default();
                and_with(&mut bits, fb);
                if !fb.contains(DefinitionBits::STANDARD_LAYOUT) {
                    not_sl(&mut bits);
                }
                if (union || first) && !empty_bases.is_empty() && self.zero_offset_hits(fr, &empty_bases) {
                    not_sl(&mut bits);
                }
            }
            first = false;
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
    fn class_has_fields(&self, c: DeclId, depth: usize) -> bool {
        if depth > 256 {
            return false;
        }
        if self.ast.decls.members(c).any(|m| self.kind_of(m) == DeclKind::Field) {
            return true;
        }
        let bases: Vec<QualType> = self.ast.decls.bases(c, &self.ast.lists).map(|b| b.ty).collect();
        bases.into_iter().any(|t| {
            self.ast
                .types
                .as_record(t)
                .and_then(|d| self.ast.definition(d))
                .is_some_and(|d| self.class_has_fields(d, depth + 1))
        })
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
            if seen.contains(&d) {
                rep = true;
            } else {
                seen.push(d);
            }
            rep |= self.repeated_base(d, seen, depth + 1);
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
                if fd.is_some_and(|d| d.bit_width.is_some()) && self.ast.decls.get(f).name().is_empty() {
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
