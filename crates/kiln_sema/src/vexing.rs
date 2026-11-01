//! `-Wvexing-parse`'s redundant-parentheses warning and
//! `-Wredundant-parens` (clang's `warnAboutRedundantParens`, WP-191):
//! a declarator whose outermost declarator operator is a pair of grouping
//! parentheses, `T(x);`, which could instead have been a function-style
//! cast of an existing `x` (spec/sema.md §4.2).

use kiln_ast::QualType;
use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_diag::DiagId;

use crate::Sema;
use crate::decl::sym_str;
use crate::declspec::{Chunk, DeclSite, DeclSpec, Declarator, DeclaratorContext};
use crate::lookup::LookupKind;

impl Sema<'_, '_> {
    /// clang's `warnAboutRedundantParens`, called when the outermost
    /// declarator operator of `d` (the first one applied to the
    /// decl-specifier type `base`) is a pair of grouping parentheses.
    /// `spec` is the decl-specifier-seq and its type for a declarator that
    /// could be an expression (clang's `isExpressionContext`); `None` for a
    /// parameter, exception declaration or type-id (WP-235), which can only
    /// get `-Wredundant-parens`.
    pub(crate) fn warn_redundant_parens(
        &mut self,
        spec: Option<(&DeclSpec, QualType)>,
        d: &Declarator,
    ) {
        if !self.inst.stack.is_empty() || d.site == DeclSite::Subst {
            return;
        }
        // The parentheses that close last among those around every chunk
        // (`T((a))`: the outer pair).
        let Some(pi) = d.parens.iter().rposition(|p| p.0 == d.chunks.len()) else {
            return;
        };
        let (_, lparen, _) = d.parens[pi];
        // In a new-type-id, function chunks require parentheses.
        if d.site == DeclSite::New && d.chunks.iter().any(|c| matches!(c, Chunk::Function { .. })) {
            return;
        }
        let Some((ds, base)) = spec.filter(|_| {
            matches!(
                d.ctx,
                DeclaratorContext::Block | DeclaratorContext::Condition | DeclaratorContext::ForInit
            )
        }) else {
            self.warn_plain_redundant_parens(d, lparen);
            return;
        };
        let t = &self.ast.types;
        let simple_name = d.special.is_none() && d.op_name.is_none();
        let mut could_be_temporary = !d.invalid
            && !ds.invalid
            && simple_name
            && d.name.is_some()
            // clang's `getParsedSpecifiers() == PQ_TypeSpecifier`.
            && ds.has_type_specifier()
            && ds.storage.is_none()
            && ds.thread_local.is_none()
            && ds.inline.is_none()
            && ds.virtual_.is_none()
            && ds.explicit.is_none()
            && ds.cv.is_empty()
            && ds.atomic_qual.is_none()
            && (t.as_record(base).is_some() || t.is_dependent(base))
            && !d.after_comma;
        for c in &d.chunks {
            match c {
                Chunk::Pointer { .. } => {}
                Chunk::Array { size, .. } => {
                    if size.is_none() {
                        could_be_temporary = false;
                    }
                }
                Chunk::Function { .. }
                | Chunk::LRef { .. }
                | Chunk::RRef { .. }
                | Chunk::MemberPointer { .. } => could_be_temporary = false,
            }
        }
        let name = d.name.map(|n| n.0);
        if could_be_temporary
            && let Some(n) = name
            && self.lookup_name(n, LookupKind::Ordinary).is_empty()
        {
            could_be_temporary = false;
        }
        if !could_be_temporary {
            self.warn_plain_redundant_parens(d, lparen);
            return;
        }
        let Some(n) = name else { return };
        let ident = sym_str(self, n);
        let mut w = self.diag(DiagId::sema_parens_disambiguated_as_variable, lparen).arg(&ident);
        let trivial_complete = self
            .ast
            .types
            .as_record(base)
            .and_then(|r| self.ast.definition(r))
            .is_some_and(|def| self.defn_bits(def).contains(DefinitionBits::TRIVIAL_DTOR));
        if !trivial_complete {
            let note = self
                .diag(DiagId::note_raii_guard_add_name, lparen)
                .arg(self.ty_arg(base))
                .arg(&ident);
            w = w.with_note(note);
        }
        w = w
            .with_note(self.diag(DiagId::note_function_style_cast_add_parentheses, ds.loc))
            .with_note(self.diag(DiagId::note_remove_parens_for_variable_declaration, lparen));
        self.emit(w);
    }

    /// The off-by-default "redundant parentheses surrounding declarator"
    /// [-Wredundant-parens] at the outermost grouping `(` of `d`, except
    /// for `A (::B)`, where the parentheses change the meaning.
    fn warn_plain_redundant_parens(&mut self, d: &Declarator, lparen: kiln_source::Loc) {
        let starts_with_declarator_id = d.parens.len() == 1
            && d.chunks.iter().all(|c| {
                !matches!(
                    c,
                    Chunk::Pointer { .. }
                        | Chunk::LRef { .. }
                        | Chunk::RRef { .. }
                        | Chunk::MemberPointer { .. }
                )
            });
        if starts_with_declarator_id
            && d.qualifier.is_some_and(|q| self.kind_of(q) == DeclKind::TranslationUnit)
        {
            return;
        }
        let w = self.diag(DiagId::sema_redundant_parens_around_declarator, lparen);
        self.emit(w);
    }
}
