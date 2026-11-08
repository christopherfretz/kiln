//! `-Wshadow` and its flavours (clang `CheckShadow`,
//! `DiagnoseShadowingLambdaDecls`, `CheckShadowingDeclModification` and
//! `CheckShadowInheritedFields`; spec/diag.md §5).
//!
//! A block-scope variable, parameter, structured binding or typedef-name
//! that hides an entity of the same kind found by unqualified lookup
//! ([basic.scope.hiding]) is diagnosed once, at the definition: never
//! during template instantiation; so is an unscoped enumeration's
//! enumerator that hides a variable, function or enumerator. Everything but `-Wshadow-field` is
//! gated on `-Wshadow` itself, as clang's `Diags.isIgnored` checks are.

use kiln_ast::DeclId;
use kiln_ast::decl::{Access, DeclKind, NamespaceData};
use kiln_ast::ids::LambdaId;
use kiln_ast::lambda::CaptureDefault;
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::lookup::LookupKind;

impl Sema<'_, '_> {
    /// clang `CheckShadow` for the block-scope declaration `d` named
    /// `name`, before it is in scope.
    pub(crate) fn check_shadow(&mut self, d: DeclId, name: Symbol) {
        if !self.diags.is_enabled(DiagId::sema_decl_shadow)
            || self.inst.stack.in_instantiation_record()
            || !self.in_local_scope()
        {
            return;
        }
        let Some((old, found)) = self.shadow_lookup(name) else { return };
        let typedef = |k| matches!(k, DeclKind::Typedef | DeclKind::TypeAlias);
        let ok = self.kind_of(old);
        if typedef(self.kind_of(d)) {
            if !typedef(ok) {
                return;
            }
        } else if !matches!(
            ok,
            DeclKind::Var | DeclKind::ParmVar | DeclKind::Field | DeclKind::Binding
        ) {
            return;
        }
        // A declaration in the same scope is a redeclaration (or an error).
        if self
            .scopes
            .iter()
            .rev()
            .find(|s| s.is_local())
            .is_some_and(|s| s.locals.iter().any(|&(_, x, _)| x == old || x == found))
        {
            return;
        }
        self.check_shadow_of(d, old);
    }

    /// clang `ActOnEnumConstant`'s `CheckShadow`: an enumerator of the
    /// unscoped enumeration `en`, named `name` at `loc` and not yet
    /// declared, that hides a variable, function, field or enumerator.
    /// clang diagnoses even one it then rejects as a redefinition.
    pub(crate) fn check_enumerator_shadow(&mut self, en: DeclId, name: Symbol, loc: Loc) {
        if !self.diags.is_enabled(DiagId::sema_decl_shadow)
            || self.inst.stack.in_instantiation_record()
        {
            return;
        }
        let Some((old, _)) = self.shadow_lookup(name) else { return };
        // A `ValueDecl` other than a template parameter.
        if !matches!(
            self.kind_of(old),
            DeclKind::Var
                | DeclKind::ParmVar
                | DeclKind::Field
                | DeclKind::IndirectField
                | DeclKind::EnumConstant
                | DeclKind::Binding
                | DeclKind::Function
                | DeclKind::CXXMethod
        ) {
            return;
        }
        let (Some(old_dc), Some(re_dc)) = (self.shadow_redecl_ctx(old), self.shadow_redecl_ctx(en))
        else {
            return;
        };
        // clang 22: not a field from a static or explicit object member
        // function (through lambdas).
        if self.clang_compat >= 22 && self.kind_of(old) == DeclKind::Field {
            let mut f = re_dc;
            while let Some(l) = self.lambda_of_call_op(f) {
                let Some(p) = self.lambda_parent_ctx(l) else { break };
                f = p;
            }
            if self.is_static_method(f) || self.has_explicit_object(f) {
                return;
            }
        }
        // clang 21: a member enumerator hides only a member of its own
        // class.
        if self.clang_compat >= 21
            && self.kind_of(re_dc) == DeclKind::CXXRecord
            && self.ast.first(old_dc) != self.ast.first(re_dc)
        {
            return;
        }
        if self.is_placeholder_decl(old) {
            return;
        }
        let msg = self.shadow_diag(DiagId::sema_decl_shadow, loc, old, old_dc);
        let prev = self.diag(DiagId::note_previous_declaration, self.ast.decls.get(old).loc());
        self.emit(msg.with_note(prev));
    }

    /// The single entity unqualified lookup finds for `name` and the
    /// declaration through which clang's redeclaration lookup finds it (a
    /// using-declaration's shadow, for one; clang's `getFoundDecl` is the
    /// entity). The same entity found twice (through a using-declaration
    /// and a using-directive) is one result.
    fn shadow_lookup(&mut self, name: Symbol) -> Option<(DeclId, DeclId)> {
        let r = self.lookup_name(name, LookupKind::Ordinary);
        let old = *r.decls.first()?;
        if r.ambiguity.is_some()
            || r.decls.iter().any(|&d| self.ast.first(d) != self.ast.first(old))
        {
            return None;
        }
        let found = r.found.iter().copied().find(|&f| self.redecl_lookup_finds(f))?;
        Some((old, found))
    }

    /// Whether `found`, a result of unqualified lookup at the current
    /// point, is also a result of clang's redeclaration lookup
    /// (`ForVisibleRedeclaration`), which `CheckShadow` uses: it does not
    /// look into base classes, and of the namespaces it searches only those
    /// up to the innermost enclosing namespace scope (an out-of-line
    /// definition's semantic ones too), not the namespaces outside it.
    fn redecl_lookup_finds(&self, found: DeclId) -> bool {
        let Some(mut c) = self.shadow_redecl_ctx(found) else { return false };
        match self.kind_of(c) {
            DeclKind::CXXRecord => return self.shadow_encloses(c, self.ctx),
            DeclKind::Namespace | DeclKind::TranslationUnit => {}
            _ => return true,
        }
        // The innermost lexically enclosing namespace: an out-of-line
        // definition's scopes include its semantic namespaces, which
        // redeclaration lookup searches only as far as the lexical one.
        let mut lex = None;
        let mut y = Some(self.ctx);
        for _ in 0..1024 {
            let Some(x) = y else { break };
            if matches!(self.kind_of(x), DeclKind::Namespace | DeclKind::TranslationUnit) {
                lex = Some(x);
                break;
            }
            y = self.ast.decls.lexical_ctx(x);
        }
        let Some(lex) = lex else { return true };
        let lex = self.ast.first(lex);
        let searched = |c: DeclId| {
            let c = self.ast.first(c);
            let mut x = Some(self.ctx);
            while let Some(y) = x {
                let y1 = self.ast.first(y);
                if y1 == c {
                    return true;
                }
                if y1 == lex {
                    return false;
                }
                x = self.ast.decls.get(y).ctx();
            }
            false
        };
        // A member of an inline namespace is found in the enclosing one,
        // and of an unnamed namespace through its implicit
        // using-directive ([namespace.unnamed]).
        for _ in 0..256 {
            if searched(c) {
                return true;
            }
            if !(self.is_inline_ns(c) || self.is_anon_ns(c)) {
                return false;
            }
            let Some(p) = self.shadow_redecl_ctx(c) else { return false };
            c = p;
        }
        false
    }

    fn is_inline_ns(&self, ns: DeclId) -> bool {
        self.ast.decls.namespace(self.ast.first(ns)).is_some_and(NamespaceData::is_inline)
    }

    fn is_anon_ns(&self, ns: DeclId) -> bool {
        self.kind_of(ns) == DeclKind::Namespace
            && self.ast.decls.get(ns).name().as_ident().is_none()
    }

    fn check_shadow_of(&mut self, d: DeclId, old: DeclId) {
        let new_dc = self.ctx;
        let dk = self.kind_of(d);
        let ok = self.kind_of(old);
        let is_var = matches!(dk, DeclKind::Var | DeclKind::ParmVar);
        if ok == DeclKind::Field {
            // Fields are not shadowed in a static member function: clang
            // 22 also looks through lambdas and skips an explicit object
            // member function, clang 19 checks the declaration's context.
            if self.clang_compat >= 22 {
                let mut f = new_dc;
                while let Some(l) = self.lambda_of_call_op(f) {
                    let Some(p) = self.lambda_parent_ctx(l) else { break };
                    f = p;
                }
                if self.is_static_method(f) || self.has_explicit_object(f) {
                    return;
                }
            } else if self.is_static_method(new_dc) {
                return;
            }
            // A constructor's parameter: diagnosed at the end of its
            // definition unless modified first.
            if self.kind_of(new_dc) == DeclKind::CXXConstructor && dk == DeclKind::ParmVar {
                self.shadow_ctor.push((d, old));
                return;
            }
        }
        let Some(old_dc) = self.shadow_redecl_ctx(old) else { return };
        let mut capture_loc = None;
        let mut uncaptured = false;
        if is_var
            && let Some(l) = self.lambda_of_call_op(new_dc)
            && self.lambda_parent_ctx(l).is_some_and(|p| self.shadow_encloses(old_dc, p))
        {
            let data = self.ast.exprs.lambdas.get(l);
            match ok {
                DeclKind::Var | DeclKind::ParmVar if data.default == CaptureDefault::None => {
                    // Warn only when the lambda captures it explicitly.
                    capture_loc = self.var_capture_loc(l, old);
                    uncaptured = capture_loc.is_none();
                }
                DeclKind::Var | DeclKind::ParmVar | DeclKind::Field => {
                    // Whether it is captured is known at the lambda's end.
                    if let Some(s) = self.lambdas.iter_mut().rev().find(|s| s.id == l) {
                        s.shadowing.push((d, old));
                    }
                    return;
                }
                _ => {}
            }
        }
        if is_var
            && self.is_method_kind(new_dc)
            && matches!(ok, DeclKind::Var | DeclKind::ParmVar)
            && self.has_local_storage(old)
        {
            // A local variable is not shadowed across a declaration
            // context that cannot capture it (a local class).
            let mut p = Some(new_dc);
            while let Some(dc) = p
                && self.ast.first(dc) != self.ast.first(old_dc)
            {
                let Some(l) = self.lambda_of_call_op(dc) else { return };
                p = self.lambda_parent_ctx(l);
            }
        }
        if self.is_placeholder_decl(old) {
            return;
        }
        let id = if uncaptured {
            DiagId::sema_decl_shadow_uncaptured_local
        } else {
            DiagId::sema_decl_shadow
        };
        self.emit_shadow(id, d, old, old_dc, capture_loc.map(|l| (l, 1)));
    }

    /// The warning `id` for `d` shadowing `old` (whose redeclaration
    /// context is `old_dc`), with the capture note at `capture`.
    fn emit_shadow(
        &mut self,
        id: DiagId,
        d: DeclId,
        old: DeclId,
        old_dc: DeclId,
        capture: Option<(Loc, u32)>,
    ) {
        let loc = self.ast.decls.get(d).loc();
        let mut msg = self.shadow_diag(id, loc, old, old_dc);
        if let Some((cl, explicit)) = capture {
            let note = self
                .diag(DiagId::note_var_explicitly_captured_here, cl)
                .arg(self.decl_arg(d))
                .arg(explicit);
            msg = msg.with_note(note);
        }
        let prev = self.diag(DiagId::note_previous_declaration, self.ast.decls.get(old).loc());
        self.emit(msg.with_note(prev));
    }

    /// The `-Wshadow` warning `id` at `loc` for a declaration hiding `old`
    /// (clang `computeShadowedDeclKind`), without its notes.
    fn shadow_diag(&self, id: DiagId, loc: Loc, old: DeclId, old_dc: DeclId) -> Diag {
        // The select of the id's template: clang's order, but with the
        // structured binding (the one kind without a context) first.
        let kind: u32 = match self.kind_of(old) {
            DeclKind::TypeAlias => 6,
            DeclKind::Typedef => 5,
            DeclKind::Binding => 0,
            k if self.kind_of(old_dc) == DeclKind::CXXRecord => {
                if k == DeclKind::Field {
                    4
                } else {
                    3
                }
            }
            _ if matches!(
                self.kind_of(old_dc),
                DeclKind::TranslationUnit | DeclKind::Namespace
            ) =>
            {
                2
            }
            _ => 1,
        };
        self.diag(id, loc).arg(kind).arg(self.shadow_ctx_desc(old_dc))
    }

    /// clang `DiagnoseShadowingLambdaDecls`: the declarations of lambda
    /// `l` with a capture-default that shadow a local entity or a field,
    /// once its captures are known.
    pub(crate) fn diagnose_shadowing_lambda_decls(
        &mut self,
        l: LambdaId,
        list: &[(DeclId, DeclId)],
    ) {
        for &(d, old) in list {
            let Some(old_dc) = self.shadow_redecl_ctx(old) else { continue };
            if self.kind_of(old) == DeclKind::Field {
                let id = if self.this_captured(l) {
                    DiagId::sema_decl_shadow
                } else {
                    DiagId::sema_decl_shadow_uncaptured_local
                };
                self.emit_shadow(id, d, old, old_dc, None);
            } else {
                let cl = self.var_capture_loc(l, old);
                let id = if cl.is_some() {
                    DiagId::sema_decl_shadow
                } else {
                    DiagId::sema_decl_shadow_uncaptured_local
                };
                self.emit_shadow(id, d, old, old_dc, cl.map(|l| (l, 0)));
            }
        }
    }

    /// The end of constructor `f`'s definition: its parameters that shadow
    /// a field and were not modified (clang `ActOnPopScope`).
    pub(crate) fn diagnose_ctor_param_shadows(&mut self, f: DeclId) {
        if self.shadow_ctor.is_empty() {
            return;
        }
        let mine: Vec<(DeclId, DeclId)> = {
            let params = self.ast.decls.function(f).map(|x| x.params);
            let params: Vec<DeclId> =
                params.map(|p| self.ast.lists.iter::<DeclId>(p).collect()).unwrap_or_default();
            let (mine, rest) = self.shadow_ctor.drain(..).partition(|(p, _)| params.contains(p));
            self.shadow_ctor = rest;
            mine
        };
        for (p, field) in mine {
            let Some(rec) = self.ast.decls.get(field).ctx() else { continue };
            let msg = self
                .diag(DiagId::sema_ctor_parm_shadows_field, self.ast.decls.get(p).loc())
                .arg(self.decl_arg(p))
                .arg(self.decl_arg(field))
                .arg(self.decl_arg(rec));
            let prev =
                self.diag(DiagId::note_previous_declaration, self.ast.decls.get(field).loc());
            self.emit(msg.with_note(prev));
        }
    }

    /// clang `CheckShadowingDeclModification`: `e`, about to be modified
    /// at `loc`, names a constructor parameter that shadows a field.
    pub(crate) fn check_shadowing_decl_modification(&mut self, e: kiln_ast::ExprId, loc: Loc) {
        if self.shadow_ctor.is_empty() {
            return;
        }
        let e = self.ignore_paren_imp_casts(e);
        if self.ast.exprs.get(e).kind() != kiln_ast::expr::ExprKind::DeclRefExpr {
            return;
        }
        let d = self.ast.first(self.ast.exprs.decl(e));
        let Some(i) = self.shadow_ctor.iter().position(|&(p, _)| p == d) else { return };
        let (p, field) = self.shadow_ctor.remove(i);
        let Some(rec) = self.ast.decls.get(field).ctx() else { return };
        let msg = self
            .diag(DiagId::sema_modifying_shadowing_decl, loc)
            .arg(self.decl_arg(p))
            .arg(self.shadow_ctx_desc(rec));
        let declared = self
            .diag(DiagId::note_var_declared_here, self.ast.decls.get(p).loc())
            .arg(self.decl_arg(p));
        let prev = self.diag(DiagId::note_previous_declaration, self.ast.decls.get(field).loc());
        self.emit(msg.with_note(declared).with_note(prev));
    }

    /// clang `CheckShadowInheritedFields`: the non-static data member
    /// (`is_field`) or member function parameter `name` at `loc` of class
    /// `rd` hides a non-private field of a base class reached through
    /// base-specifiers that are not private past the first step
    /// (`-Wshadow-field`).
    pub(crate) fn check_shadow_inherited_fields(
        &mut self,
        loc: Loc,
        name: Symbol,
        rd: DeclId,
        is_field: bool,
    ) {
        if !self.diags.is_enabled(DiagId::sema_shadow_field)
            || self.inst.stack.in_instantiation_record()
            || self.kind_of(rd) != DeclKind::CXXRecord
        {
            return;
        }
        let mut walk = FieldWalk::default();
        self.shadow_field_walk(rd, name, true, &mut walk, 0);
        for (b, accessible) in walk.paths {
            let Some(i) = walk.bases.iter().position(|&(x, _)| x == b) else { continue };
            if !accessible {
                continue;
            }
            let (_, field) = walk.bases.remove(i);
            let msg = self
                .diag(DiagId::sema_shadow_field, loc)
                .arg(format!("'{}'", self.ast.symbols().as_str(name)))
                .arg(self.decl_arg(rd))
                .arg(self.decl_arg(b))
                .arg(u32::from(is_field));
            let note = self.diag(DiagId::note_declared_at, self.ast.decls.get(field).loc());
            self.emit(msg.with_note(note));
        }
    }

    /// clang `CXXRecordDecl::lookupInBases` with `CheckShadowInheritedFields`'
    /// callback, from class `r` reached with path access `accessible`.
    fn shadow_field_walk(
        &self,
        r: DeclId,
        name: Symbol,
        accessible: bool,
        w: &mut FieldWalk,
        depth: u32,
    ) {
        let Some(def) = self.ast.definition(r) else { return };
        if depth > 64 {
            return;
        }
        for spec in self.ast.decls.bases(def, &self.ast.lists) {
            if self.ast.types.is_dependent(spec.ty) {
                continue;
            }
            let Some(b) = self.ast.types.as_record(spec.ty).and_then(|b| self.ast.definition(b))
            else {
                continue;
            };
            // `private` is allowed only on the first step (clang's
            // top-down path access, `CXXBasePaths::lookupInBases`).
            let acc =
                depth == 0 || accessible && self.base_access(def, spec.access) != Access::Private;
            let visit = if spec.is_virtual {
                let v = !w.virtuals.contains(&b);
                w.virtuals.push(b);
                v
            } else {
                true
            };
            let matched = w.bases.iter().any(|&(x, _)| x == b) || {
                let f =
                    self.lookup_in(b, name, LookupKind::Ordinary).decls.into_iter().find(|&f| {
                        matches!(self.kind_of(f), DeclKind::Field | DeclKind::IndirectField)
                            && self.ast.decls.get(f).misc().access() != Access::Private
                    });
                f.inspect(|&f| w.bases.push((b, f))).is_some()
            };
            if matched {
                w.paths.push((b, acc));
            } else if visit {
                self.shadow_field_walk(b, name, acc, w, depth + 1);
            }
        }
    }

    /// clang's `getRedeclContext` of `d`'s context: linkage specifications,
    /// export declarations and unscoped enumerations are transparent.
    fn shadow_redecl_ctx(&self, d: DeclId) -> Option<DeclId> {
        let mut c = self.ast.decls.get(d).ctx()?;
        while matches!(self.kind_of(c), DeclKind::LinkageSpec | DeclKind::Export)
            || self.kind_of(c) == DeclKind::Enum && !self.is_scoped_enum(c)
        {
            c = self.ast.decls.get(c).ctx()?;
        }
        Some(c)
    }

    /// clang `DeclContext::Encloses`: `outer` is `inner` or a semantic
    /// ancestor of it.
    fn shadow_encloses(&self, outer: DeclId, inner: DeclId) -> bool {
        let outer = self.ast.first(outer);
        let mut c = Some(inner);
        while let Some(x) = c {
            if self.ast.first(x) == outer {
                return true;
            }
            c = self.ast.decls.get(x).ctx();
        }
        false
    }

    /// The context enclosing lambda `l`'s closure type.
    fn lambda_parent_ctx(&self, l: LambdaId) -> Option<DeclId> {
        self.ast.decls.get(self.ast.exprs.lambdas.get(l).closure).ctx()
    }

    /// Where lambda `l` captures variable `v` (clang `getCaptureLocation`).
    fn var_capture_loc(&self, l: LambdaId, v: DeclId) -> Option<Loc> {
        self.ast
            .exprs
            .lambdas
            .get(l)
            .captures
            .iter()
            .find(|c| !c.init && c.var == Some(v))
            .map(|c| c.loc)
    }

    /// Lambda `l` captures `*this` (or `this`).
    fn this_captured(&self, l: LambdaId) -> bool {
        self.ast.exprs.lambdas.get(l).captures.iter().any(|c| c.var.is_none() && !c.init)
    }

    /// clang `NamedDecl::isPlaceholderVar`: `d` is a name-independent
    /// declaration of `_` ([basic.scope.scope]/5).
    fn is_placeholder_decl(&self, d: DeclId) -> bool {
        let Some(n) = self.ast.decls.get(d).name().as_ident() else { return false };
        if !self.is_placeholder_name(n) {
            return false;
        }
        match self.kind_of(d) {
            DeclKind::Field | DeclKind::IndirectField => true,
            DeclKind::Var | DeclKind::Binding => self.has_local_storage(d),
            _ => false,
        }
    }

    /// A declaration context as clang prints one in a diagnostic.
    fn shadow_ctx_desc(&self, dc: DeclId) -> String {
        match self.kind_of(dc) {
            // clang names an unnamed namespace itself `(anonymous)`.
            DeclKind::Namespace if self.is_anon_ns(dc) => {
                match self
                    .ast
                    .decls
                    .get(dc)
                    .ctx()
                    .filter(|&p| self.kind_of(p) == DeclKind::Namespace)
                {
                    Some(p) => format!("namespace '{}::(anonymous)'", self.qualified_decl_name(p)),
                    None => "namespace '(anonymous)'".to_owned(),
                }
            }
            DeclKind::TranslationUnit | DeclKind::Namespace | DeclKind::CXXRecord => {
                self.qualifier_desc(dc)
            }
            _ if self.lambda_of_call_op(dc).is_some() => "lambda expression".to_owned(),
            DeclKind::Function
            | DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion => format!("function '{}'", self.qualified_decl_name(dc)),
            _ => format!("'{}'", self.qualified_decl_name(dc)),
        }
    }
}

/// [`Sema::shadow_field_walk`]'s state.
#[derive(Default)]
struct FieldWalk {
    /// Per base class, the first non-private field named so found in it.
    bases: Vec<(DeclId, DeclId)>,
    /// The paths found, in order: the base class at the end and whether
    /// every base-specifier on the path is non-private.
    paths: Vec<(DeclId, bool)>,
    /// The virtual bases visited.
    virtuals: Vec<DeclId>,
}
