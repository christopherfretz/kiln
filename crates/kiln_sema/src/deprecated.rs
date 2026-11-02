//! Uses of deprecated entities ([dcl.attr.deprecated]/4; spec/sema.md
//! "Deprecation warnings"): clang's `DiagnoseUseOfDecl` availability
//! check, reduced to `[[deprecated]]` / `__attribute__((deprecated))`.
//!
//! A use is warned (`-Wdeprecated-declarations`) unless it occurs in a
//! deprecated context: inside a deprecated declaration (or a declaration
//! nested in one), or in the decl-specifiers, declarator or initializer
//! of a declaration that is itself deprecated (clang's delayed
//! diagnostics, `ShouldDiagnoseAvailabilityInContext`).

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::RawId;
use kiln_ast::templates::SpecKind;
use kiln_ast::{AttrId, DeclId, ExprId};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// The deprecation governing a use of `d` and the declaration it was
    /// found on (the one the note names): `d`'s own; an enumerator's
    /// enumeration's; the class or enumeration a typedef or alias names,
    /// when the typedef itself is not deprecated (clang
    /// `ShouldDiagnoseAvailabilityOfDecl`).
    fn deprecation_of(&self, d: DeclId) -> Option<(AttrId, DeclId)> {
        if let Some(a) = self.own_deprecation(d) {
            return Some((a, d));
        }
        match self.kind_of(d) {
            DeclKind::EnumConstant => {
                let e = self.ast.decls.get(d).ctx()?;
                (self.kind_of(e) == DeclKind::Enum)
                    .then(|| self.own_deprecation(e).map(|a| (a, e)))
                    .flatten()
            }
            DeclKind::Typedef | DeclKind::TypeAlias => {
                let u = self.ast.decls.typedef(d)?.underlying;
                let tag = self.ast.types.as_record(u).or_else(|| self.ast.types.as_enum(u))?;
                let tag = self.ast.latest(tag);
                self.own_deprecation(tag).map(|a| (a, tag))
            }
            _ => None,
        }
    }

    /// `d`'s own deprecation attribute (clang `Decl::isDeprecated`). A
    /// class template specialization not yet instantiated has its
    /// template pattern's: clang's `CheckTemplateIdType` instantiates the
    /// pattern's attributes onto it before diagnosing the use.
    fn own_deprecation(&self, d: DeclId) -> Option<AttrId> {
        if let Some(a) = self.find_attr(self.ast.latest(d), AttrKind::DEPRECATED) {
            return Some(a);
        }
        self.ast.decls.record(d)?;
        let (_, info) = self.ast.spec_info(d)?;
        if info.kind != SpecKind::Undeclared {
            return None;
        }
        let pattern = self.pattern_of(info.template)?;
        self.find_attr(self.ast.latest(pattern), AttrKind::DEPRECATED)
    }

    /// Whether `ctx` or a context enclosing it is deprecated: a use there
    /// is not warned (clang `ShouldDiagnoseAvailabilityInContext`, which
    /// walks the semantic contexts).
    pub(crate) fn in_deprecated_context(&self, ctx: DeclId) -> bool {
        let mut cur = Some(ctx);
        for _ in 0..256 {
            let Some(x) = cur else { return false };
            if self.own_deprecation(x).is_some() {
                return true;
            }
            cur = self.ast.decls.get(x).ctx();
        }
        false
    }

    /// How a deprecated declaration names itself (clang
    /// `getNameForDiagnostic`): a template specialization prints its
    /// template arguments, `f<int>`, `X<int *, int>`.
    fn deprecation_name(&self, d: DeclId) -> String {
        let mut name = self.decl_name(d);
        if let Some((_, i)) = self.ast.spec_info(d) {
            let names = crate::names::SemaNames::new(self);
            let types = &self.ast.types;
            if self.ast.decls.record(d).is_some() {
                kiln_ast::types::write_arg_list_scoped(types, &names, i.print_args, &mut name);
            } else if self.ast.decls.function(d).is_some() {
                kiln_ast::types::write_arg_list(types, &names, i.args, &mut name);
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
        let Some((a, owner)) = self.deprecation_of(d) else { return };
        let attr = *self.ast.attrs.get(a);
        let name = self.deprecation_name(d);
        let text = ExprId::from_raw(attr.a).and_then(|m| self.string_text(m));
        let msg = match text {
            Some(t) => self.diag(DiagId::sema_deprecated_msg, loc).arg(name).arg(t),
            None => self.diag(DiagId::sema_deprecated, loc).arg(name),
        };
        let note =
            self.diag(DiagId::note_marked_deprecated, attr.loc).arg(self.deprecation_name(owner));
        let msg = msg.with_note(note);
        if self.inst.stack.is_empty() && self.depr_points.last() == Some(&self.ctx) {
            let level = self.depr_points.len();
            self.delayed_deprecations.push((level, msg));
            return;
        }
        if self.in_deprecated_context(self.ctx)
            || self.depr_init.is_some_and(|v| self.in_deprecated_context(v))
        {
            return;
        }
        self.emit(msg);
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
            let keep = self.delayed_deprecations.iter().take_while(|&&(l, _)| l <= level).count();
            self.delayed_deprecations.truncate(keep);
        } else if level == 0 {
            self.flush_delayed_deprecations(1, None);
        } else {
            for (l, _) in &mut self.delayed_deprecations {
                *l = (*l).min(level);
            }
        }
    }

    /// Emits the deprecation warnings held at `level` and above for
    /// declaration `decl`, unless `decl` (or, with none, the current
    /// context) is deprecated or in a deprecated context.
    fn flush_delayed_deprecations(&mut self, level: usize, decl: Option<DeclId>) {
        let Some(start) = self.delayed_deprecations.iter().position(|&(l, _)| l >= level) else {
            return;
        };
        let held: Vec<Diag> = self.delayed_deprecations.drain(start..).map(|(_, d)| d).collect();
        let quiet = match decl {
            Some(d) => self.in_deprecated_context(d),
            None => self.in_deprecated_context(self.ctx),
        };
        if quiet || self.depr_init.is_some_and(|v| self.in_deprecated_context(v)) {
            return;
        }
        for d in held {
            self.emit(d);
        }
    }
}
