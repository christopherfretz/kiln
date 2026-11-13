//! Uses of deprecated entities ([dcl.attr.deprecated]/4; spec/sema.md
//! "Deprecation warnings"): clang's `DiagnoseUseOfDecl` availability
//! check, reduced to `[[deprecated]]` / `__attribute__((deprecated))` and
//! clang's `unavailable` (an error, WP-954; suppressed only in an
//! unavailable context).
//!
//! A use is warned (`-Wdeprecated-declarations`) unless it occurs in a
//! deprecated context: inside a deprecated declaration (or a declaration
//! nested in one), or in the decl-specifiers, declarator or initializer
//! of a declaration that is itself deprecated (clang's delayed
//! diagnostics, `ShouldDiagnoseAvailabilityInContext`).
//!
//! Also the deprecated uses of volatile of [depr.volatile.type]
//! (`-Wdeprecated-volatile`): volatile-qualified parameter types, the
//! increment / decrement of a volatile object, the use of the result of
//! an assignment to one, and a volatile structured binding declaration.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::ids::RawId;
use kiln_ast::templates::SpecKind;
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::ParamInfo;

impl Sema<'_, '_> {
    /// The availability attribute governing a use of `d` and the
    /// declaration it was found on (the one the note names): `d`'s own
    /// (`unavailable` before `deprecated`, clang `Decl::getAvailability`);
    /// an enumerator's enumeration's; the class or enumeration a typedef
    /// or alias names, when the typedef itself is available (clang
    /// `ShouldDiagnoseAvailabilityOfDecl`).
    fn deprecation_of(&self, d: DeclId) -> Option<(AttrId, DeclId)> {
        if let Some(a) = self.own_availability(d) {
            return Some((a, d));
        }
        match self.kind_of(d) {
            DeclKind::EnumConstant => {
                let e = self.ast.decls.get(d).ctx()?;
                (self.kind_of(e) == DeclKind::Enum)
                    .then(|| self.own_availability(e).map(|a| (a, e)))
                    .flatten()
            }
            DeclKind::Typedef | DeclKind::TypeAlias => {
                let u = self.ast.decls.typedef(d)?.underlying;
                let tag = self.ast.types.as_record(u).or_else(|| self.ast.types.as_enum(u))?;
                let tag = self.ast.latest(tag);
                self.own_availability(tag).map(|a| (a, tag))
            }
            _ => None,
        }
    }

    /// `d`'s own `unavailable` attribute, else its own `deprecated` one.
    fn own_availability(&self, d: DeclId) -> Option<AttrId> {
        self.own_avail(d, AttrKind::UNAVAILABLE).or_else(|| self.own_avail(d, AttrKind::DEPRECATED))
    }

    /// `d`'s own `kind` attribute (`deprecated` or `unavailable`; clang
    /// `Decl::isDeprecated` / `isUnavailable`). A class template
    /// specialization not yet instantiated has its template pattern's:
    /// clang's `CheckTemplateIdType` instantiates the pattern's
    /// attributes onto it before diagnosing the use.
    fn own_avail(&self, d: DeclId, kind: AttrKind) -> Option<AttrId> {
        if self.kind_of(d) == DeclKind::Namespace {
            // Not inherited by a reopening (`act_on_namespace_attrs`).
            let l = self.ast.latest(d);
            return self
                .ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(l))
                .find(|&a| self.ast.attrs.get(a).kind == kind);
        }
        if let Some(a) = self.find_attr(self.ast.latest(d), kind) {
            return Some(a);
        }
        if self.ast.decls.var(d).is_some() {
            // An implicit or explicitly instantiated variable template
            // specialization has its pattern's (the primary template's
            // variable or the partial specialization it was instantiated
            // from; clang `InstantiateAttrs` in
            // `BuildVariableInstantiation`).
            let (_, info) = self.ast.spec_info(d)?;
            if matches!(info.kind, SpecKind::Undeclared | SpecKind::ExplicitSpecialization)
                || info.from == d
            {
                return None;
            }
            return self.find_attr(self.ast.latest(info.from), kind);
        }
        self.ast.decls.record(d)?;
        let (_, info) = self.ast.spec_info(d)?;
        if info.kind != SpecKind::Undeclared {
            return None;
        }
        let pattern = self.pattern_of(info.template)?;
        self.find_attr(self.ast.latest(pattern), kind)
    }

    /// Whether `ctx` or a context enclosing it is unavailable (`unavail`)
    /// or either deprecated or unavailable: a deprecated use there is not
    /// warned, an unavailable one only in an unavailable context (clang
    /// `ShouldDiagnoseAvailabilityInContext`, which walks the semantic
    /// contexts).
    fn in_avail_context(&self, ctx: DeclId, unavail: bool) -> bool {
        let mut cur = Some(ctx);
        for _ in 0..256 {
            let Some(x) = cur else { return false };
            let hit = if unavail {
                self.own_avail(x, AttrKind::UNAVAILABLE).is_some()
            } else {
                self.own_availability(x).is_some()
            };
            if hit {
                return true;
            }
            cur = self.ast.decls.get(x).ctx();
        }
        false
    }

    /// How a declaration names itself in a diagnostic (clang
    /// `getNameForDiagnostic`): a template specialization prints its
    /// template arguments, `f<int>`, `X<int *, int>`, `v<long>`.
    pub(crate) fn name_for_diagnostic(&self, d: DeclId) -> String {
        let mut name = self.decl_name(d);
        if let Some((_, i)) = self.ast.spec_info(d) {
            let names = crate::names::sema_names(self);
            let types = &self.ast.types;
            if self.ast.decls.record(d).is_some() {
                kiln_ast::types::write_arg_list_scoped_of(
                    types,
                    &names,
                    d,
                    i.print_args,
                    &mut name,
                );
            } else if self.ast.decls.var(d).is_some() {
                // clang `VarTemplateSpecializationDecl::getNameForDiagnostic`:
                // canonical, without trailing defaulted arguments, even
                // for an explicit specialization written with a typedef
                // (`g<L *>` is `'g<long *>'`).
                let args = types.canonical_args(i.print_args);
                kiln_ast::types::write_arg_list_scoped_of(types, &names, d, args, &mut name);
            } else if self.ast.decls.function(d).is_some() {
                // clang's function template specialization keeps the
                // canonical arguments (`CanonicalBuilder`); the deduced
                // ones carry the call arguments' sugar.
                let args = types.canonical_args(i.args);
                let names = names.typed_args_of(d);
                kiln_ast::types::write_arg_list_scoped_of(types, &names, d, args, &mut name);
            }
        }
        name
    }

    /// `[[deprecated]]` ([dcl.attr.deprecated]/4): warns a use of `d` at
    /// `loc`, naming `d`; the note names the declaration the attribute is
    /// on. Inside a declaration's delay point (its decl-specifiers and
    /// declarator, at parse time) the warning waits for the declaration
    /// ([`Sema::flush_delayed_deprecations`]).
    pub(crate) fn diagnose_deprecated(&mut self, d: DeclId, loc: Loc) {
        // clang's `DiagnoseUseOfDecl` reports a specialization's held-back
        // substitution warnings first (spec/sema-templates.md §5.7).
        self.emit_suppressed_diags(d, loc);
        self.diagnose_if_use(d, loc);
        self.warn_deprecated_use(d, loc);
    }

    /// The `[[deprecated]]` part of [`Sema::diagnose_deprecated`] alone:
    /// for a constructor selected by overload resolution (clang
    /// `PerformConstructorInitialization`'s `DiagnoseUseOfDecl`), whose
    /// `diagnose_if` conditions the call checks with its arguments.
    pub(crate) fn warn_deprecated_use(&mut self, d: DeclId, loc: Loc) {
        let Some((a, owner)) = self.deprecation_of(d) else { return };
        self.warn_availability(d, a, owner, None, loc);
    }

    /// [`Sema::warn_deprecated_use`] of member function template `t`
    /// named by an overloaded operator call (clang `CreateFunctionRefExpr`
    /// calls `DiagnoseUseOfDecl` on the found template and then on the
    /// specialization): the template is deprecated when its pattern is,
    /// and the note is at the template itself (clang 19 and 22 warn
    /// `'operator()'` and `'operator()<int>'` for a generic lambda).
    pub(crate) fn warn_deprecated_template_use(&mut self, t: DeclId, loc: Loc) {
        let Some(pattern) = self.pattern_of(t) else { return };
        let Some((a, _)) = self.deprecation_of(pattern) else { return };
        let at = self.ast.decls.get(t).loc();
        self.warn_availability(t, a, t, Some(at), loc);
    }

    /// The warning of a use at `loc` of `d`, whose availability attribute
    /// `a` is on `owner`; the note at `note_at` or else at the attribute.
    fn warn_availability(
        &mut self,
        d: DeclId,
        a: AttrId,
        owner: DeclId,
        note_at: Option<Loc>,
        loc: Loc,
    ) {
        let attr = *self.ast.attrs.get(a);
        let unavail = attr.kind == AttrKind::UNAVAILABLE;
        let name = self.name_for_diagnostic(d);
        let text = ExprId::from_raw(attr.a).and_then(|m| self.string_msg(m));
        let (plain, with_msg) = if unavail {
            (DiagId::sema_unavailable, DiagId::sema_unavailable_msg)
        } else {
            (DiagId::sema_deprecated, DiagId::sema_deprecated_msg)
        };
        let msg = match text {
            Some(t) => self.diag(with_msg, loc).arg(name).arg(t),
            None => self.diag(plain, loc).arg(name),
        };
        // clang `DoEmitAvailabilityWarning`: a deprecation's note is at
        // its attribute; an unavailable one's at the declaration the use
        // found (its latest redeclaration).
        let note = if unavail {
            let at = self.ast.decls.get(self.ast.latest(owner)).loc();
            self.diag(DiagId::note_marked_unavailable, at)
        } else {
            self.diag(DiagId::note_marked_deprecated, note_at.unwrap_or(attr.loc))
        };
        let mut msg = msg.with_note(note.arg(self.name_for_diagnostic(owner)));
        if note_at.is_some() {
            // The template's warning shares the specialization's id and
            // location; both print.
            msg = msg.allow_duplicate();
        }
        if self.inst.stack.is_empty() && self.depr_points.last() == Some(&self.ctx) {
            let level = self.depr_points.len();
            self.delayed_deprecations.push((level, msg, unavail));
            return;
        }
        if self.in_avail_context(self.ctx, unavail)
            || self.depr_init.is_some_and(|v| self.in_avail_context(v, unavail))
        {
            return;
        }
        self.emit(msg);
    }

    /// The attribute-specifier-seq of namespace definition `ns`
    /// ([namespace.def]/1, [dcl.attr.grammar]/5). It belongs to this
    /// definition only: a reopening neither inherits nor adds to it, and a
    /// name lookup finding the namespace names its latest definition
    /// (clang merges no namespace attributes), so `[[deprecated]]` warns
    /// only while that definition is the latest. On an unnamed namespace
    /// `[[deprecated]]` is ignored with a warning, as clang does.
    pub fn act_on_namespace_attrs(&mut self, ns: DeclId, attrs: &[AttrId]) {
        let mut attrs = attrs.to_vec();
        self.add_pragma_attrs(ns, &mut attrs);
        if attrs.is_empty() {
            return;
        }
        self.take_annotations(ns, &mut attrs);
        self.check_internal_linkage_attrs(ns, &mut attrs);
        self.check_tls_model_attrs(ns, &mut attrs);
        self.check_visibility_attrs(ns, &mut attrs);
        if attrs.iter().any(|&a| self.ast.attrs.get(a).kind == AttrKind::VISIBILITY) {
            self.push_namespace_visibility(ns);
        }
        let mut kept = Vec::with_capacity(attrs.len());
        for &a in &attrs {
            let attr = *self.ast.attrs.get(a);
            if attr.kind == AttrKind::DEPRECATED && self.ast.decls.get(ns).name().is_empty() {
                let d = self.diag(DiagId::sema_deprecated_anonymous_namespace, attr.loc);
                self.emit(d);
                continue;
            }
            // `abi_tag` applies to an inline, named namespace (an inline
            // one reopened without `inline` included); clang ignores it
            // elsewhere with a warning (spec/abi.md §8.15).
            if attr.kind == AttrKind::ABI_TAG {
                let anonymous = self.ast.decls.get(ns).name().is_empty();
                let inline = self
                    .ast
                    .decls
                    .namespace(self.ast.first(ns))
                    .is_some_and(kiln_ast::decl::NamespaceData::is_inline);
                if anonymous || !inline {
                    let d = self
                        .diag(DiagId::sema_abi_tag_namespace, attr.loc)
                        .arg(u32::from(anonymous));
                    self.emit(d);
                    continue;
                }
            }
            kept.push(a);
        }
        if !kept.is_empty() {
            let _ = self.builder().attach_attrs(ns, &kept);
        }
    }

    /// Namespace `ns` named at `loc` by a nested-name-specifier, a
    /// using-directive or a namespace-alias-definition: a use
    /// ([dcl.attr.deprecated]/4). A namespace alias is not: clang warns
    /// only where the alias is defined.
    pub fn act_on_namespace_name_use(&mut self, ns: DeclId, loc: Loc) {
        if self.kind_of(ns) == DeclKind::Namespace {
            self.diagnose_deprecated(ns, loc);
        }
    }

    /// The nested-name-specifier component `n` just formed, its `::` at
    /// `cc`: a namespace it names is used there (clang's
    /// `BuildCXXNestedNameSpecifier` diagnoses at the `::`).
    pub fn act_on_nns_component_use(&mut self, n: kiln_ast::ids::NnsId, cc: Loc) {
        if let kiln_ast::types::Nns::Namespace { ns, .. } = self.ast.types.nns_view(n) {
            self.act_on_namespace_name_use(ns, cc);
        }
    }

    /// A type-name or elaborated-type-specifier naming `d` at `loc`: a
    /// class, enumeration, typedef or alias is a use (a template name is
    /// not; its specialization is, [`Sema::diagnose_template_id_use`]).
    pub(crate) fn diagnose_type_name_use(&mut self, d: DeclId, loc: Loc) {
        if matches!(
            self.kind_of(d),
            DeclKind::CXXRecord
                | DeclKind::ClassTemplateSpecialization
                | DeclKind::Enum
                | DeclKind::Typedef
                | DeclKind::TypeAlias
        ) {
            self.diagnose_deprecated(d, loc);
        }
    }

    /// A template-id naming class template specialization `spec`, with
    /// the template name at `loc` (clang `CheckTemplateIdType`), also
    /// when re-formed by substitution.
    pub(crate) fn diagnose_template_id_use(&mut self, spec: DeclId, loc: Loc) {
        self.diagnose_deprecated(spec, loc);
    }

    /// Opens a deprecation delay point (clang's `ParsingDeclRAIIObject`):
    /// deprecation warnings until the matching
    /// [`Sema::end_delayed_deprecation_to`] wait for the declaration they
    /// belong to. Returns the level to close to. Every access delay point
    /// ([`Sema::begin_delayed_access`]) is one too; the parser opens one
    /// alone around a member-declaration's decl-specifiers and first
    /// declarator.
    pub fn begin_delayed_deprecation(&mut self) -> usize {
        let level = self.depr_points.len();
        self.depr_points.push(self.ctx);
        level
    }

    /// Closes the deprecation delay points opened above `level` for
    /// declaration `decl` (`None`: the current context); none if they
    /// were already closed.
    pub fn end_delayed_deprecation_to(&mut self, level: usize, decl: Option<DeclId>) {
        while self.depr_points.len() > level {
            let l = self.depr_points.len();
            self.depr_points.pop();
            self.flush_delayed_deprecations(l, decl);
        }
    }

    /// Closes the innermost `n` deprecation delay points without
    /// emitting: `drop` discards their warnings (clang's
    /// `PopParsingDeclaration` with no declaration); otherwise they join
    /// the enclosing point, or are emitted now when there is none
    /// (clang's `redelay`).
    pub(crate) fn pop_delayed_deprecations(&mut self, n: usize, drop: bool) {
        let level = self.depr_points.len().saturating_sub(n);
        self.depr_points.truncate(level);
        if drop {
            let keep = self.delayed_deprecations.iter().take_while(|&&(l, ..)| l <= level).count();
            self.delayed_deprecations.truncate(keep);
        } else if level == 0 {
            self.flush_delayed_deprecations(1, None);
        } else {
            for (l, ..) in &mut self.delayed_deprecations {
                *l = (*l).min(level);
            }
        }
    }

    /// Emits the deprecation warnings held at `level` and above for
    /// declaration `decl`, unless `decl` (or, with none, the current
    /// context) is deprecated or in a deprecated context.
    fn flush_delayed_deprecations(&mut self, level: usize, decl: Option<DeclId>) {
        let Some(start) = self.delayed_deprecations.iter().position(|&(l, ..)| l >= level) else {
            return;
        };
        let held: Vec<(Diag, bool)> =
            self.delayed_deprecations.drain(start..).map(|(_, d, u)| (d, u)).collect();
        let at = decl.unwrap_or(self.ctx);
        for (d, unavail) in held {
            if self.in_avail_context(at, unavail)
                || self.depr_init.is_some_and(|v| self.in_avail_context(v, unavail))
            {
                continue;
            }
            self.emit(d);
        }
    }

    /// [depr.volatile.type]/2 (C++20 [dcl.fct]/4): each volatile-qualified
    /// parameter of a function declarator is deprecated, warned at the
    /// parameter (clang `GetFullTypeForDeclarator`; the caller checks the
    /// language mode).
    /// [depr.volatile.type]/3 (C++20 [expr.ass]/5): a simple assignment
    /// to volatile-qualified `lhs` is deprecated unless it is a
    /// discarded-value expression or an unevaluated operand (clang
    /// `CheckAssignmentOperands` records the left operand; a discarded
    /// one is dropped by [`Sema::discard_volatile_assign`]; the rest are
    /// warned at the end of the body or initializer,
    /// [`Sema::warn_volatile_assigns`]).
    pub(crate) fn note_volatile_assign(&mut self, lhs: ExprId) {
        if self.std < kiln_lex::Std::Cxx20
            || self.unevaluated > 0
            || !self.ast.types.effective_quals(self.ety(lhs)).is_volatile()
        {
            return;
        }
        match self.func.as_mut() {
            Some(f) => f.volatile_assigns.push(lhs),
            None => self.volatile_assigns.push(lhs),
        }
    }

    /// clang `CheckUnusedVolatileAssignment`: discarded-value expression
    /// `e`, ignoring parentheses and implicit conversions, is a simple
    /// assignment whose use is not deprecated.
    pub(crate) fn discard_volatile_assign(&mut self, e: ExprId) {
        let list = match self.func.as_ref() {
            Some(f) => &f.volatile_assigns,
            None => &self.volatile_assigns,
        };
        if list.is_empty() {
            return;
        }
        let mut x = e;
        while matches!(
            self.ast.exprs.get(x).kind(),
            ExprKind::ParenExpr | ExprKind::ImplicitCastExpr
        ) {
            x = self.ast.exprs.operand(x);
        }
        let ex = &self.ast.exprs;
        if ex.get(x).kind() != ExprKind::BinaryOperator || ex.binary_op(x) != BinaryOp::Assign {
            return;
        }
        let lhs = ex.children(x, &self.ast.lists)[0];
        let list = match self.func.as_mut() {
            Some(f) => &mut f.volatile_assigns,
            None => &mut self.volatile_assigns,
        };
        list.retain(|&l| l != lhs);
    }

    /// Warns the volatile assignments of [`Sema::note_volatile_assign`]
    /// still pending at the end of their body or initializer (clang
    /// `PopExpressionEvaluationContext`), at the left operand.
    pub(crate) fn warn_volatile_assigns(&mut self, pending: Vec<ExprId>) {
        for l in pending {
            let at = self.begin_loc(l);
            let d = self
                .diag(DiagId::sema_deprecated_volatile_assign, at)
                .arg(self.ty_arg(self.ety(l)));
            self.emit(d);
        }
    }

    /// [depr.volatile.type]/1 (C++20 [expr.pre.incr]/1, [expr.post.incr]/1):
    /// an increment or decrement of a volatile-qualified operand of type
    /// `t` is deprecated, warned at operator `loc` (clang
    /// `CheckIncrementDecrementOperand`).
    pub(crate) fn check_volatile_inc_dec(&mut self, inc: bool, t: QualType, loc: Loc) {
        if self.std >= kiln_lex::Std::Cxx20 && self.ast.types.effective_quals(t).is_volatile() {
            let d = self
                .diag(DiagId::sema_deprecated_volatile_incdec, loc)
                .arg(if inc { "0" } else { "1" })
                .arg(self.ty_arg(t));
            self.emit(d);
        }
    }

    pub(crate) fn check_volatile_params(&mut self, params: &[ParamInfo]) {
        for p in params {
            if !self.ast.types.effective_quals(p.ty).contains(kiln_ast::types::Quals::VOLATILE) {
                continue;
            }
            let at = p
                .decl
                .map_or_else(|| p.name.map_or(p.loc, |n| n.1), |pd| self.ast.decls.get(pd).loc());
            let d = self.diag(DiagId::sema_deprecated_volatile_param, at).arg(self.ty_arg(p.ty));
            self.emit(d);
        }
    }
}
