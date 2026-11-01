//! `-Wvexing-parse`'s redundant-parentheses warning and
//! `-Wredundant-parens` (clang's `warnAboutRedundantParens`, WP-191):
//! a declarator whose outermost declarator operator is a pair of grouping
//! parentheses, `T(x);`, which could instead have been a function-style
//! cast of an existing `x` (spec/sema.md §4.2).

use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::QualType;
use kiln_diag::DiagId;

use crate::declspec::{Chunk, DeclSpec, Declarator, DeclaratorContext};
use crate::lookup::LookupKind;
use crate::Sema;
use crate::decl::sym_str;

impl Sema<'_, '_> {
    /// clang's `warnAboutRedundantParens`, called when the outermost
    /// declarator operator of `d` (the first one applied to the
    /// decl-specifier type `base`) is a pair of grouping parentheses.
    pub(crate) fn warn_redundant_parens(&mut self, ds: &DeclSpec, base: QualType, d: &Declarator) {
        if !self.inst.stack.is_empty() {
            return;
        }
        // The parentheses that close last among those around every chunk
        // (`T((a))`: the outer pair).
        let Some(pi) = d.parens.iter().rposition(|p| p.0 == d.chunks.len()) else {
            return;
        };
        let (_, lparen, _) = d.parens[pi];
        let t = &self.ast.types;
        let simple_name = d.special.is_none() && d.op_name.is_none();
        let mut could_be_temporary = matches!(
            d.ctx,
            DeclaratorContext::Block | DeclaratorContext::Condition | DeclaratorContext::ForInit
        ) && !d.invalid
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
        let mut starts_with_declarator_id = true;
        if d.parens.len() > 1 {
            starts_with_declarator_id = false;
        }
        for c in &d.chunks {
            match c {
                Chunk::Pointer { .. } => starts_with_declarator_id = false,
                Chunk::Array { size, .. } => {
                    if size.is_none() {
                        could_be_temporary = false;
                    }
                }
                Chunk::Function { .. } => could_be_temporary = false,
                Chunk::LRef { .. } | Chunk::RRef { .. } | Chunk::MemberPointer { .. } => {
                    could_be_temporary = false;
                    starts_with_declarator_id = false;
                }
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
            // `A (::B)`: the parentheses change the meaning.
            if starts_with_declarator_id
                && d.qualifier.is_some_and(|q| self.kind_of(q) == DeclKind::TranslationUnit)
            {
                return;
            }
            let w = self.diag(DiagId::sema_redundant_parens_around_declarator, lparen);
            self.emit(w);
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
            let note = self.diag(DiagId::note_raii_guard_add_name, lparen).arg(self.ty_arg(base)).arg(&ident);
            w = w.with_note(note);
        }
        w = w
            .with_note(self.diag(DiagId::note_function_style_cast_add_parentheses, ds.loc))
            .with_note(self.diag(DiagId::note_remove_parens_for_variable_declaration, lparen));
        self.emit(w);
    }
}
